//! macOS disk inventory, read from `diskutil` and `ioreg` — **not yet
//! tested on a real Mac**. The parsing below is unit-tested against sample
//! output (and runs on Linux too); the commands themselves only ever ran
//! in CI's macOS build.
//!
//! - `diskutil list -plist physical`: the physical disks and their
//!   partitions (APFS containers and disk images are virtual and left out)
//! - `diskutil info -plist <id>`: bus, model, file system, label, mount point
//! - `ioreg`'s IOMedia objects: each partition's byte offset (`Base`), which
//!   `diskutil` doesn't report but the layout needs
//! - `diskutil info -plist /`: which disk the running system lives on

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use super::{usable_range, InventoryError};
use crate::models::{
    BusType, ByteSize, Disk, DiskId, FileSystem, HealthStatus, MediaType, Partition,
    PartitionFlags, PartitionId, PartitionKind, PartitionTable, Segment,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub(crate) struct DiskutilList {
    pub all_disks_and_partitions: Vec<ListedDisk>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub(crate) struct ListedDisk {
    pub device_identifier: String,
    /// The partition scheme, e.g. `GUID_partition_scheme`.
    pub content: String,
    pub size: u64,
    pub partitions: Vec<ListedPartition>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub(crate) struct ListedPartition {
    pub device_identifier: String,
    /// The partition type, e.g. `EFI` or `Microsoft Basic Data`.
    pub content: String,
    pub size: u64,
}

/// The parts of `diskutil info -plist` MoonDisk uses, for whole disks and
/// partitions alike.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub(crate) struct DiskutilInfo {
    pub device_identifier: String,
    pub bus_protocol: String,
    pub media_name: String,
    pub device_block_size: u32,
    pub solid_state: Option<bool>,
    pub writable_media: Option<bool>,
    #[serde(rename = "SMARTStatus")]
    pub smart_status: String,
    pub filesystem_type: String,
    pub volume_name: String,
    pub mount_point: String,
    pub parent_whole_disk: String,
    #[serde(rename = "APFSPhysicalStores")]
    pub apfs_physical_stores: Vec<ApfsPhysicalStore>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ApfsPhysicalStore {
    #[serde(rename = "APFSPhysicalStore")]
    pub device_identifier: String,
}

pub(crate) fn parse_plist<T: DeserializeOwned>(xml: &[u8]) -> Result<T, InventoryError> {
    plist::from_bytes(xml).map_err(|e| InventoryError::ReadFailed(format!("diskutil: {e}")))
}

/// `disk4` -> (4, None), `disk4s2` -> (4, Some(2)). `None` for anything
/// else, e.g. APFS snapshots (`disk3s1s1`).
pub(crate) fn split_identifier(id: &str) -> Option<(u32, Option<u32>)> {
    let rest = id.strip_prefix("disk")?;
    let mut parts = rest.splitn(2, 's');
    let disk = parts.next()?.parse().ok()?;
    let partition = match parts.next() {
        Some(p) => Some(p.parse().ok()?),
        None => None,
    };
    Some((disk, partition))
}

/// Byte offset of every partition within its disk, keyed by BSD name
/// (`disk4s2`), from `ioreg -a -l -r -c IOMedia`. IOKit stores it as the
/// `Base` property of each IOMedia object; the objects are nested, so the
/// whole tree is walked.
pub(crate) fn media_offsets(ioreg_xml: &[u8]) -> Result<HashMap<String, u64>, InventoryError> {
    let root: plist::Value = plist::from_bytes(ioreg_xml)
        .map_err(|e| InventoryError::ReadFailed(format!("ioreg: {e}")))?;
    let mut offsets = HashMap::new();
    collect_offsets(&root, &mut offsets);
    Ok(offsets)
}

fn collect_offsets(value: &plist::Value, out: &mut HashMap<String, u64>) {
    match value {
        plist::Value::Dictionary(dict) => {
            let name = dict.get("BSD Name").and_then(plist::Value::as_string);
            let base = dict.get("Base").and_then(plist::Value::as_unsigned_integer);
            if let (Some(name), Some(base)) = (name, base) {
                out.insert(name.to_string(), base);
            }
            for child in dict.values() {
                collect_offsets(child, out);
            }
        }
        plist::Value::Array(items) => {
            for child in items {
                collect_offsets(child, out);
            }
        }
        _ => {}
    }
}

/// Where the running system lives, from `diskutil info -plist /`.
#[derive(Debug, Default)]
pub(crate) struct SystemMedia {
    /// Whole disks, e.g. `disk0`.
    pub disks: Vec<String>,
    /// Partitions holding the system, e.g. the APFS physical store `disk0s2`.
    pub partitions: Vec<String>,
}

impl SystemMedia {
    pub fn from_root_info(root: &DiskutilInfo) -> Self {
        // On APFS, `/` is a volume on a synthesized container disk
        // (`disk3`); the physical store names the real partition behind it.
        // Without APFS (HFS+), the parent whole disk is the real disk.
        let mut media = SystemMedia::default();
        for store in &root.apfs_physical_stores {
            if let Some((disk, Some(_))) = split_identifier(&store.device_identifier) {
                media.disks.push(format!("disk{disk}"));
                media.partitions.push(store.device_identifier.clone());
            }
        }
        if !root.parent_whole_disk.is_empty() {
            media.disks.push(root.parent_whole_disk.clone());
        }
        if !root.device_identifier.is_empty() {
            media.partitions.push(root.device_identifier.clone());
        }
        media
    }
}

fn bus_type(protocol: &str) -> BusType {
    match protocol {
        "USB" => BusType::Usb,
        "SATA" => BusType::Sata,
        // Apple Silicon's internal SSD reports "Apple Fabric".
        "PCI-Express" | "PCI" | "NVMe" | "Apple Fabric" => BusType::Nvme,
        "Disk Image" | "Virtual Interface" => BusType::Virtual,
        _ => BusType::Unknown,
    }
}

fn partition_table(scheme: &str) -> PartitionTable {
    match scheme {
        "GUID_partition_scheme" => PartitionTable::Gpt,
        "FDisk_partition_scheme" => PartitionTable::Mbr,
        _ => PartitionTable::None,
    }
}

fn partition_kind(content: &str) -> PartitionKind {
    match content {
        "EFI" => PartitionKind::Efi,
        "Microsoft Reserved" => PartitionKind::MicrosoftReserved,
        "Windows Recovery" | "Apple_Boot" | "Apple_APFS_Recovery" => PartitionKind::Recovery,
        "Microsoft Basic Data"
        | "DOS_FAT_32"
        | "DOS_FAT_16"
        | "DOS_FAT_12"
        | "Windows_FAT_32"
        | "Windows_FAT_16"
        | "Windows_NTFS" => PartitionKind::BasicData,
        "Linux Filesystem" | "Linux" => PartitionKind::LinuxFilesystem,
        "Linux Swap" | "Linux_Swap" => PartitionKind::LinuxSwap,
        _ => PartitionKind::Other,
    }
}

/// macOS names file systems by their bundle (`msdos`, `exfat`, …). A
/// partition macOS can't read has none, so fall back to its type.
fn file_system(filesystem_type: &str, content: &str) -> FileSystem {
    match filesystem_type {
        "msdos" => FileSystem::Fat32,
        "" => match content {
            "Apple_APFS" => FileSystem::Apfs,
            "Apple_HFS" => FileSystem::HfsPlus,
            "Linux Swap" | "Linux_Swap" => FileSystem::LinuxSwap,
            _ => FileSystem::Unknown,
        },
        other => FileSystem::from_lsblk_fstype(other),
    }
}

/// Builds one `Disk` from what `diskutil`/`ioreg` reported about it.
pub(crate) fn build_disk(
    listed: &ListedDisk,
    info: &DiskutilInfo,
    partition_infos: &HashMap<String, DiskutilInfo>,
    offsets: &HashMap<String, u64>,
    system: &SystemMedia,
) -> Result<Disk, InventoryError> {
    let Some((number, None)) = split_identifier(&listed.device_identifier) else {
        return Err(InventoryError::ReadFailed(format!(
            "unexpected disk identifier {}",
            listed.device_identifier
        )));
    };
    let id = DiskId::from(format!("/dev/{}", listed.device_identifier));
    let table = partition_table(&listed.content);
    let no_info = DiskutilInfo::default();

    let mut partitions = Vec::new();
    for p in &listed.partitions {
        let Some((_, Some(part_number))) = split_identifier(&p.device_identifier) else {
            continue;
        };
        let start = *offsets.get(&p.device_identifier).ok_or_else(|| {
            InventoryError::ReadFailed(format!("no offset for {}", p.device_identifier))
        })?;
        let pinfo = partition_infos
            .get(&p.device_identifier)
            .unwrap_or(&no_info);
        let kind = partition_kind(&p.content);

        let mut flags = PartitionFlags::empty();
        if kind == PartitionKind::Efi {
            flags |= PartitionFlags::BOOT;
        }
        if system.partitions.contains(&p.device_identifier) {
            flags |= PartitionFlags::SYSTEM | PartitionFlags::LOCKED;
        }
        if !pinfo.mount_point.is_empty() {
            flags |= PartitionFlags::ACTIVE_MOUNT;
        }

        partitions.push(Partition {
            id: PartitionId::new(&id, part_number),
            number: part_number,
            start: ByteSize(start),
            size: ByteSize(p.size),
            used: None,
            fs: file_system(&pinfo.filesystem_type, &p.content),
            kind,
            label: Some(pinfo.volume_name.clone()).filter(|l| !l.is_empty()),
            flags,
            drive_letter: None,
            mountpoints: Some(pinfo.mount_point.clone())
                .filter(|m| !m.is_empty())
                .into_iter()
                .collect(),
        });
    }
    partitions.sort_by_key(|p| p.start.0);

    let (usable_start, usable_end) = usable_range(listed.size, table);
    let mut layout = Vec::new();
    let mut cursor = usable_start;
    for p in partitions {
        if p.start.0 > cursor {
            layout.push(Segment::Unallocated {
                start: ByteSize(cursor),
                size: ByteSize(p.start.0 - cursor),
            });
        }
        cursor = cursor.max(p.start.0 + p.size.0);
        layout.push(Segment::Partition(p));
    }
    if cursor < usable_end {
        layout.push(Segment::Unallocated {
            start: ByteSize(cursor),
            size: ByteSize(usable_end - cursor),
        });
    }

    Ok(Disk {
        id,
        display_name: format!("Disk {number}"),
        vendor: String::new(),
        model: info.media_name.trim().to_string(),
        serial: None,
        bus: bus_type(&info.bus_protocol),
        media: match info.solid_state {
            Some(true) => MediaType::Ssd,
            _ => MediaType::Unknown,
        },
        size: ByteSize(listed.size),
        logical_sector_size: if info.device_block_size > 0 {
            info.device_block_size
        } else {
            512
        },
        table,
        health: match info.smart_status.as_str() {
            "Verified" => HealthStatus::Ok,
            "Failing" => HealthStatus::Failing,
            _ => HealthStatus::Unknown,
        },
        read_only: info.writable_media == Some(false),
        is_system_disk: system.disks.contains(&listed.device_identifier),
        layout,
    })
}

#[cfg(target_os = "macos")]
pub use provider::MacDiskProvider;

#[cfg(target_os = "macos")]
mod provider {
    use super::*;
    use crate::platform::{DiskInventory, InventorySource};
    use std::process::Command;

    pub struct MacDiskProvider;

    fn run(program: &str, args: &[&str]) -> Result<Vec<u8>, InventoryError> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| InventoryError::ReadFailed(format!("could not start {program}: {e}")))?;
        if !out.status.success() {
            return Err(InventoryError::ReadFailed(format!(
                "{program}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(out.stdout)
    }

    fn info(identifier: &str) -> Result<DiskutilInfo, InventoryError> {
        parse_plist(&run("diskutil", &["info", "-plist", identifier])?)
    }

    impl DiskInventory for MacDiskProvider {
        fn source(&self) -> InventorySource {
            InventorySource::MacOs
        }

        fn list_disks(&self) -> Result<Vec<Disk>, InventoryError> {
            let list: DiskutilList =
                parse_plist(&run("diskutil", &["list", "-plist", "physical"])?)?;
            let offsets = media_offsets(&run("ioreg", &["-a", "-l", "-r", "-c", "IOMedia"])?)?;
            // Only used to label the system disk, so failing to find it
            // shouldn't hide every disk.
            let system = info("/")
                .map(|root| SystemMedia::from_root_info(&root))
                .unwrap_or_default();

            list.all_disks_and_partitions
                .iter()
                .map(|listed| {
                    let disk_info = info(&listed.device_identifier)?;
                    let partition_infos = listed
                        .partitions
                        .iter()
                        .filter_map(|p| {
                            let i = info(&p.device_identifier).ok()?;
                            Some((p.device_identifier.clone(), i))
                        })
                        .collect();
                    build_disk(listed, &disk_info, &partition_infos, &offsets, &system)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1024 * 1024;

    fn plist_doc(body: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">{body}</plist>"#
        )
        .into_bytes()
    }

    const LIST: &str = r#"<dict>
  <key>AllDisks</key><array><string>disk0</string><string>disk4</string></array>
  <key>AllDisksAndPartitions</key>
  <array>
    <dict>
      <key>Content</key><string>GUID_partition_scheme</string>
      <key>DeviceIdentifier</key><string>disk4</string>
      <key>OSInternal</key><false/>
      <key>Partitions</key>
      <array>
        <dict>
          <key>Content</key><string>EFI</string>
          <key>DeviceIdentifier</key><string>disk4s1</string>
          <key>DiskUUID</key><string>D7A4F0C6-0000-0000-0000-000000000000</string>
          <key>Size</key><integer>209715200</integer>
          <key>VolumeName</key><string>EFI</string>
        </dict>
        <dict>
          <key>Content</key><string>Microsoft Basic Data</string>
          <key>DeviceIdentifier</key><string>disk4s2</string>
          <key>MountPoint</key><string>/Volumes/STICK</string>
          <key>Size</key><integer>8589934592</integer>
          <key>VolumeName</key><string>STICK</string>
        </dict>
      </array>
      <key>Size</key><integer>30752636928</integer>
    </dict>
  </array>
  <key>WholeDisks</key><array><string>disk4</string></array>
</dict>"#;

    const DISK_INFO: &str = r#"<dict>
  <key>BusProtocol</key><string>USB</string>
  <key>Content</key><string>GUID_partition_scheme</string>
  <key>DeviceBlockSize</key><integer>512</integer>
  <key>DeviceIdentifier</key><string>disk4</string>
  <key>DeviceNode</key><string>/dev/disk4</string>
  <key>Internal</key><false/>
  <key>MediaName</key><string>SanDisk Ultra </string>
  <key>SMARTStatus</key><string>Not Supported</string>
  <key>Size</key><integer>30752636928</integer>
  <key>WholeDisk</key><true/>
  <key>WritableMedia</key><true/>
</dict>"#;

    const STICK_INFO: &str = r#"<dict>
  <key>Content</key><string>Microsoft Basic Data</string>
  <key>DeviceIdentifier</key><string>disk4s2</string>
  <key>FilesystemName</key><string>ExFAT</string>
  <key>FilesystemType</key><string>exfat</string>
  <key>MountPoint</key><string>/Volumes/STICK</string>
  <key>ParentWholeDisk</key><string>disk4</string>
  <key>VolumeName</key><string>STICK</string>
</dict>"#;

    const IOREG: &str = r#"<array>
  <dict>
    <key>BSD Name</key><string>disk4</string>
    <key>Base</key><integer>0</integer>
    <key>IOObjectClass</key><string>IOMedia</string>
    <key>IORegistryEntryChildren</key>
    <array>
      <dict>
        <key>IOObjectClass</key><string>IOGUIDPartitionScheme</string>
        <key>IORegistryEntryChildren</key>
        <array>
          <dict>
            <key>BSD Name</key><string>disk4s1</string>
            <key>Base</key><integer>20480</integer>
            <key>Content</key><string>C12A7328-F81F-11D2-BA4B-00A0C93EC93B</string>
            <key>Size</key><integer>209715200</integer>
          </dict>
          <dict>
            <key>BSD Name</key><string>disk4s2</string>
            <key>Base</key><integer>210763776</integer>
            <key>Size</key><integer>8589934592</integer>
          </dict>
        </array>
      </dict>
    </array>
    <key>Size</key><integer>30752636928</integer>
  </dict>
</array>"#;

    fn stick() -> Disk {
        let list: DiskutilList = parse_plist(&plist_doc(LIST)).unwrap();
        let info: DiskutilInfo = parse_plist(&plist_doc(DISK_INFO)).unwrap();
        let stick: DiskutilInfo = parse_plist(&plist_doc(STICK_INFO)).unwrap();
        let offsets = media_offsets(&plist_doc(IOREG)).unwrap();
        let partition_infos = HashMap::from([("disk4s2".to_string(), stick)]);
        build_disk(
            &list.all_disks_and_partitions[0],
            &info,
            &partition_infos,
            &offsets,
            &SystemMedia::default(),
        )
        .unwrap()
    }

    #[test]
    fn splits_disk_and_partition_identifiers() {
        assert_eq!(split_identifier("disk4"), Some((4, None)));
        assert_eq!(split_identifier("disk12s3"), Some((12, Some(3))));
        assert_eq!(split_identifier("disk3s1s1"), None);
        assert_eq!(split_identifier("/dev/disk4"), None);
    }

    #[test]
    fn finds_nested_partition_offsets_in_ioreg_output() {
        let offsets = media_offsets(&plist_doc(IOREG)).unwrap();
        assert_eq!(offsets["disk4"], 0);
        assert_eq!(offsets["disk4s1"], 20480);
        assert_eq!(offsets["disk4s2"], 210763776);
    }

    #[test]
    fn builds_a_usb_disk_with_its_partitions() {
        let disk = stick();
        assert_eq!(disk.id.0, "/dev/disk4");
        assert_eq!(disk.display_name, "Disk 4");
        assert_eq!(disk.model, "SanDisk Ultra");
        assert_eq!(disk.bus, BusType::Usb);
        assert_eq!(disk.table, PartitionTable::Gpt);
        assert!(!disk.read_only);
        assert!(!disk.is_system_disk);

        let parts: Vec<_> = disk.partitions().collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].kind, PartitionKind::Efi);
        assert!(parts[0].flags.contains(PartitionFlags::BOOT));
        // No `diskutil info` for the EFI partition: its type is kept, the
        // file system is unknown rather than guessed.
        assert_eq!(parts[0].fs, FileSystem::Unknown);

        let data = parts[1];
        assert_eq!(data.id.0, "/dev/disk4#2");
        assert_eq!(data.number, 2);
        assert_eq!(data.start.0, 210763776);
        assert_eq!(data.fs, FileSystem::ExFat);
        assert_eq!(data.label.as_deref(), Some("STICK"));
        assert_eq!(data.mountpoints, vec!["/Volumes/STICK".to_string()]);
        assert!(data.flags.contains(PartitionFlags::ACTIVE_MOUNT));
    }

    #[test]
    fn fills_the_gaps_with_free_space() {
        let disk = stick();
        let Segment::Unallocated { start, size } = disk.layout.last().unwrap() else {
            panic!("expected trailing free space, got {:?}", disk.layout);
        };
        assert_eq!(start.0, 210763776 + 8589934592);
        // GPT keeps its backup table in the last MiB.
        assert_eq!(start.0 + size.0, 30752636928 - MIB);
    }

    #[test]
    fn finds_the_system_disk_behind_the_apfs_container() {
        let root: DiskutilInfo = parse_plist(&plist_doc(
            r#"<dict>
  <key>APFSContainerReference</key><string>disk3</string>
  <key>APFSPhysicalStores</key>
  <array><dict><key>APFSPhysicalStore</key><string>disk0s2</string></dict></array>
  <key>DeviceIdentifier</key><string>disk3s1s1</string>
  <key>FilesystemType</key><string>apfs</string>
  <key>MountPoint</key><string>/</string>
  <key>ParentWholeDisk</key><string>disk3</string>
</dict>"#,
        ))
        .unwrap();
        let system = SystemMedia::from_root_info(&root);
        assert!(system.disks.contains(&"disk0".to_string()));
        assert!(system.partitions.contains(&"disk0s2".to_string()));
    }

    #[test]
    fn maps_macos_file_system_names() {
        assert_eq!(file_system("msdos", "DOS_FAT_32"), FileSystem::Fat32);
        assert_eq!(file_system("exfat", ""), FileSystem::ExFat);
        assert_eq!(file_system("ntfs", ""), FileSystem::Ntfs);
        assert_eq!(file_system("apfs", ""), FileSystem::Apfs);
        assert_eq!(file_system("hfs", ""), FileSystem::HfsPlus);
        assert_eq!(file_system("", "Apple_APFS"), FileSystem::Apfs);
        assert_eq!(file_system("", "Linux Filesystem"), FileSystem::Unknown);
    }
}
