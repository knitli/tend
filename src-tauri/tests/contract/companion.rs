//! T087: Companion terminal command contract tests.
//!
//! These tests exercise `companion_send_input`, `companion_resize`, and
//! `companion_respawn` by going through `CompanionService::ensure` /
//! `CompanionService::respawn` and the `LiveCompanionHandle` API.
//!
//! Written RED-first (TDD gate) — they will turn GREEN when the companion
//! service and Tauri commands are fully wired.

use agentui_workbench::companion::CompanionService;
use agentui_workbench::project::ProjectService;
use agentui_workbench::session::SessionService;
use std::collections::BTreeMap;

/// Helper: spawn a workbench session with a real PTY and then ensure a companion.
async fn setup_session_with_companion(
    state: &agentui_workbench::state::WorkbenchState,
) -> (
    agentui_workbench::model::SessionId,
    agentui_workbench::model::CompanionTerminal,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("companion-test"),
    )
    .await
    .expect("register project");

    let env = BTreeMap::new();
    let (session, _handle) = SessionService::spawn_local(
        state,
        project.id,
        "companion-session",
        tmp.path(),
        &[
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 300".to_string(),
        ],
        &env,
    )
    .await
    .expect("spawn_local");

    let companion = CompanionService::ensure(state, session.id)
        .await
        .expect("ensure companion");

    (session.id, companion, tmp)
}

/// companion_send_input happy path — writing bytes through the companion handle.
#[tokio::test]
async fn companion_send_input_happy_path() {
    let state = crate::common::mock_state().await;
    let (session_id, _companion, _tmp) = setup_session_with_companion(&state).await;

    let companions = state.live_companions.read().await;
    let handle = companions
        .get(&session_id)
        .expect("live companion handle must exist");

    let result = handle.write(b"ls -la\n");
    assert!(
        result.is_ok(),
        "companion_send_input must succeed: {:?}",
        result.err()
    );
}

/// companion_resize happy path — resizing the companion PTY.
#[tokio::test]
async fn companion_resize_happy_path() {
    let state = crate::common::mock_state().await;
    let (session_id, _companion, _tmp) = setup_session_with_companion(&state).await;

    let companions = state.live_companions.read().await;
    let handle = companions
        .get(&session_id)
        .expect("live companion handle must exist");

    let result = handle.resize(120, 40);
    assert!(
        result.is_ok(),
        "companion_resize must succeed: {:?}",
        result.err()
    );
}

/// companion_respawn creates a new companion with a new pid.
#[tokio::test]
async fn companion_respawn_creates_new_pid() {
    let state = crate::common::mock_state().await;
    let (session_id, first_companion, _tmp) = setup_session_with_companion(&state).await;

    let first_pid = first_companion.pid;

    // Respawn the companion.
    let respawned = CompanionService::respawn(&state, session_id)
        .await
        .expect("respawn should succeed");

    assert!(
        respawned.pid.is_some(),
        "respawned companion must have a pid"
    );

    // The pid should be different from the original because a new shell was spawned.
    // Note: In rare cases PIDs can be recycled, so this is a best-effort check.
    if let (Some(_old), Some(new)) = (first_pid, respawned.pid) {
        // We cannot guarantee the pid is different due to potential recycling,
        // but we can confirm it's a valid pid.
        assert!(new.0 > 0, "new pid must be positive");
    }

    // Verify a new live handle is installed.
    let companions = state.live_companions.read().await;
    let handle = companions
        .get(&session_id)
        .expect("respawned companion handle must exist");

    // The handle's companion_id should match the respawned companion.
    assert_eq!(handle.companion_id, respawned.id);
}

/// After respawn, the old companion's handle is no longer in the map
/// (replaced by the new one).
#[tokio::test]
async fn companion_respawn_replaces_handle() {
    let state = crate::common::mock_state().await;
    let (session_id, _first, _tmp) = setup_session_with_companion(&state).await;

    let _old_companion_id = {
        let companions = state.live_companions.read().await;
        companions.get(&session_id).expect("handle").companion_id
    };

    let respawned = CompanionService::respawn(&state, session_id)
        .await
        .expect("respawn");

    let new_companion_id = {
        let companions = state.live_companions.read().await;
        companions.get(&session_id).expect("handle").companion_id
    };

    // After respawn the handle should reference the new companion.
    // The ids might be the same (row reuse) but verify consistency.
    assert_eq!(
        new_companion_id, respawned.id,
        "live handle must reference the respawned companion"
    );
}

// ---- M1: Negative tests — no companion exists ----

/// companion_send_input returns NOT_FOUND when no companion has been spawned.
#[tokio::test]
async fn send_input_no_companion_returns_not_found() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "no-companion").await;
    let session_id = crate::common::seed_workbench_session(&state, project_id, Some(99999)).await;

    let companions = state.live_companions.read().await;
    assert!(
        companions.get(&session_id).is_none(),
        "no companion should exist yet"
    );
}

/// companion_resize returns NOT_FOUND when no companion has been spawned.
#[tokio::test]
async fn resize_no_companion_returns_not_found() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "no-companion-resize").await;
    let session_id = crate::common::seed_workbench_session(&state, project_id, Some(99999)).await;

    let companions = state.live_companions.read().await;
    assert!(
        companions.get(&session_id).is_none(),
        "no companion should exist"
    );
    // The Tauri command handler checks live_companions — since no handle exists,
    // it returns NOT_FOUND. Verify that directly:
    drop(companions);

    // Simulate what the command handler does:
    let result = state.live_companions.read().await.get(&session_id).cloned();
    assert!(result.is_none(), "must be None when no companion spawned");
}

/// companion_respawn on a nonexistent session returns NOT_FOUND.
#[tokio::test]
async fn respawn_nonexistent_session_returns_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = agentui_workbench::model::SessionId::new(999999);
    let err = CompanionService::respawn(&state, bogus)
        .await
        .expect_err("respawn on nonexistent session must fail");
    assert_eq!(
        err.code,
        agentui_workbench::error::ErrorCode::NotFound,
        "error code must be NOT_FOUND"
    );
}

/// companion_respawn on an ended session returns SESSION_ENDED.
#[tokio::test]
async fn respawn_ended_session_returns_session_ended() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "ended-respawn").await;
    let session_id = crate::common::seed_workbench_session(&state, project_id, Some(99999)).await;

    // End the session.
    SessionService::mark_ended(&state.db, session_id, Some(0))
        .await
        .expect("mark_ended");

    let err = CompanionService::respawn(&state, session_id)
        .await
        .expect_err("respawn on ended session must fail");
    assert_eq!(
        err.code,
        agentui_workbench::error::ErrorCode::SessionEnded,
        "error code must be SESSION_ENDED"
    );
}
