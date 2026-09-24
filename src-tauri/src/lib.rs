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
    let builder = tauri::Builder::default();

    // The updater/process plugins are desktop-only (Tauri has no mobile
    // update mechanism of its own); MoonDisk itself is desktop-only too,
    // but the app is scaffolded against the general Tauri 2 mobile-capable
    // template, so this is guarded the same way the template guards it.
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
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
