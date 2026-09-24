//! Detects and hard-blocks the disk hosting the currently running OS.
//!
//! This is independent of (and in addition to) the ordinary partition
//! protections in `docs/supported-operations.md` §6 — it exists so that a
//! bug elsewhere (a wrong disk ID from the UI, a stale plan, …) cannot
//! result in MoonDisk formatting the machine it is running on. The
//! executor re-derives this immediately before every write; it never
//! trusts an `is_system_disk` flag handed to it from outside this module.

use std::process::Command;

/// Returns the mount source for `/` on Linux, e.g. `/dev/nvme0n1p2`, or
/// `None` if `/` isn't backed by a real block device (overlayfs, tmpfs,
/// containers, …) — in which case there is nothing on this machine for
/// MoonDisk to protect by this mechanism.
#[cfg(target_os = "linux")]
pub fn system_disk_source_linux() -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    let root_source = mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let source = fields.next()?;
        let target = fields.next()?;
        (target == "/").then(|| source.to_string())
    })?;

    if !root_source.starts_with("/dev/") {
        // overlay, tmpfs, none, a network filesystem, ... — no block
        // device to protect.
        return None;
    }

    Some(resolve_to_parent_disk(&root_source))
}

/// Best-effort resolution of a partition device to its parent disk, via
/// `lsblk -no pkname` (a single, fixed, read-only invocation — see
/// `docs/safety-model.md` §3 layer L4). Falls back to the input path
/// unchanged if resolution fails, which still protects at least that exact
/// device node.
///
/// Known limitation: for one level of LVM/LUKS indirection this returns
/// the immediate parent (the physical volume or LUKS device), not
/// necessarily a raw disk. Full device-mapper awareness is tracked in
/// `docs/roadmap.md` (LVM/LUKS are explicit alpha non-goals).
fn resolve_to_parent_disk(partition_path: &str) -> String {
    let output = Command::new("lsblk")
        .args(["-no", "pkname", partition_path])
        .env("LC_ALL", "C")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let parent = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if parent.is_empty() {
                partition_path.to_string()
            } else {
                format!("/dev/{parent}")
            }
        }
        _ => partition_path.to_string(),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn system_disk_source_linux() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_falls_back_to_input_when_lsblk_unavailable_or_unresolvable() {
        // A path that cannot possibly be a real partition; lsblk will fail
        // or return nothing, and the fallback must still be the original
        // path rather than panicking or silently returning "protect
        // nothing".
        let resolved = resolve_to_parent_disk("/dev/does-not-exist-xyz");
        assert_eq!(resolved, "/dev/does-not-exist-xyz");
    }
}
