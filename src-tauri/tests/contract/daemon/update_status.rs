//! T038: `update_status` daemon IPC contract tests.

use agentui_protocol::{Request, Response, SessionStatusWire};
use agentui_workbench::daemon::handlers::dispatch;

/// Status enum validation: working, idle, and needs_input are all accepted.
#[tokio::test]
async fn update_status_valid_statuses() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "status-test").await;
    let session_id = crate::common::seed_wrapper_session(&state, project_id, Some(7000)).await;

    let statuses = [
        SessionStatusWire::Working,
        SessionStatusWire::Idle,
        SessionStatusWire::NeedsInput,
    ];

    for status in statuses {
        let req = Request::UpdateStatus {
            session_id: session_id.get(),
            status,
            reason: None,
            summary: None,
        };

        let resp = dispatch(req, &state).await;

        match resp {
            Response::Ack => {
                // Expected — status update accepted.
            }
            Response::Err { code, message, .. } => {
                panic!("expected Ack for {status:?}, got Err({code:?}): {message}");
            }
            other => panic!("expected Ack, got {other:?}"),
        }
    }
}

/// NOT_FOUND: update_status for an unknown session_id returns NOT_FOUND.
#[tokio::test]
async fn update_status_not_found() {
    let state = crate::common::mock_state().await;

    let req = Request::UpdateStatus {
        session_id: 999_999,
        status: SessionStatusWire::Working,
        reason: None,
        summary: None,
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Err { code, .. } => {
            assert_eq!(
                code,
                agentui_protocol::ErrorCode::NotFound,
                "unknown session_id should return NOT_FOUND"
            );
        }
        Response::Ack => {
            panic!("expected NOT_FOUND error for unknown session, got Ack");
        }
        other => panic!("expected Err, got {other:?}"),
    }
}
