//! MoonDisk – Tauri application entry point.
//!
//! See `docs/architecture.md` §5 for the module layout and
//! `docs/safety-model.md` for the safety architecture that still applies
//! even though MoonDisk performs real disk writes: every write goes
//! through validation → a confirmation the user must explicitly give →
//! `operations::DiskOperationExecutor`, and the executor hard-blocks the
//! disk hosting the running OS before any write happens. See
//! `commands::Mode` for how mock vs. real mode is selected
//! (`MOONDISK_MODE=real`; mock is the default).

pub mod commands;
pub mod models;
pub mod operations;
pub mod platform;
pub mod security;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::disks_list,
            commands::disk_get,
            commands::execute_operation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the MoonDisk application");
}
