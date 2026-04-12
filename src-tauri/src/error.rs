//! Structured error type for the workbench backend.
//!
//! T014: `WorkbenchError { code, message, details }` matches the Tauri JSON
//! error envelope in `contracts/tauri-commands.md §8`. Serde-serializable so
//! Tauri can reject commands with it directly.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The full catalog of error codes. Superset of the wire-visible codes from
/// `tend-protocol::ErrorCode` — the daemon server translates between the
/// two surfaces in `daemon/server.rs`.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Generic "entity does not exist" (bare id lookup failure).
    NotFound,
    /// Duplicate insert would violate a unique constraint.
    AlreadyExists,
    /// Project path is already registered (returns existing id).
    AlreadyRegistered,
    /// Attempt to unarchive a project that is not archived.
    NotArchived,
    /// Filesystem path does not exist at canonicalization time.
    PathNotFound,
    /// Path exists but isn't a directory.
    PathNotADirectory,
    /// Session working directory is invalid (doesn't exist / unreadable).
    WorkingDirectoryInvalid,
    /// Operation rejected because the owning project is archived.
    ProjectArchived,
    /// PTY or process spawn failed. `details.os_error` carries errno.
    SpawnFailed,
    /// Companion terminal could not be created.
    CompanionSpawnFailed,
    /// PTY write returned an error.
    WriteFailed,
    /// Operation requires a live session but this session has ended.
    SessionEnded,
    /// Operation not valid on a wrapper-owned (mirror) session.
    SessionReadOnly,
    /// Note or reminder content was empty / whitespace-only.
    ContentEmpty,
    /// Layout name already in use.
    NameTaken,
    /// Daemon IPC: unknown kind, missing field, wrong protocol version.
    ProtocolError,
    /// Daemon IPC: frame exceeded MAX_FRAME_SIZE.
    MessageTooLarge,
    /// Daemon IPC: socket permissions rejected.
    Unauthorized,
    /// Fallback; `message` carries detail.
    Internal,
}

impl ErrorCode {
    /// Translate to the wire-visible protocol subset. Panics only if a code
    /// that is not wire-representable ends up on the daemon surface, which is
    /// a bug that the daemon `dispatch` should have caught upstream.
    pub fn to_protocol(self) -> tend_protocol::ErrorCode {
        use tend_protocol::ErrorCode as P;
        match self {
            Self::NotFound => P::NotFound,
            Self::PathNotFound => P::PathNotFound,
            Self::ProtocolError => P::ProtocolError,
            Self::MessageTooLarge => P::MessageTooLarge,
            Self::Unauthorized => P::Unauthorized,
            _ => P::Internal,
        }
    }
}

/// Errors surfaced across the Tauri command boundary and the daemon socket.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkbenchError {
    /// Machine-readable code.
    pub code: ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional structured details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl WorkbenchError {
    /// Construct with just code + message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// Construct with code + message + structured details.
    pub fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details),
        }
    }

    // ---- Convenience constructors for frequently-raised codes. ----

    /// Generic NOT_FOUND.
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, what)
    }

    /// INTERNAL fallback.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    /// Build a SESSION_READ_ONLY rejection (wrapper-owned session).
    pub fn session_read_only(session_id: i64) -> Self {
        Self::with_details(
            ErrorCode::SessionReadOnly,
            "session is wrapper-owned — input via the launching terminal",
            serde_json::json!({ "session_id": session_id }),
        )
    }
}

impl fmt::Display for WorkbenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for WorkbenchError {}

// ---- Automatic conversions from common error sources. ----

impl From<sqlx::Error> for WorkbenchError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Self::new(ErrorCode::NotFound, "row not found"),
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => Self::new(
                ErrorCode::AlreadyExists,
                format!("unique constraint violated: {db_err}"),
            ),
            other => Self::new(ErrorCode::Internal, format!("database error: {other}")),
        }
    }
}

impl From<std::io::Error> for WorkbenchError {
    fn from(err: std::io::Error) -> Self {
        Self::with_details(
            ErrorCode::Internal,
            format!("io error: {err}"),
            serde_json::json!({ "os_error": err.raw_os_error() }),
        )
    }
}

impl From<serde_json::Error> for WorkbenchError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorCode::Internal, format!("serde_json error: {err}"))
    }
}

/// Convenience alias for `Result<T, WorkbenchError>`.
pub type WorkbenchResult<T> = std::result::Result<T, WorkbenchError>;
