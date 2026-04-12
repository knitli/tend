//! `CompanionTerminal` domain type.

use super::{CompanionId, Pid, SessionId, Timestamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A companion shell paired with a session. Lazily spawned on first activation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompanionTerminal {
    /// Surrogate id.
    pub id: CompanionId,
    /// The session this companion belongs to (1:1).
    pub session_id: SessionId,
    /// Current pid (None when not spawned).
    pub pid: Option<Pid>,
    /// Resolved shell path (respects `$SHELL`, falls back to `/bin/sh`).
    pub shell_path: PathBuf,
    /// Working directory the companion was spawned in. Matches session's
    /// `working_directory` at pairing time (worktree-aware).
    pub initial_cwd: PathBuf,
    /// Most recent spawn time.
    pub started_at: Timestamp,
    /// Previous spawn's exit time; cleared on respawn.
    pub ended_at: Option<Timestamp>,
}
