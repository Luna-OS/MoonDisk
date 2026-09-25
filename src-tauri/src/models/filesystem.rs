use serde::{Deserialize, Serialize};

/// Filesystems MoonDisk recognizes. See `docs/supported-operations.md` §2
/// for the capability matrix (what can be formatted/resized/labeled per
/// filesystem).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileSystem {
    Ntfs,
    Fat32,
    ExFat,
    Ext2,
    Ext3,
    Ext4,
    Btrfs,
    Xfs,
    LinuxSwap,
    Apfs,
    /// Mac OS Extended (HFS+).
    HfsPlus,
    /// The partition has no recognizable filesystem signature.
    Unformatted,
    /// A filesystem exists but MoonDisk does not recognize it.
    Unknown,
}

impl FileSystem {
    /// Parse the `FSTYPE` MoonDisk gets from `lsblk`/`blkid`. Unknown values
    /// map to `Unknown` rather than failing, since disks *will* show up
    /// with filesystems this list doesn't cover and the UI must still be
    /// able to display them (just with most actions disabled).
    pub fn from_lsblk_fstype(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" => FileSystem::Unformatted,
            "ntfs" => FileSystem::Ntfs,
            "vfat" | "fat32" => FileSystem::Fat32,
            "exfat" => FileSystem::ExFat,
            "ext2" => FileSystem::Ext2,
            "ext3" => FileSystem::Ext3,
            "ext4" => FileSystem::Ext4,
            "btrfs" => FileSystem::Btrfs,
            "xfs" => FileSystem::Xfs,
            "swap" => FileSystem::LinuxSwap,
            "apfs" => FileSystem::Apfs,
            "hfs" | "hfsplus" => FileSystem::HfsPlus,
            _ => FileSystem::Unknown,
        }
    }

    pub fn can_format(self) -> bool {
        !matches!(self, FileSystem::Unknown)
    }

    pub fn can_resize(self) -> bool {
        matches!(
            self,
            FileSystem::Ntfs
                | FileSystem::Fat32
                | FileSystem::Ext2
                | FileSystem::Ext3
                | FileSystem::Ext4
                | FileSystem::Btrfs
                | FileSystem::Unformatted
        )
        // exFAT and XFS intentionally excluded: see
        // docs/supported-operations.md §2, notes 4 and 6.
    }

    pub fn windows_readable(self) -> bool {
        matches!(
            self,
            FileSystem::Ntfs | FileSystem::Fat32 | FileSystem::ExFat
        )
    }
}

/// What role a partition plays, independent of its filesystem. Drives the
/// system-protection rules in `docs/supported-operations.md` §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PartitionKind {
    Efi,
    MicrosoftReserved,
    Recovery,
    BasicData,
    LinuxFilesystem,
    LinuxSwap,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_lsblk_fstypes() {
        assert_eq!(FileSystem::from_lsblk_fstype("ext4"), FileSystem::Ext4);
        assert_eq!(FileSystem::from_lsblk_fstype("vfat"), FileSystem::Fat32);
        assert_eq!(FileSystem::from_lsblk_fstype(""), FileSystem::Unformatted);
        assert_eq!(
            FileSystem::from_lsblk_fstype("zfs_member"),
            FileSystem::Unknown
        );
    }

    #[test]
    fn unknown_filesystem_blocks_every_action() {
        assert!(!FileSystem::Unknown.can_format());
        assert!(!FileSystem::Unknown.can_resize());
    }

    #[test]
    fn exfat_and_xfs_cannot_resize() {
        assert!(!FileSystem::ExFat.can_resize());
        assert!(!FileSystem::Xfs.can_resize());
    }
}
