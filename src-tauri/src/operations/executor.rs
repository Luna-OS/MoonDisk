//! The only thing in MoonDisk allowed to change a disk.
//!
//! Every implementation MUST, in this order: re-fetch the current disk
//! state, re-run `validator::validate`, re-check
//! `security::system_protection`, and only then act. This is enforced by
//! convention (there is no way to make the trait itself require it), so
//! the Linux/Windows executors are reviewed accordingly — see
//! `docs/safety-model.md` §5.6.

use super::OperationRequest;
use crate::models::DiskId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Bestätigung fehlt oder ist ungültig")]
    ConfirmationMissing,
    #[error("Validierung fehlgeschlagen: {0}")]
    ValidationFailed(#[from] super::ValidationError),
    #[error("dies ist der Systemdatenträger, auf dem MoonDisk läuft")]
    SystemDisk,
    #[error("Ausführung fehlgeschlagen: {0}")]
    Failed(String),
    #[error("diese Operation ist noch nicht implementiert: {0}")]
    NotImplemented(String),
    #[error("Lesezugriff auf den Datenträger fehlgeschlagen: {0}")]
    Inventory(#[from] crate::platform::InventoryError),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    pub disk: DiskId,
    pub applied: usize,
    pub total: usize,
}

/// Confirmation the caller (the Tauri command layer) has already checked
/// the typed phrase for, if this request needed one. Executors still
/// re-validate everything else themselves; this only carries the fact
/// that the human-facing confirmation step happened.
#[derive(Debug, Clone, Copy)]
pub struct Confirmation {
    pub phrase_confirmed: bool,
}

pub trait DiskOperationExecutor {
    /// Executes a single already-validated request for real (or, for the
    /// mock executor, against the in-memory mock state). Implementations
    /// apply at most one `OperationRequest` per call — the caller
    /// sequences a multi-step plan and stops at the first failure, which
    /// is what lets `ExecutionReport` say exactly how far it got.
    fn execute(
        &mut self,
        req: &OperationRequest,
        confirmation: Confirmation,
    ) -> Result<(), ExecutionError>;
}

/// Trivial in-memory executor used by tests and by the frontend's mock
/// mode. It does not touch `platform::mock::MockDiskProvider`'s static
/// sample data (that would need interior mutability to be genuinely
/// stateful across calls); it exists so the planner/confirmation flow has
/// something safe to run against end-to-end.
pub struct NullExecutor {
    pub calls: Vec<OperationRequest>,
}

impl NullExecutor {
    pub fn new() -> Self {
        NullExecutor { calls: Vec::new() }
    }
}

impl Default for NullExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskOperationExecutor for NullExecutor {
    fn execute(
        &mut self,
        req: &OperationRequest,
        confirmation: Confirmation,
    ) -> Result<(), ExecutionError> {
        if req.risk() >= super::RiskLevel::High && !confirmation.phrase_confirmed {
            return Err(ExecutionError::ConfirmationMissing);
        }
        self.calls.push(req.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn null_executor_requires_confirmation_for_critical_ops() {
        let mut exec = NullExecutor::new();
        let req = OperationRequest::DeletePartition {
            partition: PartitionId::new(&DiskId::from("d"), 1),
        };
        let result = exec.execute(
            &req,
            Confirmation {
                phrase_confirmed: false,
            },
        );
        assert!(matches!(result, Err(ExecutionError::ConfirmationMissing)));
        assert!(exec.calls.is_empty());
    }

    #[test]
    fn null_executor_records_confirmed_calls() {
        let mut exec = NullExecutor::new();
        let req = OperationRequest::SetLabel {
            partition: PartitionId::new(&DiskId::from("d"), 1),
            label: "x".into(),
        };
        exec.execute(
            &req,
            Confirmation {
                phrase_confirmed: false,
            },
        )
        .unwrap();
        assert_eq!(exec.calls.len(), 1);
    }
}
