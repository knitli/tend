//! T085: `session_activate` contract tests.
//!
//! These tests exercise `CompanionService::ensure` — the backing logic behind
//! the `session_activate` Tauri command. The companion terminal is lazily
//! spawned on first activation and reused on subsequent calls.
//!
//! Written RED-first (TDD gate) — they will turn GREEN as the service stubs
//! become real implementations.

use agentui_workbench::companion::CompanionService;
use agentui_workbench::error::ErrorCode;
use agentui_workbench::model::SessionId;
use agentui_workbench::project::ProjectService;
use agentui_workbench::session::SessionService;
use std::collections::BTreeMap;

/// Helper: spawn a workbench-owned session backed by a real PTY in a temp dir.
/// Returns `(SessionId, tempfile::TempDir)` — caller must hold `TempDir` alive.
async fn spawn_real_session(
    state: &agentui_workbench::state::WorkbenchState,
) -> (agentui_workbench::model::SessionId, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project = ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("activate-test"))
        .await
        .expect("register project");

    let env = BTreeMap::new();
    let (session, _handle) = SessionService::spawn_local(
        state,
        project.id,
        "activate-session",
        tmp.path(),
        &["/bin/sh".to_string(), "-c".to_string(), "sleep 300".to_string()],
        &env,
    )
    .await
    .expect("spawn_local");

    (session.id, tmp)
}

/// First activation spawns a companion — a `companion_terminals` row is created.
#[tokio::test]
async fn first_activation_spawns_companion() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_real_session(&state).await;

    let companion = CompanionService::ensure(&state, session_id)
        .await
        .expect("ensure should spawn a companion");

    assert_eq!(companion.session_id, session_id);
    assert!(companion.pid.is_some(), "companion must have a pid after spawn");
    assert!(companion.ended_at.is_none(), "companion must not be ended immediately");

    // Verify a DB row exists.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM companion_terminals WHERE session_id = ?1",
    )
    .bind(session_id.get())
    .fetch_one(state.db.pool())
    .await
    .expect("count companions");
    assert_eq!(count.0, 1, "exactly one companion_terminals row expected");
}

/// Second activation reuses the same companion — same id returned.
#[tokio::test]
async fn second_activation_reuses_companion() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_real_session(&state).await;

    let first = CompanionService::ensure(&state, session_id)
        .await
        .expect("first ensure");
    let second = CompanionService::ensure(&state, session_id)
        .await
        .expect("second ensure");

    assert_eq!(
        first.id, second.id,
        "second activation must return the same companion id"
    );
}

/// A killed companion is respawned transparently on next activation.
#[tokio::test]
async fn killed_companion_respawned_on_next_activation() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_real_session(&state).await;

    let first = CompanionService::ensure(&state, session_id)
        .await
        .expect("first ensure");

    // Kill the companion via the handle.
    {
        let companions = state.live_companions.read().await;
        if let Some(handle) = companions.get(&session_id) {
            let _ = handle.kill();
        }
    }

    // Give the process a moment to exit.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let reborn = CompanionService::ensure(&state, session_id)
        .await
        .expect("ensure after kill should respawn");

    // The companion id may be the same row (updated in-place) but the pid
    // should be fresh.
    assert!(
        reborn.pid.is_some(),
        "respawned companion must have a new pid"
    );
    assert_eq!(
        reborn.id, first.id,
        "respawned companion should reuse the same DB row"
    );
}

/// SESSION_ENDED error for an ended session.
#[tokio::test]
async fn ensure_on_ended_session_returns_session_ended() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "ended-activate").await;
    let session_id = crate::common::seed_workbench_session(&state, project, Some(9000)).await;

    // Mark the session as ended.
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET status = 'ended', ended_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(session_id.get())
        .execute(state.db.pool())
        .await
        .expect("mark ended");

    let result = CompanionService::ensure(&state, session_id).await;
    assert!(result.is_err(), "ensure on ended session must fail");
    assert_eq!(
        result.unwrap_err().code,
        ErrorCode::SessionEnded,
        "error code must be SESSION_ENDED"
    );
}

/// COMPANION_SPAWN_FAILED if working directory does not exist.
#[tokio::test]
async fn ensure_fails_when_cwd_missing() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "bad-cwd").await;
    let session_id = crate::common::seed_workbench_session(&state, project, Some(9100)).await;

    // Point the session at a nonexistent directory.
    sqlx::query("UPDATE sessions SET working_directory = '/nonexistent/agentui-test-dir' WHERE id = ?1")
        .bind(session_id.get())
        .execute(state.db.pool())
        .await
        .expect("update cwd");

    let result = CompanionService::ensure(&state, session_id).await;
    assert!(result.is_err(), "ensure with missing cwd must fail");
    assert_eq!(
        result.unwrap_err().code,
        ErrorCode::CompanionSpawnFailed,
        "error code must be COMPANION_SPAWN_FAILED"
    );
}

/// ensure on a nonexistent session returns NOT_FOUND.
#[tokio::test]
async fn ensure_nonexistent_session_returns_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = SessionId::new(999_999);

    let result = CompanionService::ensure(&state, bogus).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
}
