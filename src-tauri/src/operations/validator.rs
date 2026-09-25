//! Validates an `OperationRequest` against a disk's *current* layout.
//! Rules mirror `docs/supported-operations.md`. This module never touches
//! a disk — it only reads the `Disk` snapshot it's given.

use super::OperationRequest;
use crate::models::{ByteSize, Disk, Segment};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Datenträger ist schreibgeschützt")]
    DiskReadOnly,
    #[error("Partition wurde nicht gefunden")]
    PartitionNotFound,
    #[error("kein ausreichender nicht zugewiesener Speicher an dieser Stelle")]
    NoFreeSpace,
    #[error("Anfang oder Größe ist nicht auf 1 MiB ausgerichtet")]
    Alignment,
    #[error("Größe unterschreitet die Mindestgröße")]
    SizeTooSmall,
    #[error("dieses Dateisystem unterstützt diese Aktion nicht")]
    FilesystemUnsupported,
    #[error("Label ist ungültig oder zu lang")]
    InvalidLabel,
    #[error("Laufwerksbuchstabe muss ein einzelner Buchstabe A-Z sein")]
    InvalidDriveLetter,
}

const ALIGNMENT: u64 = 1024 * 1024; // 1 MiB, see docs/supported-operations.md §4
const MIN_PARTITION_SIZE: u64 = 1024 * 1024;

fn is_aligned(v: ByteSize) -> bool {
    v.0 % ALIGNMENT == 0
}

/// Checks `req` against `disk`, which must be the *current* state of the
/// exact disk the request targets (callers re-fetch this immediately
/// before validating — see `docs/safety-model.md` §5.4 on stale plans).
pub fn validate(disk: &Disk, req: &OperationRequest) -> Result<(), ValidationError> {
    if disk.read_only {
        return Err(ValidationError::DiskReadOnly);
    }

    match req {
        OperationRequest::CreatePartition {
            start,
            size,
            drive_letter,
            ..
        } => {
            if !is_aligned(*start) || !is_aligned(*size) {
                return Err(ValidationError::Alignment);
            }
            if size.0 < MIN_PARTITION_SIZE {
                return Err(ValidationError::SizeTooSmall);
            }
            let end = start
                .checked_add(*size)
                .ok_or(ValidationError::NoFreeSpace)?;
            let fits_in_free_region = disk.layout.iter().any(|seg| match seg {
                Segment::Unallocated {
                    start: fs,
                    size: fsz,
                } => {
                    let free_end = fs.checked_add(*fsz).unwrap_or(ByteSize::ZERO);
                    *start >= *fs && end <= free_end
                }
                Segment::Partition(_) => false,
            });
            if !fits_in_free_region {
                return Err(ValidationError::NoFreeSpace);
            }
            if let Some(l) = drive_letter {
                if !l.is_ascii_alphabetic() {
                    return Err(ValidationError::InvalidDriveLetter);
                }
            }
            Ok(())
        }
        OperationRequest::DeletePartition { partition } => {
            find_partition(disk, partition)?;
            Ok(())
        }
        OperationRequest::FormatPartition {
            partition,
            filesystem,
            label,
        } => {
            find_partition(disk, partition)?;
            if !filesystem.can_format() {
                return Err(ValidationError::FilesystemUnsupported);
            }
            if let Some(l) = label {
                validate_label(l)?;
            }
            Ok(())
        }
        OperationRequest::SetLabel { partition, label } => {
            find_partition(disk, partition)?;
            validate_label(label)
        }
        OperationRequest::SetDriveLetter {
            partition,
            drive_letter,
        } => {
            find_partition(disk, partition)?;
            if !drive_letter.is_ascii_alphabetic() {
                return Err(ValidationError::InvalidDriveLetter);
            }
            Ok(())
        }
    }
}

fn find_partition<'a>(
    disk: &'a Disk,
    id: &crate::models::PartitionId,
) -> Result<&'a crate::models::Partition, ValidationError> {
    disk.partitions()
        .find(|p| &p.id == id)
        .ok_or(ValidationError::PartitionNotFound)
}

fn validate_label(label: &str) -> Result<(), ValidationError> {
    if label.is_empty() || label.chars().count() > 32 || label.chars().any(|c| c.is_control()) {
        return Err(ValidationError::InvalidLabel);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn sample_disk() -> Disk {
        let id = DiskId::from("test-disk");
        let locked = Partition {
            id: PartitionId::new(&id, 1),
            number: 1,
            start: ByteSize(ALIGNMENT),
            size: ByteSize(ALIGNMENT * 100),
            used: None,
            fs: FileSystem::Ext4,
            kind: PartitionKind::LinuxFilesystem,
            label: Some("root".into()),
            flags: PartitionFlags::SYSTEM | PartitionFlags::LOCKED,
            drive_letter: None,
            mountpoints: vec!["/".into()],
        };
        let free_start = locked.end().unwrap();
        Disk {
            id,
            display_name: "Test".into(),
            vendor: "Test".into(),
            model: "Test".into(),
            serial: None,
            bus: BusType::Virtual,
            media: MediaType::Unknown,
            size: ByteSize(ALIGNMENT * 200),
            logical_sector_size: 512,
            table: PartitionTable::Gpt,
            health: HealthStatus::Ok,
            read_only: false,
            is_system_disk: false,
            layout: vec![
                Segment::Unallocated {
                    start: ByteSize::ZERO,
                    size: ByteSize(ALIGNMENT),
                },
                Segment::Partition(locked),
                Segment::Unallocated {
                    start: free_start,
                    size: ByteSize(ALIGNMENT * 200).checked_sub(free_start).unwrap(),
                },
            ],
        }
    }

    #[test]
    fn allows_deleting_a_system_flagged_partition_on_the_system_disk() {
        // System/boot/EFI partition protection was removed at the user's
        // explicit request; `is_system_disk` and `PartitionFlags::SYSTEM |
        // LOCKED` remain informational only from here on.
        let mut disk = sample_disk();
        disk.is_system_disk = true;
        let req = OperationRequest::DeletePartition {
            partition: PartitionId::new(&disk.id, 1),
        };
        assert_eq!(validate(&disk, &req), Ok(()));
    }

    #[test]
    fn accepts_a_valid_drive_letter() {
        let disk = sample_disk();
        let req = OperationRequest::SetDriveLetter {
            partition: PartitionId::new(&disk.id, 1),
            drive_letter: 'D',
        };
        assert_eq!(validate(&disk, &req), Ok(()));
    }

    #[test]
    fn rejects_a_non_letter_drive_letter() {
        let disk = sample_disk();
        let req = OperationRequest::SetDriveLetter {
            partition: PartitionId::new(&disk.id, 1),
            drive_letter: '5',
        };
        assert_eq!(
            validate(&disk, &req),
            Err(ValidationError::InvalidDriveLetter)
        );
    }

    #[test]
    fn accepts_create_that_fits_in_free_space() {
        let disk = sample_disk();
        let req = OperationRequest::CreatePartition {
            disk: disk.id.clone(),
            start: ByteSize(ALIGNMENT * 101),
            size: ByteSize(ALIGNMENT * 10),
            filesystem: FileSystem::Ext4,
            label: None,
            drive_letter: None,
        };
        assert_eq!(validate(&disk, &req), Ok(()));
    }

    #[test]
    fn rejects_create_that_overlaps_the_locked_partition() {
        let disk = sample_disk();
        let req = OperationRequest::CreatePartition {
            disk: disk.id.clone(),
            start: ByteSize(ALIGNMENT * 50),
            size: ByteSize(ALIGNMENT * 10),
            filesystem: FileSystem::Ext4,
            label: None,
            drive_letter: None,
        };
        assert_eq!(validate(&disk, &req), Err(ValidationError::NoFreeSpace));
    }

    #[test]
    fn rejects_misaligned_create() {
        let disk = sample_disk();
        let req = OperationRequest::CreatePartition {
            disk: disk.id.clone(),
            start: ByteSize(ALIGNMENT * 101 + 7),
            size: ByteSize(ALIGNMENT * 10),
            filesystem: FileSystem::Ext4,
            label: None,
            drive_letter: None,
        };
        assert_eq!(validate(&disk, &req), Err(ValidationError::Alignment));
    }

    #[test]
    fn rejects_create_with_a_non_letter_drive_letter() {
        let disk = sample_disk();
        let req = OperationRequest::CreatePartition {
            disk: disk.id.clone(),
            start: ByteSize(ALIGNMENT * 101),
            size: ByteSize(ALIGNMENT * 10),
            filesystem: FileSystem::Ext4,
            label: None,
            drive_letter: Some('5'),
        };
        assert_eq!(
            validate(&disk, &req),
            Err(ValidationError::InvalidDriveLetter)
        );
    }

    #[test]
    fn rejects_format_with_unsupported_filesystem() {
        let disk = sample_disk();
        // Create an unlocked partition to try formatting.
        let mut disk = disk;
        disk.layout.push(Segment::Partition(Partition {
            id: PartitionId::new(&disk.id, 2),
            number: 2,
            start: ByteSize(ALIGNMENT * 101),
            size: ByteSize(ALIGNMENT * 10),
            used: None,
            fs: FileSystem::Unknown,
            kind: PartitionKind::Other,
            label: None,
            flags: PartitionFlags::empty(),
            drive_letter: None,
            mountpoints: vec![],
        }));
        let req = OperationRequest::FormatPartition {
            partition: PartitionId::new(&disk.id, 2),
            filesystem: FileSystem::Unknown,
            label: None,
        };
        assert_eq!(
            validate(&disk, &req),
            Err(ValidationError::FilesystemUnsupported)
        );
    }

    #[test]
    fn rejects_empty_label() {
        let disk = sample_disk();
        let mut disk = disk;
        disk.layout.push(Segment::Partition(Partition {
            id: PartitionId::new(&disk.id, 2),
            number: 2,
            start: ByteSize(ALIGNMENT * 101),
            size: ByteSize(ALIGNMENT * 10),
            used: None,
            fs: FileSystem::Ext4,
            kind: PartitionKind::LinuxFilesystem,
            label: None,
            flags: PartitionFlags::empty(),
            drive_letter: None,
            mountpoints: vec![],
        }));
        let req = OperationRequest::SetLabel {
            partition: PartitionId::new(&disk.id, 2),
            label: "".into(),
        };
        assert_eq!(validate(&disk, &req), Err(ValidationError::InvalidLabel));
    }
}
