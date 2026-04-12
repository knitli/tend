//! T067: `notification_preference_get` + `notification_preference_set` contract tests.
//!
//! These tests exercise the expected behavior of `PreferenceService`.
//! They are RED by design and will turn GREEN when Phase 4 wires the
//! preference Tauri commands end-to-end.

use tend_workbench::model::{NotificationChannel, QuietHours};
use tend_workbench::notifications::PreferenceService;

/// Getting the global default returns a preference (auto-created if absent).
#[tokio::test]
async fn get_global_default_returns_preference() {
    let state = crate::common::mock_state().await;

    // --- Act ---
    let pref = PreferenceService::get(&state.db, None)
        .await
        .expect("get global default should succeed");

    // --- Assert ---
    assert!(
        pref.project_id.is_none(),
        "global default must have project_id = None"
    );
    // Channels should have at least InApp by default.
    assert!(
        !pref.channels.is_empty(),
        "global default should have at least one channel enabled"
    );
}

/// Set a per-project preference and get it back with matching fields.
#[tokio::test]
async fn set_and_get_project_preference() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "pref-proj").await;

    let channels = vec![
        NotificationChannel::InApp,
        NotificationChannel::OsNotification,
    ];
    let quiet = QuietHours {
        start: "22:00".into(),
        end: "07:00".into(),
        timezone: "local".into(),
    };

    // --- Act: set ---
    PreferenceService::set(&state.db, Some(project), &channels, Some(&quiet))
        .await
        .expect("set project preference should succeed");

    // --- Act: get ---
    let pref = PreferenceService::get(&state.db, Some(project))
        .await
        .expect("get project preference should succeed");

    // --- Assert ---
    assert_eq!(pref.project_id, Some(project));
    assert_eq!(pref.channels, channels);
    assert_eq!(
        pref.quiet_hours.as_ref().map(|q| q.start.as_str()),
        Some("22:00"),
    );
    assert_eq!(
        pref.quiet_hours.as_ref().map(|q| q.end.as_str()),
        Some("07:00"),
    );
}

/// When no project-specific preference exists, get falls back to the global default.
#[tokio::test]
async fn project_fallback_to_global() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "pref-fallback").await;

    // Set a global default with known channels.
    let global_channels = vec![
        NotificationChannel::InApp,
        NotificationChannel::TerminalBell,
    ];
    PreferenceService::set(&state.db, None, &global_channels, None)
        .await
        .expect("set global preference should succeed");

    // --- Act: get for a project that has no override ---
    let pref = PreferenceService::get(&state.db, Some(project))
        .await
        .expect("get should fall back to global");

    // --- Assert: should return the global channels ---
    assert_eq!(
        pref.channels, global_channels,
        "project with no override must inherit global channels"
    );
}

/// Setting channels twice for the same project updates in-place (upsert).
#[tokio::test]
async fn set_channels_updates_existing() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "pref-upsert").await;

    // First set.
    PreferenceService::set(
        &state.db,
        Some(project),
        &[NotificationChannel::InApp],
        None,
    )
    .await
    .expect("first set");

    // Second set with different channels.
    let updated_channels = vec![
        NotificationChannel::OsNotification,
        NotificationChannel::TerminalBell,
    ];
    PreferenceService::set(&state.db, Some(project), &updated_channels, None)
        .await
        .expect("second set");

    // --- Assert: only one row, latest channels ---
    let pref = PreferenceService::get(&state.db, Some(project))
        .await
        .expect("get after upsert");

    assert_eq!(
        pref.channels, updated_channels,
        "channels must reflect the latest set call"
    );

    // Verify exactly one row for this project.
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notification_preferences WHERE project_id = ?1")
            .bind(project.get())
            .fetch_one(state.db.pool())
            .await
            .expect("count rows");

    assert_eq!(
        count.0, 1,
        "upsert must leave exactly one preference row per project"
    );
}
