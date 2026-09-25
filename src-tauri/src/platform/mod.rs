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

// Also compiled for tests on Linux: the diskutil/ioreg parsing and the
// argument building are plain functions tested there too.
#[cfg(any(target_os = "macos", all(test, unix)))]
pub mod macos;

#[cfg(any(target_os = "macos", all(test, unix)))]
pub mod macos_executor;

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
            "PowerShell failed: {}",
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

use crate::models::{Disk, DiskId, PartitionTable};
use thiserror::Error;

/// No partition may start in the first sectors of a disk (MBR / GPT
/// protective MBR + primary GPT header) or, on GPT, reach into the last
/// ones (backup GPT header + entry array). Both are well under 1 MiB, and
/// Windows and parted both align partitions to 1 MiB anyway, so exactly
/// 1 MiB is reserved at each end — the same grid as everything else.
pub(crate) const PARTITION_TABLE_RESERVE: u64 = 1024 * 1024;

/// The byte range `[start, end)` partitions may occupy on a disk of
/// `disk_size` bytes. A disk with no table yet is treated like GPT, since
/// that's what MoonDisk initializes it as before creating a partition.
pub(crate) fn usable_range(disk_size: u64, table: PartitionTable) -> (u64, u64) {
    let start = PARTITION_TABLE_RESERVE.min(disk_size);
    let end = match table {
        PartitionTable::Mbr => disk_size,
        PartitionTable::Gpt | PartitionTable::None => {
            disk_size.saturating_sub(PARTITION_TABLE_RESERVE)
        }
    };
    (start, end.max(start))
}

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("disk {0} not found")]
    DiskNotFound(DiskId),
    #[error("could not read disk information: {0}")]
    ReadFailed(String),
    #[error("not supported on this platform")]
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
    MacOs,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1024 * 1024;

    #[test]
    fn usable_range_never_starts_at_byte_zero() {
        for table in [
            PartitionTable::Gpt,
            PartitionTable::Mbr,
            PartitionTable::None,
        ] {
            let (start, _) = usable_range(100 * MIB, table);
            assert_eq!(start, MIB, "{table:?}");
        }
    }

    #[test]
    fn usable_range_reserves_the_gpt_backup_table_but_not_on_mbr() {
        assert_eq!(usable_range(100 * MIB, PartitionTable::Gpt).1, 99 * MIB);
        assert_eq!(usable_range(100 * MIB, PartitionTable::None).1, 99 * MIB);
        assert_eq!(usable_range(100 * MIB, PartitionTable::Mbr).1, 100 * MIB);
    }

    #[test]
    fn usable_range_is_empty_not_inverted_on_a_tiny_disk() {
        let (start, end) = usable_range(MIB / 2, PartitionTable::Gpt);
        assert!(end >= start);
    }
}
