//! Tauri command handlers for notifications and alert management.
//!
//! T078: `notification_preference_get`, `notification_preference_set`,
//! `session_acknowledge_alert`.

use crate::error::WorkbenchError;
use crate::model::{AlertId, NotificationChannel, ProjectId, QuietHours};
use crate::notifications::{AlertService, PreferenceService};
use crate::state::WorkbenchState;
use serde::Deserialize;
use tauri::State;

/// Args for `notification_preference_get`.
#[derive(Deserialize)]
pub struct NotificationPreferenceGetArgs {
    /// Project id for project-specific prefs. Omit for global default.
    pub project_id: Option<i64>,
}

/// Get notification preferences (project-specific or global).
#[tauri::command]
pub async fn notification_preference_get(
    state: State<'_, WorkbenchState>,
    args: NotificationPreferenceGetArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project_id = args.project_id.map(ProjectId::new);
    let pref = PreferenceService::get(&state.db, project_id).await?;
    Ok(serde_json::json!({ "preference": pref }))
}

/// Args for `notification_preference_set`.
#[derive(Deserialize)]
pub struct NotificationPreferenceSetArgs {
    /// Project id for project-specific prefs. Omit for global default.
    pub project_id: Option<i64>,
    /// Channels to enable.
    pub channels: Vec<NotificationChannel>,
    /// Optional quiet hours window.
    pub quiet_hours: Option<QuietHours>,
}

/// Set notification preferences (project-specific or global).
#[tauri::command]
pub async fn notification_preference_set(
    state: State<'_, WorkbenchState>,
    args: NotificationPreferenceSetArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project_id = args.project_id.map(ProjectId::new);
    let pref = PreferenceService::set(
        &state.db,
        project_id,
        &args.channels,
        args.quiet_hours.as_ref(),
    )
    .await?;
    Ok(serde_json::json!({ "preference": pref }))
}

/// Args for `session_acknowledge_alert`.
#[derive(Deserialize)]
pub struct SessionAcknowledgeAlertArgs {
    /// The session that owns the alert (used for ownership verification).
    pub session_id: i64,
    /// The alert id to acknowledge.
    pub alert_id: i64,
}

/// Acknowledge (clear) an alert by user action.
#[tauri::command]
pub async fn session_acknowledge_alert(
    state: State<'_, WorkbenchState>,
    args: SessionAcknowledgeAlertArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let alert_id = AlertId::new(args.alert_id);
    let session_id = crate::model::SessionId::new(args.session_id);
    AlertService::acknowledge(&state.db, alert_id, session_id).await?;

    // Emit alert:cleared event.
    let _ = state
        .event_bus
        .send(crate::state::SessionEventEnvelope::AlertCleared {
            alert_id,
            by: crate::model::AlertClearedBy::User,
        });

    Ok(serde_json::json!({}))
}
