//! T066: `session_acknowledge_alert` contract tests.
//!
//! These tests exercise the expected behavior of `AlertService::acknowledge`.
//! They are RED by design and will turn GREEN when Phase 4 wires alert
//! acknowledgment end-to-end (service + Tauri command + event emission).

use agentui_workbench::model::AlertId;
use agentui_workbench::notifications::AlertService;

/// Acknowledging an open alert sets `acknowledged_at` and `cleared_by = user`.
#[tokio::test]
async fn acknowledge_clears_open_alert() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "alert-ack").await;
    let session_id = crate::common::seed_workbench_session(&state, project, Some(6000)).await;

    // Set session to needs_input so the alert is semantically valid.
    sqlx::query("UPDATE sessions SET status = 'needs_input' WHERE id = ?1")
        .bind(session_id.get())
        .execute(state.db.pool())
        .await
        .expect("set needs_input");

    // Insert an open alert row.
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO alerts (session_id, project_id, kind, raised_at)
        VALUES (?1, ?2, 'needs_input', ?3)
        RETURNING id
        "#,
    )
    .bind(session_id.get())
    .bind(project.get())
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert alert");
    let alert_id = AlertId::new(row.0);

    // --- Act ---
    AlertService::acknowledge(&state.db, alert_id, session_id)
        .await
        .expect("acknowledge should succeed");

    // --- Assert ---
    let (acked_at, cleared_by): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT acknowledged_at, cleared_by FROM alerts WHERE id = ?1")
            .bind(alert_id.get())
            .fetch_one(state.db.pool())
            .await
            .expect("read back alert");

    assert!(
        acked_at.is_some(),
        "acknowledged_at must be set after acknowledge"
    );
    assert_eq!(
        cleared_by.as_deref(),
        Some("user"),
        "cleared_by must be 'user' when acknowledged by user"
    );
}

/// Acknowledging a nonexistent alert returns NOT_FOUND.
#[tokio::test]
async fn acknowledge_nonexistent_alert_returns_not_found() {
    let state = crate::common::mock_state().await;
    let bogus_id = AlertId::new(999_999);

    let bogus_session = agentui_workbench::model::SessionId::new(999_998);
    let result = AlertService::acknowledge(&state.db, bogus_id, bogus_session).await;

    assert!(
        result.is_err(),
        "acknowledging a nonexistent alert must return an error"
    );
    let err_msg = format!("{}", result.unwrap_err());
    // The error should indicate not-found semantics (case-insensitive check).
    let lower = err_msg.to_lowercase();
    assert!(
        lower.contains("not found") || lower.contains("not_found") || lower.contains("notfound"),
        "error should indicate not-found, got: {err_msg}"
    );
}

/// Acknowledging an already-cleared alert is idempotent (no error).
#[tokio::test]
async fn acknowledge_already_cleared_is_idempotent() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "alert-idem").await;
    let session_id = crate::common::seed_workbench_session(&state, project, Some(6100)).await;

    // Set session to needs_input.
    sqlx::query("UPDATE sessions SET status = 'needs_input' WHERE id = ?1")
        .bind(session_id.get())
        .execute(state.db.pool())
        .await
        .expect("set needs_input");

    // Insert an already-acknowledged alert.
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO alerts (session_id, project_id, kind, raised_at, acknowledged_at, cleared_by)
        VALUES (?1, ?2, 'needs_input', ?3, ?3, 'user')
        RETURNING id
        "#,
    )
    .bind(session_id.get())
    .bind(project.get())
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert pre-acked alert");
    let alert_id = AlertId::new(row.0);

    // --- Act: acknowledge again ---
    let result = AlertService::acknowledge(&state.db, alert_id, session_id).await;

    // --- Assert: no error (idempotent) ---
    assert!(
        result.is_ok(),
        "re-acknowledging an already-cleared alert must be idempotent, got: {:?}",
        result.err()
    );
}
