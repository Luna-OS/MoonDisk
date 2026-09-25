//! macOS write executor — **not yet tested on a real Mac**. Every write is
//! one `diskutil` call with a fixed argument array (no shell), built by
//! [`diskutil_args`], which is unit-tested on Linux too.
//!
//! `diskutil` has no "create a partition at this offset": it can only add
//! a partition directly after an existing one (`addPartition`), or lay out
//! an empty disk from scratch (`partitionDisk`). Free space in front of the
//! first partition therefore can't be used on macOS.
//!
//! Writing a disk image needs root to open the raw device; macOS' own
//! `authopen` asks for the password and hands back an open file descriptor
//! over a socket, so MoonDisk itself never runs as root.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::models::{Disk, FileSystem, PartitionTable, Segment};
use crate::operations::{ExecutionError, OperationRequest};

/// `diskutil`'s name for a file system MoonDisk can create.
fn diskutil_format(fs: FileSystem) -> Result<&'static str, ExecutionError> {
    match fs {
        FileSystem::Apfs => Ok("APFS"),
        FileSystem::HfsPlus => Ok("JHFS+"),
        FileSystem::ExFat => Ok("ExFAT"),
        FileSystem::Fat32 => Ok("FAT32"),
        _ => Err(ExecutionError::NotImplemented(
            "macOS can only create APFS, Mac OS Extended, exFAT and FAT32".into(),
        )),
    }
}

/// `diskutil` needs a volume name for every format. FAT names are
/// upper-case by convention (and Windows shows them that way anyway).
fn volume_name(fs: FileSystem, label: Option<&str>) -> String {
    let name = label.filter(|l| !l.is_empty()).unwrap_or("Untitled");
    if fs == FileSystem::Fat32 {
        name.to_uppercase()
    } else {
        name.to_string()
    }
}

/// `/dev/disk4` + 2 -> `/dev/disk4s2`.
fn partition_device(disk: &Disk, number: u32) -> String {
    format!("{}s{number}", disk.id.0)
}

/// `/dev/disk4` -> `/dev/rdisk4`: the unbuffered "raw" device node, which
/// is many times faster for writing whole images.
pub(crate) fn raw_device_path(disk_path: &str) -> String {
    disk_path.replacen("/dev/disk", "/dev/rdisk", 1)
}

/// Sizes for `diskutil` in exact bytes: its K/M/G suffixes are powers of
/// ten, not the binary units MoonDisk shows.
fn bytes(n: u64) -> String {
    format!("{n}B")
}

/// The `diskutil` arguments that carry out `req` on `disk`. `req` must
/// already be validated against `disk`.
pub(crate) fn diskutil_args(
    disk: &Disk,
    req: &OperationRequest,
) -> Result<Vec<String>, ExecutionError> {
    let partition = |id| {
        disk.partitions()
            .find(|p| &p.id == id)
            .ok_or_else(|| ExecutionError::Failed(format!("partition {id} not found")))
    };
    let args = match req {
        OperationRequest::CreatePartition {
            start,
            size,
            filesystem,
            label,
            ..
        } => {
            let format = diskutil_format(*filesystem)?.to_string();
            let name = volume_name(*filesystem, label.as_deref());
            let before = disk
                .partitions()
                .filter(|p| p.start.0 < start.0)
                .max_by_key(|p| p.start.0);
            if let Some(p) = before {
                vec![
                    "addPartition".into(),
                    partition_device(disk, p.number),
                    format,
                    name,
                    bytes(size.0),
                ]
            } else if disk.partitions().next().is_none() {
                // An empty disk gets a fresh table with the new partition;
                // whatever the user left over stays free.
                let scheme = if disk.table == PartitionTable::Mbr {
                    "MBR"
                } else {
                    "GPT"
                };
                let free_end = disk
                    .layout
                    .iter()
                    .find_map(|s| match s {
                        Segment::Unallocated {
                            start: s,
                            size: len,
                        } if s.0 <= start.0 && start.0 < s.0 + len.0 => Some(s.0 + len.0),
                        _ => None,
                    })
                    .unwrap_or(start.0 + size.0);
                let mut args = vec![
                    "partitionDisk".into(),
                    disk.id.0.clone(),
                    scheme.into(),
                    format,
                    name,
                ];
                if start.0 + size.0 >= free_end {
                    args.push("R".into());
                } else {
                    args.extend([
                        bytes(size.0),
                        "Free Space".into(),
                        "%noformat%".into(),
                        "R".into(),
                    ]);
                }
                args
            } else {
                return Err(ExecutionError::NotImplemented(
                    "macOS can only add a partition directly after an existing one — \
                     free space in front of the first partition can't be used"
                        .into(),
                ));
            }
        }
        OperationRequest::DeletePartition { partition: id } => {
            let p = partition(id)?;
            vec![
                "eraseVolume".into(),
                "free".into(),
                "free".into(),
                partition_device(disk, p.number),
            ]
        }
        OperationRequest::FormatPartition {
            partition: id,
            filesystem,
            label,
        } => {
            let p = partition(id)?;
            vec![
                "eraseVolume".into(),
                diskutil_format(*filesystem)?.into(),
                volume_name(*filesystem, label.as_deref()),
                partition_device(disk, p.number),
            ]
        }
        OperationRequest::SetLabel {
            partition: id,
            label,
        } => {
            let p = partition(id)?;
            vec![
                "renameVolume".into(),
                partition_device(disk, p.number),
                label.clone(),
            ]
        }
        OperationRequest::EraseDisk {
            filesystem, label, ..
        } => {
            // MBR is what every OS and firmware reads on a USB stick; APFS
            // and Mac OS Extended need GPT.
            let scheme = match filesystem {
                FileSystem::Apfs | FileSystem::HfsPlus => "GPT",
                _ => "MBR",
            };
            vec![
                "eraseDisk".into(),
                diskutil_format(*filesystem)?.into(),
                volume_name(*filesystem, label.as_deref()),
                scheme.into(),
                disk.id.0.clone(),
            ]
        }
        OperationRequest::SetDriveLetter { .. } => {
            return Err(ExecutionError::NotImplemented(
                "drive letters only exist on Windows".into(),
            ))
        }
    };
    Ok(args)
}

/// Receives the file descriptor `authopen -stdoutpipe` sends back.
#[cfg(unix)]
pub(crate) fn receive_fd(
    socket: &std::os::unix::net::UnixStream,
) -> std::io::Result<std::os::fd::OwnedFd> {
    let mut data = [0u8; 64];
    let (_, fds) = crate::fdpass::recv(socket, &mut data)?;
    fds.into_iter().next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "macOS did not grant access to the drive (was the password dialog cancelled?)",
        )
    })
}

#[cfg(target_os = "macos")]
pub use real::*;

#[cfg(target_os = "macos")]
mod real {
    use super::*;
    use crate::operations::{validate, Confirmation, DiskOperationExecutor, RiskLevel};
    use crate::platform::macos::MacDiskProvider;
    use crate::platform::DiskInventory;
    use std::fs::File;
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};

    pub struct MacDiskExecutor;

    impl DiskOperationExecutor for MacDiskExecutor {
        fn execute(
            &mut self,
            req: &OperationRequest,
            confirmation: Confirmation,
        ) -> Result<(), ExecutionError> {
            let disk = MacDiskProvider.disk(&req.disk_id())?;
            validate(&disk, req)?;
            if req.risk() >= RiskLevel::High && !confirmation.confirmed {
                return Err(ExecutionError::ConfirmationMissing);
            }
            let args = diskutil_args(&disk, req)?;
            diskutil(&args.iter().map(String::as_str).collect::<Vec<_>>())
        }
    }

    fn diskutil(args: &[&str]) -> Result<(), ExecutionError> {
        let out = Command::new("diskutil")
            .args(args)
            .output()
            .map_err(|e| ExecutionError::Failed(format!("could not start diskutil: {e}")))?;
        if !out.status.success() {
            // diskutil prints most errors to stdout.
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let message = if stderr.trim().is_empty() {
                stdout.trim().lines().last().unwrap_or_default().to_string()
            } else {
                stderr.trim().to_string()
            };
            return Err(ExecutionError::Failed(format!("diskutil: {message}")));
        }
        Ok(())
    }

    /// Before writing a whole-disk image: unmount every volume on it.
    pub fn prepare_raw_write(disk: &Disk) -> Result<(), ExecutionError> {
        diskutil(&["unmountDisk", "force", &disk.id.0])
    }

    /// The raw device isn't cached, so there's nothing to drop.
    pub fn drop_read_cache(_disk: &Disk) -> Result<(), ExecutionError> {
        Ok(())
    }

    /// After writing a whole-disk image: mount what macOS can read of it.
    /// Best-effort — an installer image often has nothing macOS mounts.
    pub fn finish_raw_write(disk: &Disk) {
        let _ = diskutil(&["mountDisk", &disk.id.0]);
    }

    /// Opens the raw device for reading and writing. The native app's
    /// helper already runs as root; otherwise `authopen` asks for the
    /// password.
    pub fn open_raw(disk: &Disk) -> std::io::Result<File> {
        let raw = raw_device_path(&disk.id.0);
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&raw);
        }
        let (ours, theirs) = UnixStream::pair()?;
        let mut child = Command::new("/usr/libexec/authopen")
            .args(["-stdoutpipe", "-o", &libc::O_RDWR.to_string(), &raw])
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::os::fd::OwnedFd::from(theirs)))
            .stderr(Stdio::null())
            .spawn()?;
        // `Command` (and with it our copy of `theirs`) is gone by now, so
        // this sees end-of-file if authopen exits without sending an fd.
        let fd = receive_fd(&ours);
        let _ = child.wait();
        fd.map(File::from)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::models::{
        BusType, ByteSize, DiskId, HealthStatus, MediaType, Partition, PartitionFlags, PartitionId,
        PartitionKind,
    };

    const MIB: u64 = 1024 * 1024;

    fn partition(disk: &DiskId, number: u32, start: u64, size: u64) -> Segment {
        Segment::Partition(Partition {
            id: PartitionId::new(disk, number),
            number,
            start: ByteSize(start),
            size: ByteSize(size),
            used: None,
            fs: FileSystem::ExFat,
            kind: PartitionKind::BasicData,
            label: Some("STICK".into()),
            flags: PartitionFlags::empty(),
            drive_letter: None,
            mountpoints: vec![],
        })
    }

    fn disk(layout: Vec<Segment>) -> Disk {
        Disk {
            id: DiskId::from("/dev/disk4"),
            display_name: "Disk 4".into(),
            vendor: String::new(),
            model: "Stick".into(),
            serial: None,
            bus: BusType::Usb,
            media: MediaType::Unknown,
            size: ByteSize(1024 * MIB),
            logical_sector_size: 512,
            table: PartitionTable::Gpt,
            health: HealthStatus::Unknown,
            read_only: false,
            is_system_disk: false,
            layout,
        }
    }

    fn free(start: u64, size: u64) -> Segment {
        Segment::Unallocated {
            start: ByteSize(start),
            size: ByteSize(size),
        }
    }

    fn create(d: &Disk, start: u64, size: u64, fs: FileSystem, label: &str) -> OperationRequest {
        OperationRequest::CreatePartition {
            disk: d.id.clone(),
            start: ByteSize(start),
            size: ByteSize(size),
            filesystem: fs,
            label: Some(label.into()),
            drive_letter: None,
        }
    }

    #[test]
    fn adds_a_partition_after_the_one_in_front_of_the_free_space() {
        let id = DiskId::from("/dev/disk4");
        let d = disk(vec![
            partition(&id, 1, MIB, 200 * MIB),
            free(201 * MIB, 822 * MIB),
        ]);
        let args = diskutil_args(
            &d,
            &create(&d, 201 * MIB, 100 * MIB, FileSystem::Apfs, "Data"),
        );
        assert_eq!(
            args.unwrap(),
            ["addPartition", "/dev/disk4s1", "APFS", "Data", "104857600B"]
        );
    }

    #[test]
    fn partitions_an_empty_disk_and_keeps_the_rest_free() {
        let d = disk(vec![free(MIB, 1022 * MIB)]);
        let args = diskutil_args(&d, &create(&d, MIB, 100 * MIB, FileSystem::Fat32, "boot"));
        assert_eq!(
            args.unwrap(),
            [
                "partitionDisk",
                "/dev/disk4",
                "GPT",
                "FAT32",
                "BOOT",
                "104857600B",
                "Free Space",
                "%noformat%",
                "R"
            ]
        );

        let all = diskutil_args(&d, &create(&d, MIB, 1022 * MIB, FileSystem::ExFat, "Data"));
        assert_eq!(
            all.unwrap(),
            ["partitionDisk", "/dev/disk4", "GPT", "ExFAT", "Data", "R"]
        );
    }

    #[test]
    fn cannot_use_free_space_in_front_of_the_first_partition() {
        let id = DiskId::from("/dev/disk4");
        let d = disk(vec![
            free(MIB, 100 * MIB),
            partition(&id, 1, 101 * MIB, 200 * MIB),
        ]);
        let args = diskutil_args(&d, &create(&d, MIB, 50 * MIB, FileSystem::ExFat, "X"));
        assert!(matches!(args, Err(ExecutionError::NotImplemented(_))));
    }

    #[test]
    fn deletes_formats_and_renames_by_device_node() {
        let id = DiskId::from("/dev/disk4");
        let d = disk(vec![partition(&id, 2, MIB, 200 * MIB)]);
        let pid = PartitionId::new(&id, 2);

        let delete = OperationRequest::DeletePartition {
            partition: pid.clone(),
        };
        assert_eq!(
            diskutil_args(&d, &delete).unwrap(),
            ["eraseVolume", "free", "free", "/dev/disk4s2"]
        );

        let format = OperationRequest::FormatPartition {
            partition: pid.clone(),
            filesystem: FileSystem::HfsPlus,
            label: None,
        };
        assert_eq!(
            diskutil_args(&d, &format).unwrap(),
            ["eraseVolume", "JHFS+", "Untitled", "/dev/disk4s2"]
        );

        let rename = OperationRequest::SetLabel {
            partition: pid,
            label: "Backup".into(),
        };
        assert_eq!(
            diskutil_args(&d, &rename).unwrap(),
            ["renameVolume", "/dev/disk4s2", "Backup"]
        );
    }

    #[test]
    fn erases_a_whole_disk() {
        let id = DiskId::from("/dev/disk4");
        let d = disk(vec![partition(&id, 2, MIB, 200 * MIB)]);
        let erase = |fs| OperationRequest::EraseDisk {
            disk: id.clone(),
            filesystem: fs,
            label: Some("usb".into()),
        };
        assert_eq!(
            diskutil_args(&d, &erase(FileSystem::Fat32)).unwrap(),
            ["eraseDisk", "FAT32", "USB", "MBR", "/dev/disk4"]
        );
        assert_eq!(
            diskutil_args(&d, &erase(FileSystem::Apfs)).unwrap(),
            ["eraseDisk", "APFS", "usb", "GPT", "/dev/disk4"]
        );
    }

    #[test]
    fn refuses_file_systems_macos_cannot_create() {
        let d = disk(vec![free(MIB, 1022 * MIB)]);
        let args = diskutil_args(&d, &create(&d, MIB, 100 * MIB, FileSystem::Ntfs, "X"));
        assert!(matches!(args, Err(ExecutionError::NotImplemented(_))));
    }

    #[test]
    fn uses_the_raw_device_node() {
        assert_eq!(raw_device_path("/dev/disk4"), "/dev/rdisk4");
    }

    #[test]
    fn reports_a_denied_authorization_when_no_fd_arrives() {
        use std::os::unix::net::UnixStream;
        let (a, b) = UnixStream::pair().unwrap();
        drop(a);
        let err = receive_fd(&b).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }
}
