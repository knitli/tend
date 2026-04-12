//! T070: Integration test — alert autoclear on session resume.
//!
//! When a session transitions from `NeedsInput` back to `Working`, the open
//! alert row must be automatically cleared with `cleared_by = 'session_resumed'`
//! and `acknowledged_at IS NOT NULL`.
//!
//! This exercises the H7 autoclear path in `SessionService::set_status`.

use tend_workbench::model::{SessionStatus, StatusSource};
use tend_workbench::session::SessionService;

#[tokio::test]
async fn update_status_working_clears_needs_input_alert() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "alert-autoclear").await;
    let session_id = crate::common::seed_workbench_session(&state, project_id, Some(9999)).await;

    // 1. Transition to NeedsInput — this creates an alert row via H6.
    SessionService::set_status(
        &state.db,
        session_id,
        SessionStatus::NeedsInput,
        StatusSource::Ipc,
        Some("test prompt detected"),
    )
    .await
    .expect("set_status → NeedsInput");

    // Verify the alert row exists and is open.
    let alert_row: (i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT id, acknowledged_at, cleared_by FROM alerts WHERE session_id = ?1 AND acknowledged_at IS NULL",
    )
    .bind(session_id.get())
    .fetch_one(state.db.pool())
    .await
    .expect("open alert must exist after NeedsInput transition");

    let alert_id = alert_row.0;
    assert!(
        alert_row.1.is_none(),
        "alert must be unacknowledged while session is NeedsInput"
    );
    assert!(
        alert_row.2.is_none(),
        "cleared_by must be NULL while alert is open"
    );

    // 2. Subscribe to event bus before the resume transition.
    let mut _rx = state.event_bus.subscribe();

    // 3. Transition back to Working — H7 must clear the alert.
    SessionService::set_status(
        &state.db,
        session_id,
        SessionStatus::Working,
        StatusSource::Ipc,
        None,
    )
    .await
    .expect("set_status → Working");

    // 4. Verify the alert row is now cleared.
    let cleared_row: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT acknowledged_at, cleared_by FROM alerts WHERE id = ?1")
            .bind(alert_id)
            .fetch_one(state.db.pool())
            .await
            .expect("alert row must still exist after clearing");

    assert!(
        cleared_row.0.is_some(),
        "acknowledged_at must be set after session resumes Working"
    );
    assert_eq!(
        cleared_row.1.as_deref(),
        Some("session_resumed"),
        "cleared_by must be 'session_resumed' when auto-cleared by Working transition"
    );

    // 5. Verify the session status is now Working.
    let status_row: (String,) = sqlx::query_as("SELECT status FROM sessions WHERE id = ?1")
        .bind(session_id.get())
        .fetch_one(state.db.pool())
        .await
        .expect("session row must exist");
    assert_eq!(
        status_row.0, "working",
        "session must be in Working status after resume"
    );
}

/// Verify that ending a session also clears any open NeedsInput alert
/// with `cleared_by = 'session_ended'`.
#[tokio::test]
async fn mark_ended_clears_needs_input_alert() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "alert-end-clear").await;
    let session_id = crate::common::seed_workbench_session(&state, project_id, Some(9998)).await;

    // Transition to NeedsInput to create an alert.
    SessionService::set_status(
        &state.db,
        session_id,
        SessionStatus::NeedsInput,
        StatusSource::Ipc,
        Some("blocked on confirmation"),
    )
    .await
    .expect("set_status → NeedsInput");

    // Confirm alert exists.
    let alert_row: (i64,) =
        sqlx::query_as("SELECT id FROM alerts WHERE session_id = ?1 AND acknowledged_at IS NULL")
            .bind(session_id.get())
            .fetch_one(state.db.pool())
            .await
            .expect("open alert must exist");
    let alert_id = alert_row.0;

    // End the session (exit code 0).
    SessionService::mark_ended(&state.db, session_id, Some(0))
        .await
        .expect("mark_ended");

    // The alert should now be cleared with session_ended.
    let cleared: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT acknowledged_at, cleared_by FROM alerts WHERE id = ?1")
            .bind(alert_id)
            .fetch_one(state.db.pool())
            .await
            .expect("alert row must exist");

    assert!(
        cleared.0.is_some(),
        "acknowledged_at must be set after session ends"
    );
    assert_eq!(
        cleared.1.as_deref(),
        Some("session_ended"),
        "cleared_by must be 'session_ended' when session is marked ended"
    );
}
