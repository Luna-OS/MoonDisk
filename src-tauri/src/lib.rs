//! MoonDisk – Tauri application entry point.
//!
//! This is the Phase 2 scaffold: it wires up the Tauri builder with no
//! custom commands or plugins yet. The module layout documented in
//! `docs/architecture.md` §5 and §10 (`commands/`, `platform/`,
//! `operations/`, `security/`, `reporting/`, `github/`, …) is added
//! incrementally in the phases that follow — see `docs/roadmap.md`.
//!
//! Nothing in this crate reads, simulates, or performs a disk operation
//! yet. Real (write) access to disks is out of scope for the entire
//! `0.1.0-alpha.x` series — see `docs/safety-model.md` §3.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running the MoonDisk application");
}
