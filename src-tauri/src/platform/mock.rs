//! Mock disk provider: realistic, deterministic sample data for UI
//! development, demos, and tests. See `docs/safety-model.md` §4 for the
//! original scenario design this is based on.

use super::{DiskInventory, InventoryError, InventorySource};
use crate::models::*;

pub struct MockDiskProvider;

impl DiskInventory for MockDiskProvider {
    fn source(&self) -> InventorySource {
        InventorySource::Mock
    }

    fn list_disks(&self) -> Result<Vec<Disk>, InventoryError> {
        Ok(vec![
            windows_system_disk(),
            linux_system_disk(),
            usb_stick(),
        ])
    }
}

fn gib(n: u64) -> ByteSize {
    ByteSize(n * 1024 * 1024 * 1024)
}
fn mib(n: u64) -> ByteSize {
    ByteSize(n * 1024 * 1024)
}

fn windows_system_disk() -> Disk {
    let id = DiskId::from("mock-disk-0");
    let efi = Partition {
        id: PartitionId::new(&id, 1),
        number: 1,
        start: mib(1),
        size: mib(100),
        used: Some(mib(40)),
        fs: FileSystem::Fat32,
        kind: PartitionKind::Efi,
        label: Some("EFI".into()),
        flags: PartitionFlags::LOCKED,
        drive_letter: None,
        mountpoints: vec![],
    };
    let msr = Partition {
        id: PartitionId::new(&id, 2),
        number: 2,
        start: efi.end().unwrap(),
        size: mib(16),
        used: None,
        fs: FileSystem::Unformatted,
        kind: PartitionKind::MicrosoftReserved,
        label: None,
        flags: PartitionFlags::LOCKED,
        drive_letter: None,
        mountpoints: vec![],
    };
    let system = Partition {
        id: PartitionId::new(&id, 3),
        number: 3,
        start: msr.end().unwrap(),
        size: gib(180),
        used: Some(gib(96)),
        fs: FileSystem::Ntfs,
        kind: PartitionKind::BasicData,
        label: Some("Windows".into()),
        flags: PartitionFlags::SYSTEM | PartitionFlags::BOOT | PartitionFlags::LOCKED,
        drive_letter: Some('C'),
        mountpoints: vec![],
    };
    let recovery = Partition {
        id: PartitionId::new(&id, 4),
        number: 4,
        start: system.end().unwrap(),
        size: mib(750),
        used: Some(mib(600)),
        fs: FileSystem::Ntfs,
        kind: PartitionKind::Recovery,
        label: Some("Recovery".into()),
        flags: PartitionFlags::LOCKED,
        drive_letter: None,
        mountpoints: vec![],
    };
    let end = recovery.end().unwrap();
    let total = gib(1000);
    let layout = vec![
        Segment::Unallocated {
            start: ByteSize::ZERO,
            size: efi.start,
        },
        Segment::Partition(efi),
        Segment::Partition(msr),
        Segment::Partition(system),
        Segment::Partition(recovery),
        Segment::Unallocated {
            start: end,
            size: total.checked_sub(end).unwrap(),
        },
    ];
    Disk {
        id,
        display_name: "Datenträger 0".into(),
        vendor: "Lunaris Storage".into(),
        model: "Lunaris NV-1000".into(),
        serial: Some("MOCK-0001".into()),
        bus: BusType::Nvme,
        media: MediaType::Ssd,
        size: total,
        logical_sector_size: 512,
        table: PartitionTable::Gpt,
        health: HealthStatus::Ok,
        read_only: false,
        is_system_disk: false,
        layout,
    }
}

fn linux_system_disk() -> Disk {
    let id = DiskId::from("mock-disk-1");
    let efi = Partition {
        id: PartitionId::new(&id, 1),
        number: 1,
        start: mib(1),
        size: mib(512),
        used: Some(mib(80)),
        fs: FileSystem::Fat32,
        kind: PartitionKind::Efi,
        label: Some("EFI".into()),
        flags: PartitionFlags::LOCKED,
        drive_letter: None,
        mountpoints: vec!["/boot/efi".into()],
    };
    let boot = Partition {
        id: PartitionId::new(&id, 2),
        number: 2,
        start: efi.end().unwrap(),
        size: gib(1),
        used: Some(mib(300)),
        fs: FileSystem::Ext4,
        kind: PartitionKind::LinuxFilesystem,
        label: Some("boot".into()),
        flags: PartitionFlags::LOCKED,
        drive_letter: None,
        mountpoints: vec!["/boot".into()],
    };
    let root = Partition {
        id: PartitionId::new(&id, 3),
        number: 3,
        start: boot.end().unwrap(),
        size: gib(180),
        used: Some(gib(60)),
        fs: FileSystem::Btrfs,
        kind: PartitionKind::LinuxFilesystem,
        label: Some("root".into()),
        flags: PartitionFlags::SYSTEM | PartitionFlags::BOOT | PartitionFlags::LOCKED,
        drive_letter: None,
        mountpoints: vec!["/".into()],
    };
    let swap = Partition {
        id: PartitionId::new(&id, 4),
        number: 4,
        start: root.end().unwrap(),
        size: gib(8),
        used: None,
        fs: FileSystem::LinuxSwap,
        kind: PartitionKind::LinuxSwap,
        label: Some("swap".into()),
        flags: PartitionFlags::empty(),
        drive_letter: None,
        mountpoints: vec![],
    };
    let swap_end = swap.end().unwrap();
    let home_start = swap_end.checked_add(gib(40)).unwrap();
    let home = Partition {
        id: PartitionId::new(&id, 5),
        number: 5,
        start: home_start,
        size: gib(247),
        used: Some(gib(120)),
        fs: FileSystem::Ext4,
        kind: PartitionKind::LinuxFilesystem,
        label: Some("home".into()),
        flags: PartitionFlags::empty(),
        drive_letter: None,
        mountpoints: vec!["/home".into()],
    };
    let end = home.end().unwrap();
    let total = gib(512);
    Disk {
        id,
        display_name: "Datenträger 1".into(),
        vendor: "Selene Systems".into(),
        model: "Selene S2-512".into(),
        serial: Some("MOCK-0002".into()),
        bus: BusType::Nvme,
        media: MediaType::Ssd,
        size: total,
        logical_sector_size: 512,
        table: PartitionTable::Gpt,
        health: HealthStatus::Ok,
        read_only: false,
        is_system_disk: false,
        layout: vec![
            Segment::Unallocated {
                start: ByteSize::ZERO,
                size: efi.start,
            },
            Segment::Partition(efi),
            Segment::Partition(boot),
            Segment::Partition(root),
            Segment::Partition(swap),
            Segment::Unallocated {
                start: swap_end,
                size: gib(40),
            },
            Segment::Partition(home),
            Segment::Unallocated {
                start: end,
                size: total.checked_sub(end).unwrap(),
            },
        ],
    }
}

fn usb_stick() -> Disk {
    let id = DiskId::from("mock-disk-3");
    let data = Partition {
        id: PartitionId::new(&id, 1),
        number: 1,
        start: mib(1),
        size: gib(59).checked_add(mib(600)).unwrap(),
        used: Some(gib(12)),
        fs: FileSystem::ExFat,
        kind: PartitionKind::BasicData,
        label: Some("STICK".into()),
        flags: PartitionFlags::empty(),
        drive_letter: Some('E'),
        mountpoints: vec!["/media/stick".into()],
    };
    let end = data.end().unwrap();
    let total = gib(64);
    Disk {
        id,
        display_name: "Datenträger 3".into(),
        vendor: "Sternschnuppe".into(),
        model: "Sternschnuppe Stick 64".into(),
        serial: Some("MOCK-0004".into()),
        bus: BusType::Usb,
        media: MediaType::Unknown,
        size: total,
        logical_sector_size: 512,
        table: PartitionTable::Mbr,
        health: HealthStatus::Ok,
        read_only: false,
        is_system_disk: false,
        layout: vec![
            Segment::Unallocated {
                start: ByteSize::ZERO,
                size: data.start,
            },
            Segment::Partition(data),
            Segment::Unallocated {
                start: end,
                size: total.checked_sub(end).unwrap(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_disks_have_contiguous_gap_free_layouts() {
        let provider = MockDiskProvider;
        for disk in provider.list_disks().unwrap() {
            let mut cursor = ByteSize::ZERO;
            for seg in &disk.layout {
                assert_eq!(
                    seg.start(),
                    cursor,
                    "gap or overlap in {} layout at {:?}",
                    disk.id,
                    seg
                );
                cursor = seg.start().checked_add(seg.size()).unwrap();
            }
            assert_eq!(
                cursor, disk.size,
                "{} layout doesn't add up to disk size",
                disk.id
            );
        }
    }

    #[test]
    fn mock_never_marks_a_disk_as_the_system_disk() {
        // The mock provider must never claim to be running on real
        // hardware; is_system_disk is only ever set by a real provider
        // that has actually checked. See security::system_protection.
        let provider = MockDiskProvider;
        for disk in provider.list_disks().unwrap() {
            assert!(!disk.is_system_disk);
        }
    }
}
