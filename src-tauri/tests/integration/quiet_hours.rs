//! T071: Integration test — quiet hours suppression.
//!
//! When the current local time falls within the configured quiet-hours window,
//! OS notifications must be suppressed while in-app alerts continue to fire.
//!
//! Since we cannot test actual OS notifications in CI, this test validates the
//! preference resolution logic: given a preference row with quiet hours covering
//! "now", the effective channel list must exclude `OsNotification` but include
//! `InApp`.

use agentui_workbench::model::{NotificationChannel, QuietHours};
use agentui_workbench::notifications::PreferenceService;

/// During quiet hours, OsNotification is filtered out but InApp remains.
#[tokio::test]
async fn quiet_hours_suppresses_os_notification_but_emits_in_app() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "quiet-hours-test").await;

    // Build a quiet-hours window that spans the entire day (00:00–23:59)
    // so the test is always "inside" quiet hours regardless of CI timezone.
    let quiet_hours = QuietHours {
        start: "00:00".to_string(),
        end: "23:59".to_string(),
        timezone: "local".to_string(),
    };

    let channels = vec![
        NotificationChannel::OsNotification,
        NotificationChannel::InApp,
        NotificationChannel::TerminalBell,
    ];

    // Seed the preference row for this project (set = upsert).
    PreferenceService::set(&state.db, Some(project_id), &channels, Some(&quiet_hours))
        .await
        .expect("set notification preference");

    // Resolve the effective channels for this project at "now".
    let effective = PreferenceService::resolve_channels(&state.db, project_id)
        .await
        .expect("resolve_channels");

    // OsNotification must be suppressed during quiet hours.
    assert!(
        !effective.contains(&NotificationChannel::OsNotification),
        "OsNotification must be suppressed during quiet hours, got: {effective:?}"
    );

    // InApp must still be present — quiet hours only suppress OS notifications.
    assert!(
        effective.contains(&NotificationChannel::InApp),
        "InApp must remain active during quiet hours, got: {effective:?}"
    );
}

/// Outside quiet hours, all configured channels are returned.
#[tokio::test]
async fn outside_quiet_hours_all_channels_returned() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "no-quiet-hours").await;

    // No quiet hours configured at all.
    let channels = vec![
        NotificationChannel::OsNotification,
        NotificationChannel::InApp,
    ];

    PreferenceService::set(&state.db, Some(project_id), &channels, None)
        .await
        .expect("set preference without quiet hours");

    let effective = PreferenceService::resolve_channels(&state.db, project_id)
        .await
        .expect("resolve_channels");

    assert!(
        effective.contains(&NotificationChannel::OsNotification),
        "OsNotification must be present when no quiet hours are configured"
    );
    assert!(
        effective.contains(&NotificationChannel::InApp),
        "InApp must be present when no quiet hours are configured"
    );
}

/// When all channels are set to Silent, no channels are returned even outside
/// quiet hours.
#[tokio::test]
async fn silent_mode_returns_empty_channels() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "silent-test").await;

    let channels = vec![NotificationChannel::Silent];

    PreferenceService::set(&state.db, Some(project_id), &channels, None)
        .await
        .expect("set silent preference");

    let effective = PreferenceService::resolve_channels(&state.db, project_id)
        .await
        .expect("resolve_channels");

    assert!(
        effective.is_empty() || effective == vec![NotificationChannel::Silent],
        "Silent mode should suppress all delivery channels, got: {effective:?}"
    );
}
