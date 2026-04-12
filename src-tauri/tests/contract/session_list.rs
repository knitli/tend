//! T035: `session_list` contract tests.
//!
//! These tests exercise the expected behavior of `SessionService::list`.
//! They are RED by design — the service stub returns Internal errors.
//! They will turn GREEN when T049 implements the real query.

use tend_workbench::session::SessionService;

/// Filtered by project: only sessions for the given project are returned.
#[tokio::test]
async fn list_filtered_by_project() {
    let state = crate::common::mock_state().await;
    let project_a = crate::common::seed_project(&state, "proj-a").await;
    let project_b = crate::common::seed_project(&state, "proj-b").await;

    // Seed sessions under each project.
    crate::common::seed_workbench_session(&state, project_a, Some(1000)).await;
    crate::common::seed_workbench_session(&state, project_a, Some(1001)).await;
    crate::common::seed_workbench_session(&state, project_b, Some(2000)).await;

    let sessions = SessionService::list(&state, Some(project_a), false)
        .await
        .expect("list should succeed");

    assert_eq!(
        sessions.len(),
        2,
        "should return only sessions for project A"
    );
    for s in &sessions {
        assert_eq!(s.session.project_id, project_a);
    }
}

/// include_ended flag: ended sessions are excluded by default but included
/// when the flag is true.
#[tokio::test]
async fn list_include_ended_flag() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "ended-test").await;

    // Seed one live and one ended session.
    crate::common::seed_workbench_session(&state, project, Some(3000)).await;
    let ended_id = crate::common::seed_workbench_session(&state, project, Some(3001)).await;

    // Mark the second session as ended.
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET status = 'ended', ended_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(ended_id.get())
        .execute(state.db.pool())
        .await
        .expect("mark ended");

    let without_ended = SessionService::list(&state, Some(project), false)
        .await
        .expect("list without ended");
    assert_eq!(without_ended.len(), 1, "should exclude ended sessions");

    let with_ended = SessionService::list(&state, Some(project), true)
        .await
        .expect("list with ended");
    assert_eq!(with_ended.len(), 2, "should include ended sessions");
}

/// Status-alert snapshot invariant: a session with needs_input status and an
/// open alert should have a non-null alert field in SessionSummary.
#[tokio::test]
async fn status_alert_snapshot_invariant() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "alert-test").await;
    let session_id = crate::common::seed_workbench_session(&state, project, Some(4000)).await;

    // Set status to needs_input.
    sqlx::query("UPDATE sessions SET status = 'needs_input' WHERE id = ?1")
        .bind(session_id.get())
        .execute(state.db.pool())
        .await
        .expect("set needs_input");

    // Insert an open alert for this session.
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO alerts (session_id, project_id, kind, raised_at)
        VALUES (?1, ?2, 'needs_input', ?3)
        "#,
    )
    .bind(session_id.get())
    .bind(project.get())
    .bind(&now)
    .execute(state.db.pool())
    .await
    .expect("insert alert");

    let sessions = SessionService::list(&state, Some(project), false)
        .await
        .expect("list should succeed");

    assert_eq!(sessions.len(), 1);
    let summary = &sessions[0];
    assert!(
        summary.alert.is_some(),
        "SessionSummary.alert must be non-null when session has an open alert"
    );
}

/// Ownership roundtrip: seed a wrapper session, list, verify ownership == Wrapper.
#[tokio::test]
async fn ownership_roundtrip_wrapper() {
    use tend_workbench::model::SessionOwnership;

    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "ownership-test").await;
    crate::common::seed_wrapper_session(&state, project, Some(5000)).await;

    let sessions = SessionService::list(&state, Some(project), false)
        .await
        .expect("list should succeed");

    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].session.ownership,
        SessionOwnership::Wrapper,
        "wrapper-seeded session must report ownership = Wrapper"
    );
}
