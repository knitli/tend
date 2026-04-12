//! `Alert` domain type.

use super::{AlertId, ProjectId, SessionId, Timestamp};
use serde::{Deserialize, Serialize};

/// A raised `needs_input` alert on a session.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Alert {
    /// Surrogate id.
    pub id: AlertId,
    /// The session that raised the alert.
    pub session_id: SessionId,
    /// Denormalized project id (for overview queries).
    pub project_id: ProjectId,
    /// Alert kind; v1 has only `needs_input`.
    pub kind: AlertKind,
    /// Optional human-readable reason.
    pub reason: Option<String>,
    /// When the alert was first raised.
    pub raised_at: Timestamp,
    /// When the alert was acknowledged (cleared). `None` while open.
    pub acknowledged_at: Option<Timestamp>,
    /// Who cleared the alert, if acknowledged.
    pub cleared_by: Option<AlertClearedBy>,
}

/// What kind of alert this is.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    /// Session is blocked waiting for user input.
    NeedsInput,
}

impl AlertKind {
    /// SQL enum text.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsInput => "needs_input",
        }
    }
}

/// Who cleared an alert.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AlertClearedBy {
    /// User explicitly acknowledged it in the UI.
    User,
    /// Session resumed `working` — auto-cleared.
    SessionResumed,
    /// Session ended — auto-cleared.
    SessionEnded,
}

impl AlertClearedBy {
    /// SQL enum text.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::SessionResumed => "session_resumed",
            Self::SessionEnded => "session_ended",
        }
    }
}

impl std::str::FromStr for AlertClearedBy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "session_resumed" => Ok(Self::SessionResumed),
            "session_ended" => Ok(Self::SessionEnded),
            other => Err(format!("invalid alert cleared_by: {other}")),
        }
    }
}
