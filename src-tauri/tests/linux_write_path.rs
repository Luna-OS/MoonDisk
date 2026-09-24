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
    fn attach(size_mib: u64) -> Self {
        let image_path = std::env::temp_dir().join(format!(
            "moondisk-write-path-test-{}.img",
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
    let loop_dev = LoopDevice::attach(64);
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
