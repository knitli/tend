//! T037: `register_session` daemon IPC contract tests.

use tend_protocol::{Request, Response};
use tend_workbench::daemon::handlers::dispatch;

/// Happy path: register_session creates a project + session with
/// ownership=wrapper. Returns SessionRegistered with valid ids.
#[tokio::test]
async fn register_session_happy_path() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let path_str = tmp.path().to_str().expect("utf-8 path").to_string();

    let req = Request::RegisterSession {
        project_path: path_str.clone(),
        label: Some("test-session".into()),
        working_directory: Some(path_str),
        command: Some(vec!["echo".into(), "hello".into()]),
        pid: 12345,
        metadata: None,
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::SessionRegistered {
            session_id,
            project_id,
        } => {
            assert!(session_id > 0, "session_id must be positive");
            assert!(project_id > 0, "project_id must be positive");

            // Verify the session row has ownership=wrapper in the DB.
            let row: (String,) = sqlx::query_as("SELECT ownership FROM sessions WHERE id = ?1")
                .bind(session_id)
                .fetch_one(state.db.pool())
                .await
                .expect("query session ownership");
            assert_eq!(
                row.0, "wrapper",
                "registered session must have ownership = wrapper"
            );
        }
        Response::Err { code, message, .. } => {
            panic!("expected SessionRegistered, got Err({code:?}): {message}");
        }
        other => panic!("expected SessionRegistered, got {other:?}"),
    }
}

/// PATH_NOT_FOUND: register_session with a non-existent project_path returns
/// the correct error.
#[tokio::test]
async fn register_session_path_not_found() {
    let state = crate::common::mock_state().await;

    let req = Request::RegisterSession {
        project_path: "/tmp/tend-nonexistent-path-xyz-42".into(),
        label: None,
        working_directory: None,
        command: None,
        pid: 99999,
        metadata: None,
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Err { code, .. } => {
            assert_eq!(
                code,
                tend_protocol::ErrorCode::PathNotFound,
                "non-existent path should return PATH_NOT_FOUND"
            );
        }
        Response::SessionRegistered { .. } => {
            panic!("expected PATH_NOT_FOUND error, got SessionRegistered");
        }
        other => panic!("expected Err, got {other:?}"),
    }
}
