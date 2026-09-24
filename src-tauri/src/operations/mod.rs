//! Plan → validate → confirm → execute. See `docs/safety-model.md` §5.
//!
//! `OperationRequest` is what the UI sends. `validator` checks it against
//! the rules in `docs/supported-operations.md`. `DiskOperationExecutor` is
//! the only thing allowed to actually change a disk, and every
//! implementation of it must call `security::system_protection` and
//! re-validate immediately before touching anything.

pub mod executor;
pub mod plan;
pub mod validator;

pub use executor::{Confirmation, DiskOperationExecutor, ExecutionError, ExecutionReport};
pub use plan::{OperationKind, OperationRequest, RiskLevel};
pub use validator::{validate, ValidationError};
