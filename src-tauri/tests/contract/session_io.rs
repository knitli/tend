//! T086: `session_send_input` / `session_resize` / `session_end` contract tests.
//!
//! These tests exercise the ownership gate on the three session I/O Tauri
//! commands. Workbench-owned sessions accept input/resize/end; wrapper-owned
//! sessions reject them with SESSION_READ_ONLY.
//!
//! Written RED-first (TDD gate) — they will turn GREEN once the service
//! methods and Tauri commands are wired.

use agentui_workbench::error::ErrorCode;
use agentui_workbench::project::ProjectService;
use agentui_workbench::session::SessionService;
use std::collections::BTreeMap;

/// Helper: spawn a workbench-owned session backed by a real PTY.
async fn spawn_real_workbench_session(
    state: &agentui_workbench::state::WorkbenchState,
) -> (agentui_workbench::model::SessionId, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project = ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("io-test"))
        .await
        .expect("register project");

    let env = BTreeMap::new();
    let (session, _handle) = SessionService::spawn_local(
        state,
        project.id,
        "io-session",
        tmp.path(),
        &["/bin/sh".to_string(), "-c".to_string(), "sleep 300".to_string()],
        &env,
    )
    .await
    .expect("spawn_local");

    (session.id, tmp)
}

// ---- Happy-path: workbench-owned session accepts I/O ----

/// session_send_input happy path on workbench-owned session.
#[tokio::test]
async fn send_input_happy_path_workbench_owned() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_real_workbench_session(&state).await;

    // Ownership check should pass.
    SessionService::require_workbench_owned(&state.db, session_id)
        .await
        .expect("require_workbench_owned must succeed for workbench session");

    // Write through the live handle.
    let sessions = state.live_sessions.read().await;
    let handle = sessions.get(&session_id).expect("live handle must exist");
    let result = handle.write(b"echo hello\n");
    assert!(result.is_ok(), "write to workbench-owned session must succeed");
}

/// session_resize happy path on workbench-owned session.
#[tokio::test]
async fn resize_happy_path_workbench_owned() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_real_workbench_session(&state).await;

    SessionService::require_workbench_owned(&state.db, session_id)
        .await
        .expect("require_workbench_owned must succeed");

    let sessions = state.live_sessions.read().await;
    let handle = sessions.get(&session_id).expect("live handle must exist");
    let result = handle.resize(120, 40);
    assert!(result.is_ok(), "resize on workbench-owned session must succeed");
}

/// session_end happy path on workbench-owned session.
#[tokio::test]
async fn end_happy_path_workbench_owned() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_real_workbench_session(&state).await;

    SessionService::require_workbench_owned(&state.db, session_id)
        .await
        .expect("require_workbench_owned must succeed");

    let sessions = state.live_sessions.read().await;
    let handle = sessions.get(&session_id).expect("live handle must exist");
    let result = handle.end();
    assert!(result.is_ok(), "end on workbench-owned session must succeed");
}

// ---- Rejection: wrapper-owned session returns SESSION_READ_ONLY ----

/// session_send_input on wrapper-owned session returns SESSION_READ_ONLY.
#[tokio::test]
async fn send_input_wrapper_owned_returns_read_only() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "wrapper-io").await;
    let session_id = crate::common::seed_wrapper_session(&state, project, Some(7000)).await;

    let result = SessionService::require_workbench_owned(&state.db, session_id).await;
    assert!(result.is_err(), "wrapper-owned session must be rejected");
    assert_eq!(
        result.unwrap_err().code,
        ErrorCode::SessionReadOnly,
        "error code must be SESSION_READ_ONLY"
    );
}

/// session_resize on wrapper-owned session returns SESSION_READ_ONLY.
#[tokio::test]
async fn resize_wrapper_owned_returns_read_only() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "wrapper-resize").await;
    let session_id = crate::common::seed_wrapper_session(&state, project, Some(7001)).await;

    let result = SessionService::require_workbench_owned(&state.db, session_id).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, ErrorCode::SessionReadOnly);
}

/// session_end on wrapper-owned session returns SESSION_READ_ONLY.
#[tokio::test]
async fn end_wrapper_owned_returns_read_only() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "wrapper-end").await;
    let session_id = crate::common::seed_wrapper_session(&state, project, Some(7002)).await;

    let result = SessionService::require_workbench_owned(&state.db, session_id).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, ErrorCode::SessionReadOnly);
}

/// Additionally verify that an attached-mirror handle (from LiveSessionHandle)
/// returns SESSION_READ_ONLY on write/resize/end even at the handle level.
#[tokio::test]
async fn mirror_handle_rejects_io() {
    use agentui_workbench::model::SessionId;
    use agentui_workbench::session::live::LiveSessionHandle;

    let handle = LiveSessionHandle::attached_mirror(SessionId::new(1));

    let write_err = handle.write(b"hello").unwrap_err();
    assert_eq!(write_err.code, ErrorCode::SessionReadOnly);

    let resize_err = handle.resize(80, 24).unwrap_err();
    assert_eq!(resize_err.code, ErrorCode::SessionReadOnly);

    let end_err = handle.end().unwrap_err();
    assert_eq!(end_err.code, ErrorCode::SessionReadOnly);
}
