//! `Session` domain type and associated enums.
//!
//! T013: defines `SessionStatus`, `StatusSource`, and `SessionOwnership`.
//! The last of those is immutable after row creation — enforced in
//! `SessionService`, not at the type level, since Rust doesn't give us an
//! ergonomic once-cell pattern that composes with sqlx derives.

use super::{Alert, Pid, ProjectId, SessionId, Timestamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A session row as stored in the database. Includes the persisted
/// `ownership` column per spec-panel round-1 item #2.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Session {
    /// Surrogate id.
    pub id: SessionId,
    /// Owning project.
    pub project_id: ProjectId,
    /// User-visible session label.
    pub label: String,
    /// OS pid of the agent child process (None before spawn / after exit).
    pub pid: Option<Pid>,
    /// Current status.
    pub status: SessionStatus,
    /// Whether `status` came from cooperative IPC or fallback heuristic.
    pub status_source: StatusSource,
    /// Who owns the PTY master fd for this session. Immutable.
    pub ownership: SessionOwnership,
    /// When the session was created.
    pub started_at: Timestamp,
    /// When the session ended (set iff status is `Ended` or `Error`).
    pub ended_at: Option<Timestamp>,
    /// Monotonically non-decreasing last-activity timestamp (invariant #8).
    pub last_activity_at: Timestamp,
    /// Last successful cooperative-IPC heartbeat (None if never received).
    pub last_heartbeat_at: Option<Timestamp>,
    /// Agent-provided metadata (task title, branch, model, …).
    pub metadata: SessionMetadata,
    /// Working directory the session was spawned in (worktree-aware).
    pub working_directory: PathBuf,
    /// Child exit code when available (set on `Ended`).
    pub exit_code: Option<i32>,
    /// Free-text error reason when status is `Error` (e.g. `"workbench_restart"`).
    pub error_reason: Option<String>,
}

/// Session lifecycle state.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Actively producing output or doing work.
    Working,
    /// Silent beyond the idle threshold.
    Idle,
    /// Blocked on user input — associated with an open [`Alert`].
    NeedsInput,
    /// Child exited normally (exit code 0).
    Ended,
    /// Child exited abnormally or the session errored out.
    Error,
}

impl SessionStatus {
    /// Convert to the SQL enum text value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Idle => "idle",
            Self::NeedsInput => "needs_input",
            Self::Ended => "ended",
            Self::Error => "error",
        }
    }

    /// Is this a "live" status (i.e., the session still has a running pid)?
    pub fn is_live(self) -> bool {
        matches!(self, Self::Working | Self::Idle | Self::NeedsInput)
    }
}

/// Parsing from the SQL enum text column.
impl std::str::FromStr for SessionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "working" => Ok(Self::Working),
            "idle" => Ok(Self::Idle),
            "needs_input" => Ok(Self::NeedsInput),
            "ended" => Ok(Self::Ended),
            "error" => Ok(Self::Error),
            other => Err(format!("invalid session status: {other}")),
        }
    }
}

/// Where a session's current status came from.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StatusSource {
    /// Pushed by a cooperating client over the daemon socket.
    Ipc,
    /// Derived from output-activity or prompt-pattern detection.
    Heuristic,
}

impl StatusSource {
    /// Convert to the SQL enum text value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ipc => "ipc",
            Self::Heuristic => "heuristic",
        }
    }
}

impl std::str::FromStr for StatusSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ipc" => Ok(Self::Ipc),
            "heuristic" => Ok(Self::Heuristic),
            other => Err(format!("invalid status source: {other}")),
        }
    }
}

/// Who owns the PTY master fd for a session. Immutable after row creation.
///
/// Round-1 #2 adopted Option B: wrapper-owned sessions are read-only from the
/// workbench. `require_workbench_owned` in `SessionService` gates
/// `session_send_input` / `session_resize` / `session_end` and returns
/// `SESSION_READ_ONLY` for wrapper-owned rows.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SessionOwnership {
    /// Workbench owns the PTY master; session can be typed into from the UI.
    Workbench,
    /// A CLI wrapper process owns the PTY master; workbench is a read-only mirror.
    Wrapper,
}

impl SessionOwnership {
    /// Convert to the SQL enum text value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workbench => "workbench",
            Self::Wrapper => "wrapper",
        }
    }
}

impl std::str::FromStr for SessionOwnership {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "workbench" => Ok(Self::Workbench),
            "wrapper" => Ok(Self::Wrapper),
            other => Err(format!("invalid session ownership: {other}")),
        }
    }
}

/// Agent-provided session metadata (stored as JSON in `sessions.metadata_json`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadata {
    /// Optional agent-provided task title (shown in the UI if set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_title: Option<String>,
    /// Branch name the session is working on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Model identifier (e.g. `"claude-sonnet-4.5"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Command-line the session was spawned with (for display only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    /// Forward-compat escape hatch.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty", flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// What `session_list` returns: the `Session` record plus derived fields.
///
/// `reattached_mirror` is true iff this handle was created by
/// `reconcile_and_reattach` (T025) on a workbench-owned row that survived a
/// workbench restart. In v1 the original PTY master fd does not survive that
/// restart, so the handle is promoted to a read-only mirror and the frontend
/// renders `AgentPane` with a muted banner.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    /// The persisted session record (includes `ownership`).
    #[serde(flatten)]
    pub session: Session,
    /// Short human-readable activity summary (US4). `None` on cold rows.
    pub activity_summary: Option<String>,
    /// Currently-open alert, if any. Computed in the same SQL read as the session row
    /// so a `session_list` response row is internally consistent (round-1 #4).
    pub alert: Option<Alert>,
    /// True iff reattached from a crash-recovery pass on a workbench-owned session.
    /// The frontend uses `ownership == Workbench && !reattached_mirror` to decide
    /// whether the agent pane is interactive.
    pub reattached_mirror: bool,
}
