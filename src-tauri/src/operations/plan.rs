use crate::models::{ByteSize, DiskId, FileSystem, PartitionId};
use serde::{Deserialize, Serialize};

/// What the UI is asking for. Deliberately a flat, exhaustively-typed enum
/// (not a free-form command string) so nothing here can ever become a
/// place where arbitrary text turns into a process argument — see
/// `docs/architecture.md` §6.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OperationRequest {
    CreatePartition {
        disk: DiskId,
        start: ByteSize,
        size: ByteSize,
        filesystem: FileSystem,
        label: Option<String>,
        /// Windows only — the drive letter to assign at creation. Ignored
        /// on Linux, which has no equivalent concept (see
        /// `Partition::drive_letter`).
        drive_letter: Option<char>,
    },
    DeletePartition {
        partition: PartitionId,
    },
    FormatPartition {
        partition: PartitionId,
        filesystem: FileSystem,
        label: Option<String>,
    },
    SetLabel {
        partition: PartitionId,
        label: String,
    },
    /// Windows only — assigns or changes a partition's drive letter.
    /// Linux has no equivalent concept (see `Partition::drive_letter`) and
    /// rejects this with `ExecutionError::NotImplemented`.
    SetDriveLetter {
        partition: PartitionId,
        drive_letter: char,
    },
}

impl OperationRequest {
    /// The disk this request targets. For partition-scoped requests this
    /// is parsed out of the `"{disk}#{number}"` `PartitionId` — a routing
    /// convenience only; `validator::validate` never trusts it for a
    /// security decision and always re-derives the relationship by
    /// actually looking the partition up inside the named disk.
    pub fn disk_id(&self) -> DiskId {
        match self {
            OperationRequest::CreatePartition { disk, .. } => disk.clone(),
            OperationRequest::DeletePartition { partition }
            | OperationRequest::FormatPartition { partition, .. }
            | OperationRequest::SetLabel { partition, .. }
            | OperationRequest::SetDriveLetter { partition, .. } => partition_disk(partition),
        }
    }

    pub fn kind(&self) -> OperationKind {
        match self {
            OperationRequest::CreatePartition { .. } => OperationKind::Create,
            OperationRequest::DeletePartition { .. } => OperationKind::Delete,
            OperationRequest::FormatPartition { .. } => OperationKind::Format,
            OperationRequest::SetLabel { .. } => OperationKind::SetLabel,
            OperationRequest::SetDriveLetter { .. } => OperationKind::SetDriveLetter,
        }
    }

    pub fn risk(&self) -> RiskLevel {
        match self {
            OperationRequest::SetLabel { .. } | OperationRequest::SetDriveLetter { .. } => {
                RiskLevel::Low
            }
            OperationRequest::CreatePartition { .. } => RiskLevel::Medium,
            OperationRequest::DeletePartition { .. } | OperationRequest::FormatPartition { .. } => {
                RiskLevel::Critical
            }
        }
    }
}

/// `PartitionId` is `"{disk}#{number}"` (see `models::ids`); this recovers
/// the disk half without needing a lookup.
fn partition_disk(id: &PartitionId) -> DiskId {
    DiskId::from(id.0.split('#').next().unwrap_or_default().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Create,
    Delete,
    Format,
    SetLabel,
    SetDriveLetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
