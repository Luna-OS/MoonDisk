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

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
compile_error!("MoonDisk only supports Windows, Linux and macOS");

use crate::flash::{FlashError, ImageInfo, Phase, WriteMode};
use crate::models::ByteSize;
use crate::models::{Disk, DiskId};
use crate::operations::{
    validate, Confirmation, DiskOperationExecutor, ExecutionError, OperationRequest, RiskLevel,
};
use crate::platform::DiskInventory;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Emitter;

pub struct AppState {
    /// Serializes real executions: two overlapping writes to the same
    /// disk from concurrent frontend calls would each individually
    /// re-validate against a state that could shift under them otherwise.
    exec_lock: Mutex<()>,
    flash: Arc<FlashControl>,
}

#[derive(Default)]
struct FlashControl {
    busy: AtomicBool,
    cancel: AtomicBool,
}

fn platform_inventory() -> Box<dyn DiskInventory> {
    #[cfg(target_os = "linux")]
    {
        Box::new(crate::platform::linux::LinuxDiskProvider)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(crate::platform::windows::WindowsDiskProvider)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(crate::platform::macos::MacDiskProvider)
    }
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            exec_lock: Mutex::new(()),
            flash: Arc::default(),
        }
    }

    fn inventory(&self) -> Box<dyn DiskInventory> {
        platform_inventory()
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
        #[cfg(target_os = "macos")]
        {
            Box::new(crate::platform::macos_executor::MacDiskExecutor)
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
    /// "windows", "linux" or "macos" — lets the frontend hide platform-only
    /// actions (e.g. drive letters only exist on Windows) without guessing
    /// from other data.
    pub platform: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
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
    if state.flash.busy.load(Ordering::SeqCst) {
        return Err("an image is being written to a USB drive — wait until it finishes".into());
    }
    let _guard = state
        .exec_lock
        .lock()
        .map_err(|_| "internal state is corrupted".to_string())?;

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

/// Opens the native file picker for a disk image. `None` if the user
/// closed it without choosing a file.
#[tauri::command]
pub async fn select_image_file(app: tauri::AppHandle) -> Result<Option<ImageInfo>, String> {
    use tauri_plugin_dialog::DialogExt;
    // `blocking_pick_file` must not run on the main thread; async commands
    // run on the async runtime's worker threads.
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Choose a disk image")
        .add_filter("Disk images", &["iso", "img", "raw", "bin"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    crate::flash::image_info(&path)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlashProgress {
    phase: Phase,
    done: ByteSize,
    total: ByteSize,
    bytes_per_second: f64,
}

/// Writes an image file onto a whole USB drive, erasing it. Emits
/// `flash-progress` events while running and resolves once the drive is
/// written (and verified, if `verify`).
#[tauri::command]
pub async fn flash_image(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    image_path: String,
    disk_id: String,
    mode: WriteMode,
    verify: bool,
    confirmed: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err(ExecutionError::ConfirmationMissing.to_string());
    }
    let control = state.flash.clone();
    if control.busy.swap(true, Ordering::SeqCst) {
        return Err("another image is already being written".into());
    }
    control.cancel.store(false, Ordering::SeqCst);

    let worker = control.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), FlashError> {
        let disk = platform_inventory()
            .disk(&DiskId::from(disk_id))
            .map_err(ExecutionError::from)?;

        let mut phase = Phase::Preparing;
        let mut phase_started = Instant::now();
        let mut last_emit: Option<Instant> = None;
        crate::flash::run(
            &PathBuf::from(image_path),
            &disk,
            mode,
            verify,
            &worker.cancel,
            |p, done, total| {
                let now = Instant::now();
                if p != phase {
                    phase = p;
                    phase_started = now;
                    last_emit = None;
                }
                let due = last_emit.map_or(true, |t| now - t >= Duration::from_millis(100));
                if !due && done < total {
                    return;
                }
                last_emit = Some(now);
                let secs = (now - phase_started).as_secs_f64();
                let _ = app.emit(
                    "flash-progress",
                    FlashProgress {
                        phase: p,
                        done: ByteSize(done),
                        total: ByteSize(total),
                        bytes_per_second: if secs > 0.0 { done as f64 / secs } else { 0.0 },
                    },
                );
            },
        )
    })
    .await;
    control.busy.store(false, Ordering::SeqCst);

    match result {
        Ok(r) => r.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Asks a running `flash_image` to stop after the current chunk.
#[tauri::command]
pub fn flash_cancel(state: tauri::State<AppState>) {
    state.flash.cancel.store(true, Ordering::SeqCst);
}
