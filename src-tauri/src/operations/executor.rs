//! The only thing in MoonDisk allowed to change a disk.
//!
//! Every implementation MUST, in this order: re-fetch the current disk
//! state, re-run `validator::validate`, and only then act. This is
//! enforced by convention (there is no way to make the trait itself
//! require it), so the Linux/Windows executors are reviewed accordingly.

use super::OperationRequest;
use crate::models::DiskId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("confirmation is missing or invalid")]
    ConfirmationMissing,
    #[error("validation failed: {0}")]
    ValidationFailed(#[from] super::ValidationError),
    #[error("operation failed: {0}")]
    Failed(String),
    #[error("not supported yet: {0}")]
    NotImplemented(String),
    #[error("could not read the disk: {0}")]
    Inventory(#[from] crate::platform::InventoryError),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    pub disk: DiskId,
    pub applied: usize,
    pub total: usize,
}

/// Confirmation the caller (the Tauri command layer) has already checked,
/// if this request needed one. Executors still re-validate everything else
/// themselves; this only carries the fact that the human-facing
/// confirmation click happened.
#[derive(Debug, Clone, Copy)]
pub struct Confirmation {
    pub confirmed: bool,
}

pub trait DiskOperationExecutor {
    /// Executes a single already-validated request for real. Implementations
    /// apply at most one `OperationRequest` per call — the caller
    /// sequences a multi-step plan and stops at the first failure, which
    /// is what lets `ExecutionReport` say exactly how far it got.
    fn execute(
        &mut self,
        req: &OperationRequest,
        confirmation: Confirmation,
    ) -> Result<(), ExecutionError>;
}
