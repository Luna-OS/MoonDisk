//! The JSON the helper sends the Mac app, pinned in
//! `tests/fixtures/helper-messages.json`. The Swift tests
//! (macos/Tests/MoonDiskTests) decode the same file, so a change on either
//! side that breaks the other fails a test instead of the app.
//!
//! After an intended change: `UPDATE_FIXTURES=1 cargo test --test helper_contract`.

use moondisk_lib::flash::copy::CopyModeInfo;
use moondisk_lib::flash::{ImageInfo, Phase};
use moondisk_lib::models::{
    BusType, ByteSize, Disk, DiskId, FileSystem, HealthStatus, MediaType, Partition,
    PartitionFlags, PartitionId, PartitionKind, PartitionTable, Segment,
};
use moondisk_lib::service::FlashProgress;
use serde_json::{json, Value};
use std::path::PathBuf;

const MIB: u64 = 1024 * 1024;

/// A plain partition; the tests set label, flags and mount points with
/// struct update syntax.
fn partition(disk: &DiskId, number: u32, start: u64, size: u64, fs: FileSystem) -> Partition {
    Partition {
        id: PartitionId::new(disk, number),
        number,
        start: ByteSize(start),
        size: ByteSize(size),
        used: None,
        fs,
        kind: PartitionKind::Other,
        label: None,
        flags: PartitionFlags::empty(),
        drive_letter: None,
        mountpoints: Vec::new(),
    }
}

/// An internal disk and a USB stick an Arch ISO was written to.
fn disks() -> Vec<Disk> {
    let internal = DiskId::from("disk0");
    let usb = DiskId::from("disk4");
    vec![
        Disk {
            id: internal.clone(),
            display_name: "disk0".into(),
            vendor: "Apple".into(),
            model: "APPLE SSD AP0512Z".into(),
            serial: Some("0ba0123456789".into()),
            bus: BusType::Nvme,
            media: MediaType::Ssd,
            size: ByteSize(500_277_790_720),
            logical_sector_size: 4096,
            table: PartitionTable::Gpt,
            health: HealthStatus::Ok,
            read_only: false,
            is_system_disk: true,
            layout: vec![
                Segment::Partition(Partition {
                    kind: PartitionKind::Efi,
                    label: Some("EFI".into()),
                    flags: PartitionFlags::BOOT | PartitionFlags::SYSTEM,
                    ..partition(&internal, 1, 24_576, 524_288_000, FileSystem::Fat32)
                }),
                Segment::Partition(Partition {
                    flags: PartitionFlags::SYSTEM | PartitionFlags::ACTIVE_MOUNT,
                    mountpoints: vec!["/".into(), "/System/Volumes/Data".into()],
                    ..partition(&internal, 2, 524_312_576, 494_384_795_648, FileSystem::Apfs)
                }),
                Segment::Partition(Partition {
                    kind: PartitionKind::Recovery,
                    ..partition(
                        &internal,
                        3,
                        494_909_108_224,
                        5_368_664_064,
                        FileSystem::Apfs,
                    )
                }),
            ],
        },
        Disk {
            id: usb.clone(),
            display_name: "disk4".into(),
            vendor: "SanDisk".into(),
            model: "Ultra".into(),
            serial: None,
            bus: BusType::Usb,
            media: MediaType::Unknown,
            size: ByteSize(123_060_879_360),
            logical_sector_size: 512,
            table: PartitionTable::Mbr,
            health: HealthStatus::Unknown,
            read_only: false,
            is_system_disk: false,
            layout: vec![
                Segment::Unallocated {
                    start: ByteSize(0),
                    size: ByteSize(MIB),
                },
                Segment::Partition(Partition {
                    kind: PartitionKind::Efi,
                    label: Some("ARCHISO_EFI".into()),
                    flags: PartitionFlags::BOOT,
                    mountpoints: vec!["/Volumes/ARCHISO_EFI".into()],
                    ..partition(&usb, 2, MIB, 24 * MIB, FileSystem::Fat32)
                }),
                Segment::Unallocated {
                    start: ByteSize(25 * MIB),
                    size: ByteSize(123_060_879_360 - 25 * MIB),
                },
            ],
        },
    ]
}

fn messages() -> Value {
    let image = ImageInfo {
        path: "/Users/luna/Downloads/archlinux-2024.09.01-x86_64.iso".into(),
        name: "archlinux-2024.09.01-x86_64.iso".into(),
        size: ByteSize(1_185_939_456),
        has_boot_sector: true,
        copy_mode: CopyModeInfo {
            supported: true,
            reason: None,
            label: Some("ARCH_202409".into()),
        },
    };
    let windows = ImageInfo {
        path: "/Users/luna/Downloads/Win11.iso".into(),
        name: "Win11.iso".into(),
        size: ByteSize(5_819_484_160),
        has_boot_sector: false,
        copy_mode: CopyModeInfo {
            supported: false,
            reason: Some("This is a Windows (UDF) image.".into()),
            label: None,
        },
    };
    let progress = FlashProgress {
        phase: Phase::Writing,
        done: ByteSize(536_870_912),
        total: ByteSize(1_185_939_456),
        bytes_per_second: 31_457_280.0,
    };
    json!({
        "listDisks": disks(),
        "imageInfo": [image, windows],
        "flashProgress": progress,
    })
}

#[test]
fn helper_messages_match_the_fixture() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/helper-messages.json");
    let actual = messages();
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        let mut text = serde_json::to_string_pretty(&actual).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        actual, expected,
        "the helper's JSON changed — make sure the Mac app still reads it, then run \
         UPDATE_FIXTURES=1 cargo test --test helper_contract"
    );
}

#[test]
fn flags_are_numbers() {
    let json = messages();
    let flags = &json["listDisks"][0]["layout"][0]["value"]["flags"];
    assert_eq!(flags, &json!(3));
}
