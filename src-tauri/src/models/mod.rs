//! Shared data models for disks, partitions, and filesystems.
//!
//! These types are used by every provider (mock and real) and by the
//! operations planner, so the rest of the app never has to care whether a
//! `Disk` came from `MockDiskProvider` or from a real Linux/Windows
//! backend. See `docs/architecture.md` §5.3 for the original design.

mod disk;
mod filesystem;
mod ids;
mod partition;
mod size;

pub use disk::{BusType, Disk, HealthStatus, MediaType, PartitionTable};
pub use filesystem::{FileSystem, PartitionKind};
pub use ids::{DiskId, PartitionId};
pub use partition::{Partition, PartitionFlags, Segment};
pub use size::ByteSize;
