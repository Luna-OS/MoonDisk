//! Tauri IPC layer: thin commands that deserialize, delegate to
//! `operations`/`platform`, and map errors to strings. No business logic
//! lives here — see `docs/architecture.md` §6.1.
//!
//! Scope note: this is a deliberately simplified version of the
//! multi-operation planner with dry-run tickets originally sketched in
//! `docs/safety-model.md` §5 (`OperationPlanner`, fingerprints, a
//! ticketed dry-run step). Given the scope of actually getting a real,
//! working write path shipped, each operation here is validated and
//! executed individually rather than queued into a multi-step plan first.
//! The safety-critical parts of that design are still here — re-fetch the
//! real current state, re-validate, require the typed confirmation phrase
//! for anything at High risk or above, hard-block the system disk — just
//! without the batching. A real multi-op planner UI is future work.

use crate::models::{Disk, DiskId};
use crate::operations::{
    validate, Confirmation, DiskOperationExecutor, ExecutionError, OperationRequest, RiskLevel,
};
use crate::platform::mock::MockDiskProvider;
use crate::platform::DiskInventory;
use crate::security::confirmation::phrase_matches;
use serde::Serialize;
use std::sync::Mutex;

/// `mock`: the built-in sample disks, nothing ever really changes.
/// `real`: the platform's actual disks, with real writes.
///
/// Defaults to `mock` so a fresh install/run never touches a real disk
/// without the person running it having deliberately asked for that.
/// Switching to `real` (`MOONDISK_MODE=real`) is an explicit, informed
/// choice — the same posture `docs/safety-model.md` describes, even
/// though — per an explicit, informed project decision — MoonDisk is no
/// longer mock-only software. Once in `real` mode, the safety net is
/// everything else in this module and in `security`/`operations`: every
/// write is re-validated against the live disk state, the disk hosting
/// the running OS is hard-blocked (`security::system_protection`), and
/// anything at or above `RiskLevel::High` requires the typed confirmation
/// phrase (`docs/safety-model.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Mock,
    Real,
}

impl Mode {
    pub fn from_env() -> Self {
        match std::env::var("MOONDISK_MODE").as_deref() {
            Ok("real") => Mode::Real,
            _ => Mode::Mock,
        }
    }
}

pub struct AppState {
    pub mode: Mode,
    /// Serializes real executions: two overlapping writes to the same
    /// disk from concurrent frontend calls would each individually
    /// re-validate against a state that could shift under them otherwise.
    exec_lock: Mutex<()>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            mode: Mode::from_env(),
            exec_lock: Mutex::new(()),
        }
    }

    fn inventory(&self) -> Box<dyn DiskInventory> {
        match self.mode {
            Mode::Mock => Box::new(MockDiskProvider),
            #[cfg(target_os = "linux")]
            Mode::Real => Box::new(crate::platform::linux::LinuxDiskProvider),
            #[cfg(target_os = "windows")]
            Mode::Real => Box::new(crate::platform::windows::WindowsDiskProvider),
            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            Mode::Real => Box::new(MockDiskProvider),
        }
    }

    fn executor(&self) -> Box<dyn DiskOperationExecutor> {
        match self.mode {
            Mode::Mock => Box::new(crate::operations::executor::NullExecutor::new()),
            #[cfg(target_os = "linux")]
            Mode::Real => Box::new(crate::platform::linux_executor::LinuxDiskExecutor),
            #[cfg(target_os = "windows")]
            Mode::Real => Box::new(crate::platform::windows_executor::WindowsDiskExecutor),
            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            Mode::Real => Box::new(crate::operations::executor::NullExecutor::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub mode: Mode,
}

#[tauri::command]
pub fn app_info(state: tauri::State<AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        mode: state.mode,
    }
}

#[tauri::command]
pub fn disks_list(state: tauri::State<AppState>) -> Result<Vec<Disk>, String> {
    state.inventory().list_disks().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disk_get(state: tauri::State<AppState>, disk_id: String) -> Result<Disk, String> {
    state
        .inventory()
        .disk(&DiskId::from(disk_id))
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct OperationOutcome {
    pub applied: bool,
}

/// Validates and, if it passes, executes a single operation.
///
/// `confirmation_phrase` is required (and checked against
/// `security::confirmation`) for anything the request itself reports as
/// `RiskLevel::High` or above — the frontend must not decide this on its
/// own; the classification lives in `OperationRequest::risk` and is
/// re-checked here regardless of what the UI already showed the user.
#[tauri::command]
pub fn execute_operation(
    state: tauri::State<AppState>,
    request: OperationRequest,
    confirmation_phrase: Option<String>,
    language: String,
) -> Result<OperationOutcome, String> {
    let _guard = state
        .exec_lock
        .lock()
        .map_err(|_| "interner Zustand beschädigt".to_string())?;

    let disk = state
        .inventory()
        .disk(&request.disk_id())
        .map_err(|e| ExecutionError::from(e).to_string())?;
    validate(&disk, &request)
        .map_err(ExecutionError::from)
        .map_err(|e| e.to_string())?;

    let phrase_confirmed = if request.risk() >= RiskLevel::High {
        match confirmation_phrase {
            Some(p) if phrase_matches(&p, &language) => true,
            _ => return Err(ExecutionError::ConfirmationMissing.to_string()),
        }
    } else {
        false
    };

    state
        .executor()
        .execute(&request, Confirmation { phrase_confirmed })
        .map(|_| OperationOutcome { applied: true })
        .map_err(|e| e.to_string())
}
