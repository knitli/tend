//! T131: Contract test — `SessionSummary` returned by `session_list` includes
//! `activity_summary: String | null` and, when the session has an open alert,
//! `alert: Alert | null`.

#[path = "../common/mod.rs"]
mod common;

use common::{mock_state, seed_project};

use tend_workbench::model::SessionId;
use tend_workbench::session::SessionService;

/// After inserting a session and feeding output into its live handle's activity
/// summary, `session_list` should return the derived `activity_summary`.
#[tokio::test]
async fn session_list_includes_activity_summary() {
    let state = mock_state().await;
    let project_id = seed_project(&state, "activity-test").await;

    // Insert a session row.
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO sessions (
            project_id, label, status, status_source, ownership,
            started_at, last_activity_at, metadata_json, working_directory
        ) VALUES (?1, 'test', 'working', 'heuristic', 'workbench', ?2, ?2, '{}', '/tmp')
        RETURNING id
        "#,
    )
    .bind(project_id.get())
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert session");
    let sid = SessionId::new(row.0);

    // No live session handle yet — summary should be None.
    let summaries = SessionService::list(&state, None, false)
        .await
        .expect("list");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session.id, sid);
    assert_eq!(summaries[0].activity_summary, None);

    // Install a live session handle with some activity.
    let handle = tend_workbench::session::live::LiveSessionHandle::attached_mirror(sid);
    handle
        .activity
        .lock()
        .await
        .record_chunk(b"Compiling project...\n");

    state.live_sessions.write().await.insert(sid, handle);

    // Now session_list should return the activity summary.
    let summaries = SessionService::list(&state, None, false)
        .await
        .expect("list");
    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].activity_summary,
        Some("Compiling project...".to_string())
    );
}

/// When a session has an open alert, `session_list` should include it.
#[tokio::test]
async fn session_list_includes_alert() {
    let state = mock_state().await;
    let project_id = seed_project(&state, "alert-test").await;

    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO sessions (
            project_id, label, status, status_source, ownership,
            started_at, last_activity_at, metadata_json, working_directory
        ) VALUES (?1, 'needs-input-session', 'needs_input', 'ipc', 'workbench', ?2, ?2, '{}', '/tmp')
        RETURNING id
        "#,
    )
    .bind(project_id.get())
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert session");
    let sid = SessionId::new(row.0);

    // Raise an alert.
    sqlx::query(
        r#"
        INSERT INTO alerts (session_id, project_id, kind, reason, raised_at)
        VALUES (?1, ?2, 'needs_input', 'waiting for confirmation', ?3)
        "#,
    )
    .bind(sid.get())
    .bind(project_id.get())
    .bind(&now)
    .execute(state.db.pool())
    .await
    .expect("insert alert");

    let summaries = SessionService::list(&state, None, false)
        .await
        .expect("list");
    assert_eq!(summaries.len(), 1);
    let alert = summaries[0]
        .alert
        .as_ref()
        .expect("alert should be present");
    assert_eq!(alert.reason.as_deref(), Some("waiting for confirmation"));
}

/// `session_list` with no open alert should return `alert: None`.
#[tokio::test]
async fn session_list_alert_none_when_no_alert() {
    let state = mock_state().await;
    let project_id = seed_project(&state, "no-alert").await;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO sessions (
            project_id, label, status, status_source, ownership,
            started_at, last_activity_at, metadata_json, working_directory
        ) VALUES (?1, 'clean-session', 'working', 'heuristic', 'workbench', ?2, ?2, '{}', '/tmp')
        "#,
    )
    .bind(project_id.get())
    .bind(&now)
    .execute(state.db.pool())
    .await
    .expect("insert session");

    let summaries = SessionService::list(&state, None, false)
        .await
        .expect("list");
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].alert.is_none());
    assert_eq!(summaries[0].activity_summary, None);
}

/// H5: `get_by_id` also populates `activity_summary` from the live handle.
#[tokio::test]
async fn get_by_id_includes_activity_summary() {
    let state = mock_state().await;
    let project_id = seed_project(&state, "getbyid-test").await;

    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO sessions (
            project_id, label, status, status_source, ownership,
            started_at, last_activity_at, metadata_json, working_directory
        ) VALUES (?1, 'get-test', 'working', 'heuristic', 'workbench', ?2, ?2, '{}', '/tmp')
        RETURNING id
        "#,
    )
    .bind(project_id.get())
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert session");
    let sid = SessionId::new(row.0);

    // Install a live handle with activity.
    let handle = tend_workbench::session::live::LiveSessionHandle::attached_mirror(sid);
    handle
        .activity
        .lock()
        .await
        .record_chunk(b"Building crate...\n");
    state.live_sessions.write().await.insert(sid, handle);

    let summary = SessionService::get_by_id(&state, sid)
        .await
        .expect("get_by_id");
    assert_eq!(
        summary.activity_summary,
        Some("Building crate...".to_string())
    );
}
