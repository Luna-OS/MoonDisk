//! End-to-end proof that the real Linux write path works — run against a
//! throwaway loop device backed by a file in `/tmp`, **never** against a
//! real disk. This is `#[ignore]`d by default because it needs root (to
//! attach a loop device) and the partitioning tools installed
//! (`parted`, `mkfs.ext4`, `e2fsprogs`); run it explicitly with:
//!
//! ```sh
//! cargo test --test linux_write_path -- --ignored --nocapture
//! ```
//!
//! If this ever needs to run in CI, the workflow must set those tools up
//! first (see `.github/workflows/ci.yml`).

#![cfg(target_os = "linux")]

use moondisk_lib::models::{ByteSize, DiskId, FileSystem};
use moondisk_lib::operations::{Confirmation, DiskOperationExecutor, OperationRequest};
use moondisk_lib::platform::linux::LinuxDiskProvider;
use moondisk_lib::platform::linux_executor::LinuxDiskExecutor;
use moondisk_lib::platform::DiskInventory;
use std::process::Command;

struct LoopDevice {
    path: String,
    image_path: std::path::PathBuf,
}

impl LoopDevice {
    fn attach(size_mib: u64, tag: &str) -> Self {
        let image_path = std::env::temp_dir().join(format!(
            "moondisk-write-path-test-{tag}-{}.img",
            std::process::id()
        ));
        let status = Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", image_path.display()),
                "bs=1M",
                &format!("count={size_mib}"),
            ])
            .status()
            .expect("dd should run");
        assert!(status.success(), "failed to create backing image");

        let out = Command::new("losetup")
            .args(["--show", "-f", "-P", image_path.to_str().unwrap()])
            .output()
            .expect("losetup should run");
        assert!(out.status.success(), "losetup failed: {:?}", out);
        let path = String::from_utf8(out.stdout).unwrap().trim().to_string();

        LoopDevice { path, image_path }
    }
}

impl Drop for LoopDevice {
    fn drop(&mut self) {
        let _ = Command::new("losetup").args(["-d", &self.path]).status();
        let _ = std::fs::remove_file(&self.image_path);
    }
}

fn mib(n: u64) -> ByteSize {
    ByteSize(n * 1024 * 1024)
}

#[test]
#[ignore]
fn create_format_relabel_and_delete_a_real_partition() {
    let loop_dev = LoopDevice::attach(64, "partitions");
    println!("using throwaway loop device: {}", loop_dev.path);

    // A brand-new loop device has no partition table yet; give it one the
    // same way a real disk would arrive with GPT already on it (MoonDisk
    // itself never calls `mklabel` today — creating a table from scratch
    // is out of scope, same as resize/move — so this is test setup, not
    // something the app does).
    let status = Command::new("parted")
        .args(["--script", &loop_dev.path, "mklabel", "gpt"])
        .status()
        .unwrap();
    assert!(status.success());

    let provider = LinuxDiskProvider;
    let disk_id = DiskId::from(loop_dev.path.clone());

    let disk = provider
        .disk(&disk_id)
        .expect("loop device should be listed");
    assert!(!disk.is_system_disk);
    assert!(!disk.read_only);

    let mut executor = LinuxDiskExecutor;
    let confirmation = Confirmation { confirmed: true };

    // --- create ---
    let create = OperationRequest::CreatePartition {
        disk: disk_id.clone(),
        start: mib(1),
        size: mib(30),
        filesystem: FileSystem::Ext4,
        label: Some("moondisktest".into()),
        drive_letter: None,
    };
    executor
        .execute(&create, confirmation)
        .expect("create should succeed");

    let disk = provider.disk(&disk_id).unwrap();
    let created = disk
        .partitions()
        .next()
        .expect("partition should now exist");
    assert_eq!(created.number, 1);
    assert_eq!(created.start, mib(1));
    let partition_id = created.id.clone();

    // --- format ---
    let format = OperationRequest::FormatPartition {
        partition: partition_id.clone(),
        filesystem: FileSystem::Ext4,
        label: Some("moondisktest".into()),
    };
    executor
        .execute(&format, confirmation)
        .expect("format should succeed");

    let disk = provider.disk(&disk_id).unwrap();
    let formatted = disk.partitions().next().unwrap();
    assert_eq!(formatted.fs, FileSystem::Ext4);
    assert_eq!(formatted.label.as_deref(), Some("moondisktest"));

    // --- relabel ---
    let relabel = OperationRequest::SetLabel {
        partition: partition_id.clone(),
        label: "renamed".into(),
    };
    executor
        .execute(&relabel, confirmation)
        .expect("relabel should succeed");

    let disk = provider.disk(&disk_id).unwrap();
    let relabeled = disk.partitions().next().unwrap();
    assert_eq!(relabeled.label.as_deref(), Some("renamed"));

    // --- delete ---
    let delete = OperationRequest::DeletePartition {
        partition: partition_id,
    };
    executor
        .execute(&delete, confirmation)
        .expect("delete should succeed");

    let disk = provider.disk(&disk_id).unwrap();
    assert_eq!(
        disk.partitions().count(),
        0,
        "partition should be gone after delete"
    );

    println!(
        "real create -> format -> relabel -> delete cycle succeeded on {}",
        loop_dev.path
    );
}

#[test]
#[ignore]
fn writes_a_partitioned_image_onto_a_whole_disk() {
    use moondisk_lib::flash::{verify_image, write_image};
    use moondisk_lib::models::PartitionTable;
    use moondisk_lib::platform::linux_executor::{
        drop_read_cache, finish_raw_write, prepare_raw_write,
    };
    use std::fs::{File, OpenOptions};
    use std::sync::atomic::AtomicBool;

    // Stand-in for an installer image: 16 MiB of random data with an MBR
    // and one partition, the way a hybrid Linux ISO carries its own table.
    let image_path =
        std::env::temp_dir().join(format!("moondisk-flash-source-{}.img", std::process::id()));
    let status = Command::new("dd")
        .args([
            "if=/dev/urandom",
            &format!("of={}", image_path.display()),
            "bs=1M",
            "count=16",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("parted")
        .args(["--script", image_path.to_str().unwrap(), "mklabel", "msdos"])
        .args(["mkpart", "primary", "1MiB", "15MiB"])
        .status()
        .unwrap();
    assert!(status.success());

    let loop_dev = LoopDevice::attach(64, "flash");
    let provider = LinuxDiskProvider;
    let disk_id = DiskId::from(loop_dev.path.clone());
    let disk = provider.disk(&disk_id).unwrap();
    assert_eq!(disk.partitions().count(), 0);

    prepare_raw_write(&disk).expect("prepare");
    let len = std::fs::metadata(&image_path).unwrap().len();
    let mut target = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&loop_dev.path)
        .unwrap();
    let cancel = AtomicBool::new(false);
    write_image(
        &mut File::open(&image_path).unwrap(),
        len,
        &mut target,
        &cancel,
        |_| {},
    )
    .expect("write");
    drop_read_cache(&disk).expect("flush cache");
    verify_image(
        &mut File::open(&image_path).unwrap(),
        len,
        &mut target,
        &cancel,
        |_| {},
    )
    .expect("verify");
    drop(target);
    finish_raw_write(&disk);

    let disk = provider.disk(&disk_id).unwrap();
    assert_eq!(disk.table, PartitionTable::Mbr);
    assert_eq!(
        disk.partitions().count(),
        1,
        "the kernel should see the image's partition"
    );
    println!("raw image write + verify succeeded on {}", loop_dev.path);

    let _ = std::fs::remove_file(&image_path);
}

#[test]
#[ignore]
fn erases_a_drive_an_iso_was_written_to() {
    use moondisk_lib::models::PartitionTable;
    use std::io::{Seek, SeekFrom, Write};

    let loop_dev = LoopDevice::attach(64, "erase");
    let dev = loop_dev.path.as_str();

    // What a hybrid Linux ISO (e.g. Arch's) leaves on a stick: an MBR with
    // the ISO's data partition and a small FAT EFI partition, plus the
    // ISO9660 volume descriptors at 32 KiB of the whole disk.
    let status = Command::new("parted")
        .args(["--script", dev, "mklabel", "msdos"])
        .args(["mkpart", "primary", "1MiB", "40MiB"])
        .args(["mkpart", "primary", "fat32", "40MiB", "48MiB"])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = Command::new("partprobe").arg(dev).status();
    let status = Command::new("mkfs.vfat")
        .args(["-n", "ARCHISO_EFI", &format!("{dev}p2")])
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
    {
        let mut disk = std::fs::OpenOptions::new().write(true).open(dev).unwrap();
        let mut descriptors = vec![0u8; 4096];
        descriptors[..6].copy_from_slice(b"\x01CD001");
        descriptors[2048..2054].copy_from_slice(b"\xffCD001");
        disk.seek(SeekFrom::Start(32 * 1024)).unwrap();
        disk.write_all(&descriptors).unwrap();
        disk.sync_all().unwrap();
    }

    let probe = |dev: &str| {
        let out = Command::new("blkid")
            .args(["-p", "-o", "export", dev])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert!(
        probe(dev).contains("TYPE=iso9660"),
        "test setup: {}",
        probe(dev)
    );

    let provider = LinuxDiskProvider;
    let disk_id = DiskId::from(loop_dev.path.clone());
    assert_eq!(provider.disk(&disk_id).unwrap().partitions().count(), 2);

    let erase = OperationRequest::EraseDisk {
        disk: disk_id.clone(),
        filesystem: FileSystem::ExFat,
        label: Some("USB".into()),
    };
    LinuxDiskExecutor
        .execute(&erase, Confirmation { confirmed: true })
        .expect("erase should succeed");

    let disk = provider.disk(&disk_id).unwrap();
    assert_eq!(disk.table, PartitionTable::Mbr);
    let parts: Vec<_> = disk.partitions().collect();
    assert_eq!(parts.len(), 1, "one partition over the whole drive");
    assert_eq!(parts[0].start, mib(1));
    assert!(parts[0].size.0 > 60 * 1024 * 1024);
    assert_eq!(parts[0].fs, FileSystem::ExFat);
    assert_eq!(parts[0].label.as_deref(), Some("USB"));

    let after = probe(dev);
    assert!(
        !after.contains("iso9660"),
        "the ISO signature must be gone: {after}"
    );
    println!("erased {} into one exFAT partition", loop_dev.path);
}

#[test]
#[ignore]
fn copies_an_iso_onto_a_loop_device() {
    use moondisk_lib::flash::copy::{plan, prepare, write_to};
    use std::fs::File;
    use std::sync::atomic::AtomicBool;

    let loop_dev = LoopDevice::attach(200, "copy");
    let dev = loop_dev.path.clone();
    let disk_size = 200 * 1024 * 1024;

    let fixture = format!("{}/tests/fixtures/archlike.iso", env!("CARGO_MANIFEST_DIR"));
    let mut image = File::open(&fixture).unwrap();
    let prepared = prepare(&mut image).unwrap();
    let plan = plan(&prepared, disk_size, 512).unwrap();
    let mut target = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&dev)
        .unwrap();
    let cancel = AtomicBool::new(false);
    write_to(
        &mut image,
        &prepared,
        &plan,
        &mut target,
        disk_size,
        true,
        || {
            let status = Command::new("blockdev")
                .args(["--flushbufs", &dev])
                .status()
                .unwrap();
            assert!(status.success());
            Ok(())
        },
        &cancel,
        &mut |_, _, _| {},
    )
    .expect("copy");
    drop(target);
    let _ = Command::new("partprobe").arg(&dev).status();

    let probe = |dev: &str| {
        let out = Command::new("blkid")
            .args(["-p", "-o", "export", dev])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let part = format!("{dev}p1");
    let fs = probe(&part);
    assert!(fs.contains("TYPE=vfat"), "{fs}");
    assert!(fs.contains("VERSION=FAT32"), "{fs}");
    assert!(fs.contains("LABEL=ARCH_202409"), "{fs}");
    assert!(probe(&dev).contains("PTTYPE=dos"));

    let fsck = Command::new("fsck.fat")
        .args(["-n", "-v", &part])
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&fsck.stdout));
    assert!(fsck.status.success(), "{fsck:?}");

    // The provider sees one FAT32 partition, like after Rufus.
    let disk = LinuxDiskProvider.disk(&DiskId::from(dev.clone())).unwrap();
    let parts: Vec<_> = disk.partitions().collect();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].fs, FileSystem::Fat32);
    assert_eq!(parts[0].label.as_deref(), Some("ARCH_202409"));
    println!("copied archlike.iso onto {dev}");
}
