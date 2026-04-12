//! `Project` domain type.

use super::{ProjectId, Timestamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A registered project (repo root).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    /// Surrogate id.
    pub id: ProjectId,
    /// Canonical (absolute, symlink-resolved) path to the project root.
    pub canonical_path: PathBuf,
    /// User-editable display name.
    pub display_name: String,
    /// When the project was first registered.
    pub added_at: Timestamp,
    /// When any session in this project last showed activity.
    pub last_active_at: Option<Timestamp>,
    /// When the project was archived (soft-deleted). `None` for active projects.
    pub archived_at: Option<Timestamp>,
    /// Per-project freeform settings.
    pub settings: ProjectSettings,
}

/// Per-project settings (currently a freeform bag; typed fields accumulate here).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    /// Session-row retention window in days (default 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,

    /// Additional arbitrary settings (forward-compat escape hatch).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty", flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
