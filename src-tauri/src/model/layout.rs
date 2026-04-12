//! `Layout` domain type — user-named snapshot of a `WorkspaceState`.

use super::{LayoutId, Timestamp, WorkspaceState};
use serde::{Deserialize, Serialize};

/// A named, user-saved workspace layout.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    /// Surrogate id.
    pub id: LayoutId,
    /// User-supplied name. Unique across all layouts.
    pub name: String,
    /// The snapshot payload.
    pub payload: WorkspaceState,
    /// When the layout was created.
    pub created_at: Timestamp,
    /// When the layout was last overwritten.
    pub updated_at: Timestamp,
}
