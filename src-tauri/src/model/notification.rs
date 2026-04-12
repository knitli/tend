//! Notification preference types.

use super::{NotificationPreferenceId, ProjectId, Timestamp};
use serde::{Deserialize, Serialize};

/// A stored notification preference row. `project_id = None` is the global default.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NotificationPreference {
    /// Surrogate id.
    pub id: NotificationPreferenceId,
    /// Owning project. `None` means this is the global default.
    pub project_id: Option<ProjectId>,
    /// Ordered list of enabled channels.
    pub channels: Vec<NotificationChannel>,
    /// Optional quiet-hours window.
    pub quiet_hours: Option<QuietHours>,
    /// When the preference was last updated.
    pub updated_at: Timestamp,
}

/// A channel for notification delivery.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    /// In-app alert bar entry.
    InApp,
    /// OS-level notification (Tauri notification plugin).
    OsNotification,
    /// Terminal bell in the active pane.
    TerminalBell,
    /// Suppress all channels including in-app.
    Silent,
}

/// Quiet-hours window. When `now_local ∈ [start, end]` OS notifications are
/// suppressed; the in-app bar still shows per FR-013 semantics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QuietHours {
    /// Local start time as `HH:MM` 24-hour.
    pub start: String,
    /// Local end time as `HH:MM` 24-hour. Wraps across midnight if `end < start`.
    pub end: String,
    /// Timezone key (currently always `"local"`).
    #[serde(default = "default_tz")]
    pub timezone: String,
}

fn default_tz() -> String {
    "local".into()
}
