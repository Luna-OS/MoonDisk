// Prevents an additional console window from appearing on Windows release
// builds. See src-tauri/src/lib.rs for the actual application entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    moondisk_lib::run();
}
