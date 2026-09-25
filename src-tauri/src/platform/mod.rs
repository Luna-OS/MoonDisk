//! Platform abstraction: one trait, several backends.
//!
//! `DiskInventory` is the read side (list disks/partitions).
//! `operations::DiskOperationExecutor` (see `operations/executor.rs`) is
//! the write side. Every OS-specific detail — `lsblk` parsing, PowerShell
//! cmdlets, mock data — lives behind these two traits so the rest of the
//! app (planner, validator, Tauri commands, UI) never branches on target
//! OS itself.

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub mod linux_executor;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod windows_executor;

/// Writes a fixed, embedded PowerShell script (via `include_str!` at the
/// call site) to a temp file and runs it with `-File`, passing `params` as
/// separate, already-quoted argv entries (`["-DiskNumber", "0", …]`) so
/// PowerShell's own `param()` binding receives clean typed values — never
/// a string built by concatenating a label or size into script text. See
/// `platform/windows_scripts/` for the scripts themselves.
#[cfg(target_os = "windows")]
pub fn run_powershell_script(script: &str, params: &[&str]) -> Result<String, std::io::Error> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;

    // Without this flag, spawning a console app like powershell.exe from a
    // GUI app briefly flashes a visible console window for every single
    // operation. CREATE_NO_WINDOW (winbase.h) suppresses that; the script
    // still runs and its output is still captured via the piped stdout/
    // stderr below.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut path = std::env::temp_dir();
    path.push(format!("moondisk-{}.ps1", uuid_like()));
    {
        let mut f = std::fs::File::create(&path)?;
        f.write_all(script.as_bytes())?;
    }

    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]);
    cmd.arg(&path);
    cmd.args(params);
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output();
    let _ = std::fs::remove_file(&path);
    let output = output?;

    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "PowerShell endete mit Fehler: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "windows")]
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{:x}", std::process::id())
}

use crate::models::{Disk, DiskId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("Datenträger {0} wurde nicht gefunden")]
    DiskNotFound(DiskId),
    #[error("Datenträgerinformationen konnten nicht gelesen werden: {0}")]
    ReadFailed(String),
    #[error("nicht unterstützt auf dieser Plattform")]
    Unsupported,
}

/// Read-only access to disk/partition information. No implementation of
/// this trait is permitted to write to a device — see
/// `docs/safety-model.md` §3 (layer L2, the type system itself keeps
/// inventory and execution separate).
pub trait DiskInventory: Send + Sync {
    fn source(&self) -> InventorySource;
    fn list_disks(&self) -> Result<Vec<Disk>, InventoryError>;
    fn disk(&self, id: &DiskId) -> Result<Disk, InventoryError> {
        self.list_disks()?
            .into_iter()
            .find(|d| &d.id == id)
            .ok_or_else(|| InventoryError::DiskNotFound(id.clone()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InventorySource {
    Linux,
    Windows,
}
