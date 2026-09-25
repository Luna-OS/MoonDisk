//! Tauri IPC layer: thin commands that deserialize, delegate to
//! `operations`/`platform`, and map errors to strings. No business logic
//! lives here.
//!
//! Scope note: this is a deliberately simplified version of a
//! multi-operation planner with dry-run tickets. Each operation here is
//! validated and executed individually rather than queued into a
//! multi-step plan first. The safety-critical part of that design is
//! still here — re-fetch the real current state, re-validate, require an
//! explicit confirmation click for anything at High risk or above — just
//! without the batching. A real multi-op planner UI is future work.
//!
//! MoonDisk only ever operates on the platform's actual disks — there is
//! no mock/simulated mode.

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("MoonDisk unterstützt nur Windows und Linux");

use crate::models::{Disk, DiskId};
use crate::operations::{
    validate, Confirmation, DiskOperationExecutor, ExecutionError, OperationRequest, RiskLevel,
};
use crate::platform::DiskInventory;
use serde::Serialize;
use std::sync::Mutex;

pub struct AppState {
    /// Serializes real executions: two overlapping writes to the same
    /// disk from concurrent frontend calls would each individually
    /// re-validate against a state that could shift under them otherwise.
    exec_lock: Mutex<()>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            exec_lock: Mutex::new(()),
        }
    }

    fn inventory(&self) -> Box<dyn DiskInventory> {
        #[cfg(target_os = "linux")]
        {
            Box::new(crate::platform::linux::LinuxDiskProvider)
        }
        #[cfg(target_os = "windows")]
        {
            Box::new(crate::platform::windows::WindowsDiskProvider)
        }
    }

    fn executor(&self) -> Box<dyn DiskOperationExecutor> {
        #[cfg(target_os = "linux")]
        {
            Box::new(crate::platform::linux_executor::LinuxDiskExecutor)
        }
        #[cfg(target_os = "windows")]
        {
            Box::new(crate::platform::windows_executor::WindowsDiskExecutor)
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
    /// "windows" or "linux" — lets the frontend hide platform-only actions
    /// (e.g. drive letters only exist on Windows) without guessing from
    /// other data.
    pub platform: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        },
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
/// `confirmed` must be `true` for anything the request itself reports as
/// `RiskLevel::High` or above — the frontend must not decide this on its
/// own; the classification lives in `OperationRequest::risk` and is
/// re-checked here regardless of what the UI already showed the user.
#[tauri::command]
pub fn execute_operation(
    state: tauri::State<AppState>,
    request: OperationRequest,
    confirmed: bool,
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

    if request.risk() >= RiskLevel::High && !confirmed {
        return Err(ExecutionError::ConfirmationMissing.to_string());
    }

    state
        .executor()
        .execute(&request, Confirmation { confirmed })
        .map(|_| OperationOutcome { applied: true })
        .map_err(|e| e.to_string())
}
