//! Shared sqlx query helpers.
//!
//! T016: thin adapters that map `sqlx::Error` to `WorkbenchError` and carry
//! row-not-found semantics. Services import from here rather than touching
//! sqlx types directly in command implementations.

use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};

/// Convert a sqlx `Result` into a workbench `Result`, preserving `NotFound`.
pub fn map_sqlx<T>(result: Result<T, sqlx::Error>) -> WorkbenchResult<T> {
    result.map_err(WorkbenchError::from)
}

/// Turn a sqlx `Option<T>` row fetch into `NotFound` when absent.
pub fn require_found<T>(row: Option<T>, what: impl Into<String>) -> WorkbenchResult<T> {
    row.ok_or_else(|| WorkbenchError::new(ErrorCode::NotFound, what))
}
