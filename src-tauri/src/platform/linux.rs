//! Real Linux disk provider. Reads real disk/partition information via
//! `lsblk`. This is the only place in the crate allowed to run `lsblk` —
//! always with a fixed argument list, never with anything derived from
//! user input, and never through a shell (`std::process::Command` execs
//! the binary directly, so there is no shell to inject into).

use super::{DiskInventory, InventoryError, InventorySource};
use crate::models::*;
use crate::security::system_protection::system_disk_source_linux;
use serde::Deserialize;
use std::process::Command;

pub struct LinuxDiskProvider;

/// GPT reserves a secondary header + partition entry array at the very
/// end of the disk (33 sectors — well under 1 MiB even on 4Kn drives)
/// that no partition may occupy; `lsblk`'s reported device size doesn't
/// exclude it. Windows' `New-Partition` rejects a request that reaches
/// into this region outright (a real report caught that, see
/// `platform::windows::GPT_BACKUP_RESERVE`) and `parted` on Linux is
/// subject to the same on-disk constraint, so the trailing free-space
/// segment is capped here too, for the same reason and by the same
/// already-MiB-aligned amount.
const GPT_BACKUP_RESERVE: u64 = 1024 * 1024;

// --- `lsblk -J -b -O` JSON shape (only the fields MoonDisk uses) ---

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize, Default)]
struct LsblkDevice {
    name: String,
    path: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(rename = "type")]
    dev_type: String,
    #[serde(default)]
    ro: bool,
    #[serde(default)]
    rm: bool,
    tran: Option<String>,
    rota: Option<bool>,
    model: Option<String>,
    vendor: Option<String>,
    serial: Option<String>,
    #[serde(rename = "pttype")]
    pttype: Option<String>,
    fstype: Option<String>,
    label: Option<String>,
    #[serde(rename = "parttype")]
    parttype: Option<String>,
    mountpoints: Option<Vec<Option<String>>>,
    #[serde(rename = "phy-sec")]
    phy_sec: Option<u32>,
    children: Option<Vec<LsblkDevice>>,
}

impl DiskInventory for LinuxDiskProvider {
    fn source(&self) -> InventorySource {
        InventorySource::Linux
    }

    fn list_disks(&self) -> Result<Vec<Disk>, InventoryError> {
        let output = Command::new("lsblk")
            .args(["--json", "--bytes", "--output-all", "--paths"])
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| {
                InventoryError::ReadFailed(format!("lsblk konnte nicht gestartet werden: {e}"))
            })?;

        if !output.status.success() {
            return Err(InventoryError::ReadFailed(format!(
                "lsblk endete mit Status {:?}",
                output.status.code()
            )));
        }

        let parsed: LsblkOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| InventoryError::ReadFailed(format!("lsblk-Ausgabe ungültig: {e}")))?;

        let system_source = system_disk_source_linux();

        let disks = parsed
            .blockdevices
            .into_iter()
            // "loop" devices (backing files attached via losetup) are real,
            // partitionable virtual disks — e.g. a mounted disk image —
            // and are included deliberately. Zero-size pseudo-disks
            // (zram, an unattached loop device) are not real storage and
            // are filtered out.
            .filter(|d| (d.dev_type == "disk" || d.dev_type == "loop") && d.size > 0)
            .enumerate()
            .map(|(i, dev)| to_disk(dev, i, system_source.as_deref()))
            .collect();
        Ok(disks)
    }
}

fn to_disk(dev: LsblkDevice, index: usize, system_source: Option<&str>) -> Disk {
    let path = dev
        .path
        .clone()
        .unwrap_or_else(|| format!("/dev/{}", dev.name));
    let id = DiskId::from(path.clone());

    let bus = match dev.tran.as_deref() {
        Some("nvme") => BusType::Nvme,
        Some("sata") | Some("ata") => BusType::Sata,
        Some("usb") => BusType::Usb,
        // virtio-blk (common for VMs and, notably, this project's own CI
        // and dev-sandbox disks) has no better-known transport name.
        Some("virtio") => BusType::Virtual,
        Some(_) => BusType::Unknown,
        None if dev.dev_type == "loop" || basename(&dev.name).starts_with("vd") => BusType::Virtual,
        // `tran` is frequently empty for USB card readers and some older
        // controllers; the kernel's "removable" flag is a reasonable
        // fallback signal in that case.
        None if dev.rm => BusType::Usb,
        None => BusType::Unknown,
    };
    let media = match dev.rota {
        Some(true) => MediaType::Hdd,
        Some(false) => MediaType::Ssd,
        None => MediaType::Unknown,
    };
    let table = match dev.pttype.as_deref() {
        Some("gpt") => PartitionTable::Gpt,
        Some("dos") => PartitionTable::Mbr,
        _ => PartitionTable::None,
    };

    let is_system_disk = system_source.is_some_and(|src| src.starts_with(&path));

    let children = dev.children.unwrap_or_default();
    let mut layout = Vec::new();
    let mut cursor: u64 = 0;
    for (i, child) in children.iter().enumerate() {
        // lsblk doesn't report each partition's start offset directly in a
        // portable way across util-linux versions; MoonDisk instead lays
        // partitions out back-to-back in reported order and represents any
        // remaining disk size as trailing unallocated space. Exact start
        // offsets (needed for real resize/move math) are read from
        // `/sys/class/block/<part>/start` where available.
        let start = read_sysfs_start(&child.name).unwrap_or(cursor);
        let size = child.size;
        if start > cursor {
            layout.push(Segment::Unallocated {
                start: ByteSize(cursor),
                size: ByteSize(start - cursor),
            });
        }
        let number = extract_partition_number(&child.name, &dev.name).unwrap_or((i + 1) as u32);
        let partition_id = PartitionId::new(&id, number);
        // Queried directly via `blkid` rather than trusted from lsblk's own
        // FSTYPE/LABEL columns: those are sourced from the udev database on
        // this util-linux version, so they only reflect a filesystem that
        // has already been through a udev "change" event. `blkid` probes
        // the device itself and is correct immediately after a MoonDisk
        // format/label operation, with no dependency on udev being
        // present or having caught up — this was caught by the real
        // create→format→relabel→delete integration test, not assumed.
        let (blkid_fstype, blkid_label) = blkid_probe(&child_path(&child.name));
        let fs = FileSystem::from_lsblk_fstype(
            blkid_fstype
                .as_deref()
                .or(child.fstype.as_deref())
                .unwrap_or(""),
        );
        let label = blkid_label.or_else(|| child.label.clone());
        let kind = partition_kind(child.parttype.as_deref(), fs);
        let mountpoints: Vec<String> = child
            .mountpoints
            .clone()
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .collect();
        let mut flags = PartitionFlags::empty();
        if !mountpoints.is_empty() {
            flags |= PartitionFlags::ACTIVE_MOUNT;
        }
        if is_system_disk
            && system_source
                .map(|src| src.starts_with(&child_path(&child.name)))
                .unwrap_or(false)
        {
            flags |= PartitionFlags::SYSTEM | PartitionFlags::BOOT | PartitionFlags::LOCKED;
        }
        layout.push(Segment::Partition(Partition {
            id: partition_id,
            number,
            start: ByteSize(start),
            size: ByteSize(size),
            used: None, // requires statfs on the mountpoint; left for a later pass
            fs,
            kind,
            label,
            flags,
            drive_letter: None,
            mountpoints,
        }));
        cursor = start + size;
    }
    let usable_end = if table == PartitionTable::Gpt {
        dev.size.saturating_sub(GPT_BACKUP_RESERVE)
    } else {
        dev.size
    };
    if cursor < usable_end {
        layout.push(Segment::Unallocated {
            start: ByteSize(cursor),
            size: ByteSize(usable_end - cursor),
        });
    }

    Disk {
        id,
        display_name: format!("Datenträger {index}"),
        vendor: clean_vendor_model(dev.vendor),
        model: clean_vendor_model(dev.model),
        serial: dev.serial,
        bus,
        media,
        size: ByteSize(dev.size),
        logical_sector_size: dev.phy_sec.unwrap_or(512),
        table,
        health: HealthStatus::Unknown, // real S.M.A.R.T. reading is a later pass
        read_only: dev.ro,
        is_system_disk,
        layout,
    }
}

/// virtio-blk devices report their vendor/model as a raw PCI vendor ID
/// (e.g. `"0x1af4"`) rather than a human-readable string; showing that
/// verbatim in the UI is more confusing than showing nothing, so it's
/// filtered out here and left for `Disk::model.or(vendor).unwrap_or("unbekannt")`-style
/// fallbacks in the UI to handle.
fn clean_vendor_model(raw: Option<String>) -> String {
    let value = raw.unwrap_or_default().trim().to_string();
    if value.starts_with("0x") {
        String::new()
    } else {
        value
    }
}

fn child_path(name: &str) -> String {
    if name.starts_with('/') {
        name.to_string()
    } else {
        format!("/dev/{name}")
    }
}

fn partition_kind(parttype_guid: Option<&str>, fs: FileSystem) -> PartitionKind {
    match parttype_guid.map(|s| s.to_ascii_lowercase()) {
        Some(ref g) if g == "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" => PartitionKind::Efi,
        Some(ref g) if g == "e3c9e316-0b5c-4db8-817d-f92df00215ae" => {
            PartitionKind::MicrosoftReserved
        }
        Some(ref g) if g == "de94bba4-06d1-4d40-a16a-bfd50179d6ac" => PartitionKind::Recovery,
        _ if fs == FileSystem::LinuxSwap => PartitionKind::LinuxSwap,
        _ if matches!(
            fs,
            FileSystem::Ext2
                | FileSystem::Ext3
                | FileSystem::Ext4
                | FileSystem::Btrfs
                | FileSystem::Xfs
        ) =>
        {
            PartitionKind::LinuxFilesystem
        }
        _ => PartitionKind::Other,
    }
}

/// Extracts the trailing partition number from a device name like
/// `/dev/sda1` -> 1, `/dev/nvme0n1p2` -> 2.
fn extract_partition_number(child_name: &str, _parent_name: &str) -> Option<u32> {
    let digits: String = child_name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.chars().rev().collect::<String>().parse().ok()
}

/// `lsblk --paths` puts the *full* `/dev/...` path in every `name` field
/// (this module always calls it with `--paths` — see `list_disks`), so
/// anything that needs the bare kernel device name (sysfs lookups, the
/// `vd`-prefix heuristic above) has to strip that back off first.
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Live-probes a partition's filesystem type and label via `blkid`
/// (fixed args, read-only). Returns `(None, None)` for an unformatted
/// partition or if `blkid` isn't available — callers fall back to
/// whatever lsblk itself reported in that case.
fn blkid_probe(partition_path: &str) -> (Option<String>, Option<String>) {
    let output = Command::new("blkid")
        .args(["-o", "export", partition_path])
        .env("LC_ALL", "C")
        .output();
    let Ok(output) = output else {
        return (None, None);
    };
    if !output.status.success() {
        // Exit status 2 means "no recognizable filesystem" — expected and
        // not an error condition, not logged as one.
        return (None, None);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fstype = None;
    let mut label = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("TYPE=") {
            fstype = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("LABEL=") {
            label = Some(v.to_string());
        }
    }
    (fstype, label)
}

fn read_sysfs_start(dev_name: &str) -> Option<u64> {
    let path = format!("/sys/class/block/{}/start", basename(dev_name));
    let raw = std::fs::read_to_string(path).ok()?;
    // sysfs reports the start in 512-byte sectors regardless of the
    // device's logical sector size.
    raw.trim().parse::<u64>().ok().map(|sectors| sectors * 512)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_partition_number_from_common_naming_schemes() {
        assert_eq!(extract_partition_number("sda1", "sda"), Some(1));
        assert_eq!(extract_partition_number("nvme0n1p2", "nvme0n1"), Some(2));
        assert_eq!(extract_partition_number("vda3", "vda"), Some(3));
    }

    #[test]
    fn caps_trailing_free_space_below_the_gpt_backup_table_on_gpt_disks() {
        let dev = LsblkDevice {
            name: "sdz".into(),
            path: Some("/dev/sdz".into()),
            size: 100 * 1024 * 1024,
            dev_type: "disk".into(),
            pttype: Some("gpt".into()),
            children: Some(vec![LsblkDevice {
                name: "sdz1".into(),
                path: Some("/dev/sdz1".into()),
                size: 50 * 1024 * 1024,
                dev_type: "part".into(),
                ..Default::default()
            }]),
            ..Default::default()
        };
        let disk = to_disk(dev, 0, None);
        let trailing = disk.layout.last().expect("expected a trailing segment");
        let Segment::Unallocated { start, size } = trailing else {
            panic!("expected the last segment to be unallocated, got {trailing:?}");
        };
        let end = start.0 + size.0;
        assert!(
            end <= 100 * 1024 * 1024 - GPT_BACKUP_RESERVE,
            "trailing free space (ending at {end}) must not reach into the GPT backup table \
             region — a real New-Partition/parted call there fails"
        );
    }

    // Not run in normal `cargo test` (real hardware varies run to run and
    // this is a read-only sanity check, not a behavioral assertion) — run
    // manually with `cargo test -- --ignored linux::tests::manual` while
    // developing the parser against real lsblk output.
    #[test]
    #[ignore]
    fn manual_list_real_disks() {
        let disks = LinuxDiskProvider
            .list_disks()
            .expect("lsblk should succeed");
        for disk in &disks {
            println!(
                "{} {} {} {:?} {:?} {} bytes, table={:?}",
                disk.id, disk.vendor, disk.model, disk.bus, disk.media, disk.size.0, disk.table
            );
            for seg in &disk.layout {
                println!("  {:?}", seg);
            }
        }
        assert!(
            !disks.is_empty(),
            "expected at least one real disk in this sandbox"
        );
    }
}
