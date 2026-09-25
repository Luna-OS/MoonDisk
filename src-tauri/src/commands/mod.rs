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

use crate::flash::{ImageInfo, WriteMode};
use crate::models::{Disk, DiskId};
use crate::operations::{ExecutionError, OperationRequest};
use crate::service;
use serde::Serialize;
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

impl AppState {
    pub fn new() -> Self {
        AppState {
            exec_lock: Mutex::new(()),
            flash: Arc::default(),
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
        platform: service::platform_name(),
    }
}

#[tauri::command]
pub fn disks_list() -> Result<Vec<Disk>, String> {
    service::list_disks()
}

#[tauri::command]
pub fn disk_get(disk_id: String) -> Result<Disk, String> {
    service::inventory()
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
    service::execute(&request, confirmed).map(|_| OperationOutcome { applied: true })
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
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let mut image = File::open(&image_path).map_err(|e| format!("{image_path}: {e}"))?;
        service::flash(
            &mut image,
            &disk_id,
            mode,
            verify,
            &worker.cancel,
            |progress| {
                let _ = app.emit("flash-progress", progress);
            },
        )
    })
    .await;
    control.busy.store(false, Ordering::SeqCst);

    result.map_err(|e| e.to_string())?
}

/// Asks a running `flash_image` to stop after the current chunk.
#[tauri::command]
pub fn flash_cancel(state: tauri::State<AppState>) {
    state.flash.cancel.store(true, Ordering::SeqCst);
}
