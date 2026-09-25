//! Checks the USB writer's FAT32 builder with dosfstools' `fsck.fat`, an
//! independent implementation. Skipped when `fsck.fat` isn't installed.

use moondisk_lib::flash::fat32::{FatDateTime, FatNode, Geometry, Plan};
use std::process::Command;

const MIB: u64 = 1024 * 1024;

fn file(name: &str, id: usize, size: u64) -> FatNode {
    FatNode::File {
        name: name.into(),
        date: FatDateTime::new(2024, 9, 1, 10, 30, 0),
        size,
        id,
    }
}

fn dir(name: &str, children: Vec<FatNode>) -> FatNode {
    FatNode::Dir {
        name: name.into(),
        date: FatDateTime::new(2024, 9, 1, 10, 30, 0),
        children,
    }
}

#[test]
fn fsck_accepts_the_volume() {
    if Command::new("fsck.fat").arg("--help").output().is_err() {
        eprintln!("fsck.fat not installed, skipping");
        return;
    }
    let many = (0..120)
        .map(|i| {
            file(
                &format!("package-{i:03}-with-a-long-name.pkg.tar.zst"),
                100 + i,
                700,
            )
        })
        .collect();
    let tree = vec![
        dir(
            "EFI",
            vec![dir(
                "BOOT",
                vec![file("BOOTx64.EFI", 1, 5000), file("BOOTIA32.EFI", 2, 3000)],
            )],
        ),
        dir("arch", vec![file("airootfs.sfs", 3, 3 * MIB + 17)]),
        dir("pkgs", many),
        file("README", 4, 10),
        file("readme", 5, 11),
        file("empty.uuid", 6, 0),
    ];
    let partition = 100 * MIB;
    let plan = Plan::new(
        Geometry::plan(partition, 512).unwrap(),
        "ARCH_202409",
        &tree,
    )
    .unwrap();
    let mut bytes = Vec::new();
    plan.write(&mut bytes, 2048, 0x1234_5678, |id, size, w| {
        w.write_all(&vec![id as u8; size as usize])
    })
    .unwrap();
    bytes.resize(partition as usize, 0);

    let path = std::env::temp_dir().join(format!("moondisk-fat32-{}.img", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let out = Command::new("fsck.fat")
        .args(["-n", "-v"])
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        out.status.success(),
        "fsck.fat failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
