//! `WorkspaceState` in-memory + on-disk representation.

use super::{ProjectId, SessionId};
use serde::{Deserialize, Serialize};

/// Serialized in `workspace_state.payload_json` and in named `layouts`.
///
/// Layout rows share this exact schema plus a `name` column at the row level.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceState {
    /// Version tag on the payload shape (currently 1).
    #[serde(default = "default_version")]
    pub version: u32,

    /// Projects expanded in the sidebar.
    #[serde(default)]
    pub active_project_ids: Vec<ProjectId>,

    /// Session currently shown in the split view (if any).
    #[serde(default)]
    pub focused_session_id: Option<SessionId>,

    /// High-level pane layout token ("split" | "agent_only" | …).
    #[serde(default = "default_pane_layout")]
    pub pane_layout: String,

    /// Arbitrary UI state (sidebar widths, panel toggles, etc.).
    #[serde(default)]
    pub ui: serde_json::Map<String, serde_json::Value>,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            version: 1,
            active_project_ids: Vec::new(),
            focused_session_id: None,
            pane_layout: "split".into(),
            ui: serde_json::Map::new(),
        }
    }
}

const fn default_version() -> u32 {
    1
}

fn default_pane_layout() -> String {
    "split".into()
}
