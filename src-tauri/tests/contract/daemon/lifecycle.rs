//! T039: daemon IPC lifecycle contract tests (heartbeat + end_session).

use tend_protocol::{Request, Response};
use tend_workbench::daemon::handlers::dispatch;

/// Heartbeat updates last_heartbeat_at on a live session.
#[tokio::test]
async fn heartbeat_updates_timestamp() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "heartbeat-test").await;
    let session_id = crate::common::seed_wrapper_session(&state, project_id, Some(8000)).await;

    // Record the initial last_heartbeat_at (should be NULL from seeder).
    let before: (Option<String>,) =
        sqlx::query_as("SELECT last_heartbeat_at FROM sessions WHERE id = ?1")
            .bind(session_id.get())
            .fetch_one(state.db.pool())
            .await
            .expect("query before heartbeat");
    assert!(
        before.0.is_none(),
        "last_heartbeat_at should be NULL before first heartbeat"
    );

    let req = Request::Heartbeat {
        session_id: session_id.get(),
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Ack => {
            // Verify last_heartbeat_at was updated.
            let after: (Option<String>,) =
                sqlx::query_as("SELECT last_heartbeat_at FROM sessions WHERE id = ?1")
                    .bind(session_id.get())
                    .fetch_one(state.db.pool())
                    .await
                    .expect("query after heartbeat");
            assert!(
                after.0.is_some(),
                "last_heartbeat_at should be set after heartbeat"
            );
        }
        Response::Err { code, message, .. } => {
            panic!("expected Ack for heartbeat, got Err({code:?}): {message}");
        }
        other => panic!("expected Ack, got {other:?}"),
    }
}

/// Heartbeat for unknown session returns NOT_FOUND.
#[tokio::test]
async fn heartbeat_not_found() {
    let state = crate::common::mock_state().await;

    let req = Request::Heartbeat {
        session_id: 999_999,
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Err { code, .. } => {
            assert_eq!(
                code,
                tend_protocol::ErrorCode::NotFound,
                "heartbeat for unknown session should return NOT_FOUND"
            );
        }
        Response::Ack => {
            panic!("expected NOT_FOUND for unknown session, got Ack");
        }
        other => panic!("expected Err, got {other:?}"),
    }
}

/// EndSession marks the session as ended with the given exit code.
#[tokio::test]
async fn end_session_marks_ended() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "end-test").await;
    let session_id = crate::common::seed_wrapper_session(&state, project_id, Some(9000)).await;

    let req = Request::EndSession {
        session_id: session_id.get(),
        exit_code: Some(0),
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Ack => {
            // Verify the session is now ended with the correct exit code.
            // exit_code=0 → status="ended"; non-zero → status="error".
            let row: (String, Option<i32>, Option<String>) =
                sqlx::query_as("SELECT status, exit_code, ended_at FROM sessions WHERE id = ?1")
                    .bind(session_id.get())
                    .fetch_one(state.db.pool())
                    .await
                    .expect("query session after end");

            assert_eq!(
                row.0, "ended",
                "session status should be 'ended' for exit_code=0"
            );
            assert_eq!(row.1, Some(0), "exit_code should be 0");
            assert!(row.2.is_some(), "ended_at should be set");
        }
        Response::Err { code, message, .. } => {
            panic!("expected Ack for end_session, got Err({code:?}): {message}");
        }
        other => panic!("expected Ack, got {other:?}"),
    }
}

/// EndSession for unknown session returns NOT_FOUND.
#[tokio::test]
async fn end_session_not_found() {
    let state = crate::common::mock_state().await;

    let req = Request::EndSession {
        session_id: 999_999,
        exit_code: None,
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Err { code, .. } => {
            assert_eq!(
                code,
                tend_protocol::ErrorCode::NotFound,
                "end_session for unknown session should return NOT_FOUND"
            );
        }
        Response::Ack => {
            panic!("expected NOT_FOUND for unknown session, got Ack");
        }
        other => panic!("expected Err, got {other:?}"),
    }
}
