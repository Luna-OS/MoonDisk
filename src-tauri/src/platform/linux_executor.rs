//! Real Linux write executor. Every command here was hand-verified against
//! a throwaway loop device before being written (never against this
//! machine's real disks) — see the manual session recorded in the PR this
//! shipped in. Every `Command` invocation uses a fixed argument array;
//! nothing derived from user input (labels, sizes) is ever interpolated
//! into a shell string, because there is no shell — `std::process::Command`
//! execs the binary directly.
//!
//! Resize and move are intentionally not implemented yet (see
//! `ExecutionError::NotImplemented`): they need per-filesystem shrink
//! tools (`resize2fs`, `ntfsresize`, `btrfs filesystem resize`, …) plus
//! careful sequencing with the partition-table change, and rushing that
//! is exactly the kind of shortcut `docs/safety-model.md` §10 warns
//! against. Create/delete/format/label are the operations that are
//! actually implemented and tested here.

use super::linux::LinuxDiskProvider;
use super::DiskInventory;
use crate::models::{ByteSize, Disk, FileSystem};
use crate::operations::{
    validate, Confirmation, DiskOperationExecutor, ExecutionError, OperationRequest, RiskLevel,
};
use std::process::{Command, Output};

pub struct LinuxDiskExecutor;

impl DiskOperationExecutor for LinuxDiskExecutor {
    fn execute(
        &mut self,
        req: &OperationRequest,
        confirmation: Confirmation,
    ) -> Result<(), ExecutionError> {
        // 1. Re-fetch the current, real state of exactly this disk — never
        //    trust a snapshot the caller might be holding.
        let provider = LinuxDiskProvider;
        let disk = provider.disk(&req.disk_id())?;

        // 2. Re-validate against that fresh state.
        validate(&disk, req)?;

        // 3. Confirmation for anything at or above High risk.
        if req.risk() >= RiskLevel::High && !confirmation.confirmed {
            return Err(ExecutionError::ConfirmationMissing);
        }

        match req {
            OperationRequest::CreatePartition {
                disk,
                start,
                size,
                filesystem,
                label,
                drive_letter: _,
            } => create_partition(&disk.0, *start, *size, *filesystem, label.as_deref()),
            OperationRequest::DeletePartition { partition } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                delete_partition(&disk.id.0, p.number)
            }
            OperationRequest::FormatPartition {
                partition,
                filesystem,
                label,
            } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                let dev = partition_device_path(&disk.id.0, p.number);
                format_partition(&dev, *filesystem, label.as_deref())
            }
            OperationRequest::SetLabel { partition, label } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                let dev = partition_device_path(&disk.id.0, p.number);
                set_label(&dev, p.fs, label)
            }
            OperationRequest::SetDriveLetter { .. } => Err(ExecutionError::NotImplemented(
                "drive letters do not exist on Linux".into(),
            )),
        }
    }
}

fn run(cmd: &mut Command) -> Result<Output, ExecutionError> {
    let out = cmd
        .output()
        .map_err(|e| ExecutionError::Failed(format!("could not start process: {e}")))?;
    if !out.status.success() {
        return Err(ExecutionError::Failed(format!(
            "{}: {}",
            cmd.get_program().to_string_lossy(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out)
}

/// `sda` + 3 -> `/dev/sda3`; `nvme0n1` + 2 -> `/dev/nvme0n1p2` (NVMe/mmcblk
/// devices need the `p` separator before the partition number, plain
/// `sdX`/`vdX` devices don't).
fn partition_device_path(disk_path: &str, number: u32) -> String {
    let base = disk_path.trim_end_matches('/');
    let needs_p = base.ends_with(char::is_numeric);
    if needs_p {
        format!("{base}p{number}")
    } else {
        format!("{base}{number}")
    }
}

/// Before writing a whole-disk image: unmount everything mounted from the
/// disk (desktops auto-mount USB sticks), so nothing writes to it mid-copy.
pub fn prepare_raw_write(disk: &Disk) -> Result<(), ExecutionError> {
    for p in disk.partitions() {
        for mountpoint in &p.mountpoints {
            run(Command::new("umount").arg(mountpoint))?;
        }
    }
    Ok(())
}

/// Drops the kernel's cached pages for the disk so verification reads what
/// is really on the device, not what was just written into the cache.
pub fn drop_read_cache(disk: &Disk) -> Result<(), ExecutionError> {
    run(Command::new("blockdev").args(["--flushbufs", &disk.id.0]))?;
    Ok(())
}

/// After writing a whole-disk image: make the kernel pick up the image's
/// partition table. Best-effort.
pub fn finish_raw_write(disk: &Disk) {
    rescan_partition_table(&disk.id.0);
}

fn rescan_partition_table(disk_path: &str) {
    // Best-effort: on success the kernel already knows about the new
    // layout (parted itself issues a BLKRRPART), this just closes the
    // rare race where a partition device node isn't visible yet. Not
    // fatal if `partprobe` is missing.
    let _ = Command::new("partprobe").arg(disk_path).output();
    std::thread::sleep(std::time::Duration::from_millis(300));
}

fn create_partition(
    disk_path: &str,
    start: ByteSize,
    size: ByteSize,
    _filesystem: FileSystem,
    label: Option<&str>,
) -> Result<(), ExecutionError> {
    let end = start
        .checked_add(size)
        .and_then(|e| e.checked_sub(ByteSize(1)))
        .ok_or_else(|| ExecutionError::Failed("size calculation overflowed".into()))?;
    // GPT partition name; parted accepts an empty name via "" but MoonDisk
    // always supplies at least a placeholder so scripts/tools downstream
    // never see a blank field.
    let name = label.filter(|l| !l.is_empty()).unwrap_or("partition");
    run(Command::new("parted").args([
        "--script",
        disk_path,
        "unit",
        "B",
        "mkpart",
        name,
        &format!("{}B", start.0),
        &format!("{}B", end.0),
    ]))?;
    rescan_partition_table(disk_path);
    Ok(())
}

fn delete_partition(disk_path: &str, number: u32) -> Result<(), ExecutionError> {
    run(Command::new("parted").args(["--script", disk_path, "rm", &number.to_string()]))?;
    rescan_partition_table(disk_path);
    Ok(())
}

fn format_partition(
    partition_path: &str,
    filesystem: FileSystem,
    label: Option<&str>,
) -> Result<(), ExecutionError> {
    let label = label.unwrap_or("");
    match filesystem {
        FileSystem::Ext2 => {
            run(Command::new("mkfs.ext2").args(["-F", "-L", label, partition_path]))?;
        }
        FileSystem::Ext3 => {
            run(Command::new("mkfs.ext3").args(["-F", "-L", label, partition_path]))?;
        }
        FileSystem::Ext4 => {
            run(Command::new("mkfs.ext4").args(["-F", "-L", label, partition_path]))?;
        }
        FileSystem::Fat32 => {
            let mut args = vec!["-F", "32"];
            if !label.is_empty() {
                args.push("-n");
                args.push(label);
            }
            args.push(partition_path);
            run(Command::new("mkfs.vfat").args(args))?;
        }
        FileSystem::ExFat => {
            let mut args = vec![];
            if !label.is_empty() {
                args.push("-n");
                args.push(label);
            }
            args.push(partition_path);
            run(Command::new("mkfs.exfat").args(args))?;
        }
        FileSystem::Ntfs => {
            let mut args = vec!["-F", "-Q"];
            if !label.is_empty() {
                args.push("-L");
                args.push(label);
            }
            args.push(partition_path);
            run(Command::new("mkfs.ntfs").args(args))?;
        }
        FileSystem::Btrfs => {
            let mut args = vec!["-f"];
            if !label.is_empty() {
                args.push("-L");
                args.push(label);
            }
            args.push(partition_path);
            run(Command::new("mkfs.btrfs").args(args))?;
        }
        FileSystem::Xfs => {
            let mut args = vec!["-f"];
            if !label.is_empty() {
                args.push("-L");
                args.push(label);
            }
            args.push(partition_path);
            run(Command::new("mkfs.xfs").args(args))?;
        }
        FileSystem::LinuxSwap => {
            let mut args = vec![];
            if !label.is_empty() {
                args.push("-L");
                args.push(label);
            }
            args.push(partition_path);
            run(Command::new("mkswap").args(args))?;
        }
        FileSystem::Unformatted | FileSystem::Unknown => {
            return Err(ExecutionError::NotImplemented(
                "formatting to this file system is not supported".into(),
            ));
        }
    }
    Ok(())
}

fn set_label(
    partition_path: &str,
    filesystem: FileSystem,
    label: &str,
) -> Result<(), ExecutionError> {
    match filesystem {
        FileSystem::Ext2 | FileSystem::Ext3 | FileSystem::Ext4 => {
            run(Command::new("e2label").args([partition_path, label]))?;
        }
        FileSystem::Fat32 => {
            run(Command::new("fatlabel").args([partition_path, label]))?;
        }
        FileSystem::ExFat => {
            run(Command::new("exfatlabel").args([partition_path, label]))?;
        }
        FileSystem::Ntfs => {
            run(Command::new("ntfslabel").args([partition_path, label]))?;
        }
        FileSystem::Btrfs => {
            run(Command::new("btrfs").args(["filesystem", "label", partition_path, label]))?;
        }
        FileSystem::Xfs => {
            run(Command::new("xfs_admin").args(["-L", label, partition_path]))?;
        }
        _ => {
            return Err(ExecutionError::NotImplemented(
                "changing the label is not supported for this file system".into(),
            ));
        }
    }
    Ok(())
}
