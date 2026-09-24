use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier for a disk. For real providers this is the OS device
/// path (`/dev/sda`, `\\.\PhysicalDrive0`); for the mock provider it is a
/// fixed string like `mock-disk-0`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiskId(pub String);

impl fmt::Display for DiskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for DiskId {
    fn from(s: String) -> Self {
        DiskId(s)
    }
}

impl From<&str> for DiskId {
    fn from(s: &str) -> Self {
        DiskId(s.to_string())
    }
}

/// Identifier for a partition, scoped to its disk: `{disk_id}#{number}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartitionId(pub String);

impl PartitionId {
    pub fn new(disk: &DiskId, number: u32) -> Self {
        PartitionId(format!("{}#{}", disk.0, number))
    }
}

impl fmt::Display for PartitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
