//! `Reminder` domain type (scratchpad).

use super::{ProjectId, ReminderId, Timestamp};
use serde::{Deserialize, Serialize};

/// A checkable per-project reminder.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Reminder {
    /// Surrogate id.
    pub id: ReminderId,
    /// Owning project.
    pub project_id: ProjectId,
    /// Reminder body.
    pub content: String,
    /// Current state (`open` or `done`).
    pub state: ReminderState,
    /// When the reminder was created. Used to compute the age indicator (FR-031).
    pub created_at: Timestamp,
    /// When the reminder was marked `done`. Set iff `state == Done`.
    pub done_at: Option<Timestamp>,
}

/// Whether a reminder is still open or marked done.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReminderState {
    /// Still active.
    Open,
    /// Marked done.
    Done,
}

impl ReminderState {
    /// SQL enum text.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
        }
    }
}

impl std::str::FromStr for ReminderState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Self::Open),
            "done" => Ok(Self::Done),
            other => Err(format!("invalid reminder state: {other}")),
        }
    }
}
