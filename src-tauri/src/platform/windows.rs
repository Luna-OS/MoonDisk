//! Real Windows disk provider.
//!
//! **Untested.** This module was written without access to a Windows
//! machine (this project's dev sandbox is Linux-only) — it follows the
//! same structured-command principles as `platform::linux` (fixed,
//! embedded PowerShell Storage-module scripts, parameters passed as
//! separate, typed script arguments rather than concatenated into script
//! text — see the `.ps1` files in `platform/windows_scripts/`), and it
//! compiles under `#[cfg(target_os = "windows")]`, but it has not been
//! exercised against a real Windows disk. Treat it as a reviewed draft,
//! not a verified implementation, until it has been run on real Windows
//! hardware or a Windows CI runner and someone updates this comment.

use super::{run_powershell_script, DiskInventory, InventoryError, InventorySource};
use crate::models::*;
use serde::Deserialize;

pub struct WindowsDiskProvider;

const LIST_DISKS_SCRIPT: &str = include_str!("windows_scripts/list_disks.ps1");

#[derive(Debug, Deserialize)]
struct PsDisk {
    #[serde(rename = "Number")]
    number: i32,
    #[serde(rename = "FriendlyName")]
    friendly_name: Option<String>,
    #[serde(rename = "Manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "Model")]
    model: Option<String>,
    #[serde(rename = "SerialNumber")]
    serial_number: Option<String>,
    #[serde(rename = "BusType")]
    bus_type: Option<String>,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "LogicalSectorSize")]
    logical_sector_size: Option<u32>,
    #[serde(rename = "PartitionStyle")]
    partition_style: Option<String>,
    #[serde(rename = "HealthStatus")]
    health_status: Option<String>,
    #[serde(rename = "IsReadOnly")]
    is_read_only: bool,
    #[serde(rename = "IsBoot")]
    is_boot: bool,
    #[serde(rename = "IsSystem")]
    is_system: bool,
    #[serde(rename = "Partitions")]
    partitions: Vec<PsPartition>,
}

#[derive(Debug, Deserialize)]
struct PsPartition {
    #[serde(rename = "PartitionNumber")]
    partition_number: u32,
    #[serde(rename = "Offset")]
    offset: u64,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "DriveLetter")]
    drive_letter: Option<String>,
    #[serde(rename = "GptType")]
    gpt_type: Option<String>,
    #[serde(rename = "IsBoot")]
    is_boot: bool,
    #[serde(rename = "IsSystem")]
    is_system: bool,
    #[serde(rename = "FileSystem")]
    file_system: Option<String>,
    #[serde(rename = "FileSystemLabel")]
    file_system_label: Option<String>,
}

impl DiskInventory for WindowsDiskProvider {
    fn source(&self) -> InventorySource {
        InventorySource::Windows
    }

    fn list_disks(&self) -> Result<Vec<Disk>, InventoryError> {
        let stdout = run_powershell_script(LIST_DISKS_SCRIPT, &[])
            .map_err(|e| InventoryError::ReadFailed(e.to_string()))?;

        // Get-Disk with a single disk returns a JSON object, not an array;
        // the script wraps every result in `@(...)` specifically so this
        // is always an array, but an empty machine still round-trips
        // through ConvertTo-Json as the literal text "null".
        let trimmed = stdout.trim();
        if trimmed.is_empty() || trimmed == "null" {
            return Ok(Vec::new());
        }
        let parsed: Vec<PsDisk> = serde_json::from_str(trimmed)
            .map_err(|e| InventoryError::ReadFailed(format!("PowerShell-Ausgabe ungültig: {e}")))?;

        Ok(parsed.into_iter().map(to_disk).collect())
    }
}

fn to_disk(d: PsDisk) -> Disk {
    let id = DiskId::from(format!("\\\\.\\PhysicalDrive{}", d.number));
    let bus = match d.bus_type.as_deref() {
        Some("NVMe") => BusType::Nvme,
        Some("SATA") | Some("ATA") | Some("RAID") => BusType::Sata,
        Some("USB") => BusType::Usb,
        Some("Virtual") | Some("File Backed Virtual") => BusType::Virtual,
        _ => BusType::Unknown,
    };
    let table = match d.partition_style.as_deref() {
        Some("GPT") => PartitionTable::Gpt,
        Some("MBR") => PartitionTable::Mbr,
        _ => PartitionTable::None,
    };
    let health = match d.health_status.as_deref() {
        Some("Healthy") => HealthStatus::Ok,
        Some("Warning") => HealthStatus::Warning,
        Some("Unhealthy") | Some("Unusable") => HealthStatus::Failing,
        _ => HealthStatus::Unknown,
    };

    let mut layout = Vec::new();
    let mut cursor: u64 = 0;
    for p in &d.partitions {
        if p.offset > cursor {
            layout.push(Segment::Unallocated {
                start: ByteSize(cursor),
                size: ByteSize(p.offset - cursor),
            });
        }
        let mut flags = PartitionFlags::empty();
        if p.is_boot {
            flags |= PartitionFlags::BOOT;
        }
        if p.is_system {
            flags |= PartitionFlags::SYSTEM | PartitionFlags::LOCKED;
        }
        if p.drive_letter.is_some() {
            flags |= PartitionFlags::ACTIVE_MOUNT;
        }
        let fs = FileSystem::from_lsblk_fstype(
            &p.file_system
                .clone()
                .unwrap_or_default()
                .to_ascii_lowercase(),
        );
        let kind = match p.gpt_type.as_deref() {
            Some(t) if t.eq_ignore_ascii_case("{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}") => {
                PartitionKind::Efi
            }
            Some(t) if t.eq_ignore_ascii_case("{e3c9e316-0b5c-4db8-817d-f92df00215ae}") => {
                PartitionKind::MicrosoftReserved
            }
            Some(t) if t.eq_ignore_ascii_case("{de94bba4-06d1-4d40-a16a-bfd50179d6ac}") => {
                PartitionKind::Recovery
            }
            _ => PartitionKind::BasicData,
        };
        layout.push(Segment::Partition(Partition {
            id: PartitionId::new(&id, p.partition_number),
            number: p.partition_number,
            start: ByteSize(p.offset),
            size: ByteSize(p.size),
            used: None,
            fs,
            kind,
            label: p.file_system_label.clone().filter(|l| !l.is_empty()),
            flags,
            drive_letter: p.drive_letter.as_ref().and_then(|s| s.chars().next()),
            mountpoints: vec![],
        }));
        cursor = p.offset + p.size;
    }
    if cursor < d.size {
        layout.push(Segment::Unallocated {
            start: ByteSize(cursor),
            size: ByteSize(d.size - cursor),
        });
    }

    Disk {
        id,
        display_name: format!("Datenträger {}", d.number),
        vendor: d.manufacturer.unwrap_or_default(),
        model: d.model.or(d.friendly_name).unwrap_or_default(),
        serial: d.serial_number,
        bus,
        media: MediaType::Unknown, // Get-PhysicalDisk has MediaType but isn't joined in here yet
        size: ByteSize(d.size),
        logical_sector_size: d.logical_sector_size.unwrap_or(512),
        table,
        health,
        read_only: d.is_read_only,
        is_system_disk: d.is_boot || d.is_system,
        layout,
    }
}
