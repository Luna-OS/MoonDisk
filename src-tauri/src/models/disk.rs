use super::{ByteSize, DiskId, Segment};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BusType {
    Sata,
    Nvme,
    Usb,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaType {
    Hdd,
    Ssd,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthStatus {
    Ok,
    Warning,
    Failing,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PartitionTable {
    Gpt,
    Mbr,
    /// The disk has no recognized partition table (unpartitioned or an
    /// exotic scheme MoonDisk doesn't parse).
    None,
}

/// A disk (or disk-like block device) and its full partition layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub id: DiskId,
    pub display_name: String,
    pub vendor: String,
    pub model: String,
    /// Redacted before this ever reaches a log or bug report — see
    /// `reporting::sanitizer` — but shown in the UI, which is why it isn't
    /// a stronger `Sensitive<T>` wrapper type here (that lives at the
    /// reporting boundary, not the data model).
    pub serial: Option<String>,
    pub bus: BusType,
    pub media: MediaType,
    pub size: ByteSize,
    pub logical_sector_size: u32,
    pub table: PartitionTable,
    pub health: HealthStatus,
    pub read_only: bool,
    /// True for the disk hosting the currently running OS's root
    /// filesystem. MoonDisk hard-blocks every write to this disk — see
    /// `security::system_protection`.
    pub is_system_disk: bool,
    pub layout: Vec<Segment>,
}

impl Disk {
    /// Convenience accessor used throughout the planner/validator.
    pub fn partitions(&self) -> impl Iterator<Item = &super::Partition> {
        self.layout.iter().filter_map(|s| match s {
            Segment::Partition(p) => Some(p),
            Segment::Unallocated { .. } => None,
        })
    }
}
