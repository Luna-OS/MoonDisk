use super::{ByteSize, FileSystem, PartitionId, PartitionKind};
use serde::{Deserialize, Serialize};

bitflags::bitflags! {
    /// Sent to the UIs as a plain number (the bit mask), which both the web
    /// UI and the Mac app read — not as bitflags' own "BOOT | SYSTEM" text.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PartitionFlags: u8 {
        const BOOT          = 0b0000_0001;
        const SYSTEM        = 0b0000_0010;
        /// The partition is currently mounted / has an active volume.
        const ACTIVE_MOUNT  = 0b0000_0100;
        /// MoonDisk refuses to modify this partition (see
        /// `security::system_protection`).
        const LOCKED        = 0b0000_1000;
    }
}

impl Serialize for PartitionFlags {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for PartitionFlags {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u8::deserialize(deserializer).map(PartitionFlags::from_bits_truncate)
    }
}

/// One partition on a disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    pub id: PartitionId,
    pub number: u32,
    /// Start offset, in bytes from the start of the disk.
    pub start: ByteSize,
    pub size: ByteSize,
    pub used: Option<ByteSize>,
    pub fs: FileSystem,
    pub kind: PartitionKind,
    pub label: Option<String>,
    pub flags: PartitionFlags,
    /// Windows drive letter, e.g. `Some('D')`. Only meaningful on Windows;
    /// always `None` from the Linux provider.
    pub drive_letter: Option<char>,
    /// Linux mountpoints (a Btrfs partition can have more than one
    /// subvolume mounted). Always empty from the Windows provider.
    pub mountpoints: Vec<String>,
}

impl Partition {
    pub fn end(&self) -> Option<ByteSize> {
        self.start.checked_add(self.size)
    }
}

/// One region of a disk's layout: either a partition or unallocated space.
/// A disk's `layout` is a sorted, gap-free sequence of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum Segment {
    Partition(Partition),
    Unallocated { start: ByteSize, size: ByteSize },
}

impl Segment {
    pub fn start(&self) -> ByteSize {
        match self {
            Segment::Partition(p) => p.start,
            Segment::Unallocated { start, .. } => *start,
        }
    }

    pub fn size(&self) -> ByteSize {
        match self {
            Segment::Partition(p) => p.size,
            Segment::Unallocated { size, .. } => *size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_sent_as_a_number() {
        let flags = PartitionFlags::BOOT | PartitionFlags::SYSTEM;
        assert_eq!(serde_json::to_string(&flags).unwrap(), "3");
        assert_eq!(
            serde_json::to_string(&PartitionFlags::empty()).unwrap(),
            "0"
        );
        assert_eq!(serde_json::from_str::<PartitionFlags>("3").unwrap(), flags);
    }
}
