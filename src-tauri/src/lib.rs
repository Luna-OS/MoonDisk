//! MoonDisk – Tauri application entry point.
//!
//! MoonDisk performs real disk writes against the platform's actual disks.
//! Every write goes through validation → an explicit confirmation click →
//! `operations::DiskOperationExecutor`.

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
