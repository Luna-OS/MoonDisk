//! MoonDisk – Tauri application entry point.
//!
//! MoonDisk performs real disk writes against the platform's actual disks.
//! Every write goes through validation → an explicit confirmation click →
//! `operations::DiskOperationExecutor`.

#[cfg(feature = "gui")]
pub mod commands;
#[cfg(unix)]
pub mod fdpass;
pub mod flash;
#[cfg(unix)]
pub mod helper;
pub mod models;
pub mod operations;
pub mod platform;
pub mod security;
pub mod service;

#[cfg(feature = "gui")]
use commands::AppState;

#[cfg(feature = "gui")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::disks_list,
            commands::disk_get,
            commands::execute_operation,
            commands::select_image_file,
            commands::flash_image,
            commands::flash_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the MoonDisk application");
}
