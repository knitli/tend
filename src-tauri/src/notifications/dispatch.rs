//! T076: Alert dispatch — route alerts to OS notification or in-app per prefs.
//!
//! `dispatch_alert` checks quiet hours and channels, then calls
//! `tauri_plugin_notification::Notification::new` for the OS channel.
//! In-app delivery is handled by the event bridge (`events.rs`), not here.

use crate::db::Database;
use crate::model::{Alert, NotificationChannel};
use crate::notifications::preferences::{PreferenceService, is_quiet_hours};
use tracing::{info, trace};

/// Dispatch an OS notification for an alert according to notification preferences.
///
/// In-app delivery is handled separately by the event bridge (always emitted).
/// This function only handles the OS notification channel, respecting quiet hours
/// and per-project channel configuration.
///
/// Returns `true` if an OS notification was sent.
pub async fn dispatch_alert(db: &Database, app: &tauri::AppHandle, alert: &Alert) -> bool {
    let prefs = match PreferenceService::get(db, Some(alert.project_id)).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("failed to load notification prefs for alert dispatch: {e}");
            return false;
        }
    };

    // Check if any channel is silent (suppresses everything except in-app).
    if prefs.channels.contains(&NotificationChannel::Silent) {
        trace!("alert dispatch: silent channel configured, skipping OS notification");
        return false;
    }

    // Check quiet hours — during quiet hours, suppress OS/bell but not in-app.
    let in_quiet_hours = prefs
        .quiet_hours
        .as_ref()
        .map(is_quiet_hours)
        .unwrap_or(false);

    if in_quiet_hours {
        info!("alert dispatch: in quiet hours, suppressing OS notification");
        return false;
    }

    // Check if OS notification channel is enabled.
    if !prefs
        .channels
        .contains(&NotificationChannel::OsNotification)
    {
        trace!("alert dispatch: OS notification channel not enabled");
        return false;
    }

    // Send OS notification via Tauri plugin.
    let title = "Session needs input";
    let body = match &alert.reason {
        Some(reason) => format!("Session is waiting: {reason}"),
        None => "A session is waiting for your input".to_string(),
    };

    match tauri_plugin_notification::NotificationExt::notification(app)
        .builder()
        .title(title)
        .body(&body)
        .show()
    {
        Ok(_) => {
            info!(alert_id = %alert.id, "OS notification sent");
            true
        }
        Err(e) => {
            tracing::warn!("failed to send OS notification: {e}");
            false
        }
    }
}
