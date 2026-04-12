//! `Note` domain type (scratchpad).

use super::{NoteId, ProjectId, Timestamp};
use serde::{Deserialize, Serialize};

/// A plain-text note attached to a project scratchpad.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Note {
    /// Surrogate id.
    pub id: NoteId,
    /// Owning project.
    pub project_id: ProjectId,
    /// Note body (plain text; the frontend renders light inline markdown).
    pub content: String,
    /// When the note was first created.
    pub created_at: Timestamp,
    /// When the note was last edited (monotonically non-decreasing — see invariant #8).
    pub updated_at: Timestamp,
}
