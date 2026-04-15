//! M2: Integration test for session_end full lifecycle.
//!
//! Spawns a session, calls session_end (via the service layer), waits for the
//! child to exit, and verifies the DB row transitions to "ended" and the
//! companion (if any) is cleaned up.

#[path = "../common/mod.rs"]
mod common;

use sqlx::Row;
use std::collections::BTreeMap;
use tend_workbench::companion::CompanionService;
use tend_workbench::model::SessionId;
use tend_workbench::project::ProjectService;
use tend_workbench::session::SessionService;
use tend_workbench::session::live::KillSignal;

/// Helper: spawn a real session with a long-running child.
async fn spawn_test_session(
    state: &tend_workbench::state::WorkbenchState,
) -> (SessionId, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project =
        ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("end-test"))
            .await
            .expect("register project");

    let env = BTreeMap::new();
    let (session, _handle) = SessionService::spawn_local(
        state,
        project.id,
        "end-lifecycle",
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

    (session.id, tmp)
}

/// Full lifecycle: spawn → end → verify DB status = ended.
#[tokio::test]
async fn session_end_transitions_to_ended() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_test_session(&state).await;

    // Verify session is working.
    let row = sqlx::query("SELECT status FROM sessions WHERE id = ?1")
        .bind(session_id.get())
        .fetch_one(state.db.pool())
        .await
        .expect("fetch");
    let status: String = row.try_get("status").expect("get");
    assert_eq!(status, "working");

    // Spawn the reaper BEFORE sending the kill signal so it's subscribed
    // to the event bus when the Ended event fires. Otherwise, under CPU
    // pressure the supervisor can emit Ended before the reaper subscribes
    // and the event is dropped (broadcast channels do not retain history
    // for late subscribers).
    tend_workbench::session::reaper::spawn_reaper(state.clone());

    // Send TERM signal via the handle.
    {
        let live = state.live_sessions.read().await;
        let handle = live.get(&session_id).expect("live handle");
        handle.end(KillSignal::Term).expect("end");
    }

    // Poll DB until status changes or timeout.
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let row = sqlx::query("SELECT status FROM sessions WHERE id = ?1")
            .bind(session_id.get())
            .fetch_one(state.db.pool())
            .await
            .expect("fetch");
        let status: String = row.try_get("status").expect("get");

        if status == "ended" || status == "error" {
            // Session has been marked ended by the reaper.
            break;
        }

        if tokio::time::Instant::now() > deadline {
            panic!("session did not transition to ended within 5 seconds, status: {status}");
        }
    }

    // Verify the session handle was removed from live_sessions.
    assert!(
        state.live_sessions.read().await.get(&session_id).is_none(),
        "live handle must be removed after end"
    );
}

/// Full lifecycle with companion: spawn session + companion → end session →
/// companion cleaned up.
#[tokio::test]
async fn session_end_cleans_up_companion() {
    let state = crate::common::mock_state().await;
    let (session_id, _tmp) = spawn_test_session(&state).await;

    // Ensure a companion exists.
    let _companion = CompanionService::ensure(&state, session_id)
        .await
        .expect("ensure companion");

    assert!(
        state
            .live_companions
            .read()
            .await
            .get(&session_id)
            .is_some(),
        "companion handle must exist"
    );

    // Start the reaper BEFORE sending the kill signal so it's subscribed
    // to the event bus when the Ended event fires.
    tend_workbench::session::reaper::spawn_reaper(state.clone());

    // End the session.
    {
        let live = state.live_sessions.read().await;
        let handle = live.get(&session_id).expect("live handle");
        handle.end(KillSignal::Kill).expect("end");
    }

    // Wait for reaper to process.
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let row = sqlx::query("SELECT status FROM sessions WHERE id = ?1")
            .bind(session_id.get())
            .fetch_one(state.db.pool())
            .await
            .expect("fetch");
        let status: String = row.try_get("status").expect("get");

        if status == "ended" || status == "error" {
            break;
        }

        if tokio::time::Instant::now() > deadline {
            panic!("session did not end within 5 seconds, status: {status}");
        }
    }

    // Give the companion cleanup a moment to propagate.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify companion handle was removed (C2 fix: reaper cleans up companions).
    assert!(
        state
            .live_companions
            .read()
            .await
            .get(&session_id)
            .is_none(),
        "companion handle must be removed after session end"
    );

    // Verify companion DB row is marked ended.
    let comp_row = sqlx::query("SELECT ended_at FROM companion_terminals WHERE session_id = ?1")
        .bind(session_id.get())
        .fetch_optional(state.db.pool())
        .await
        .expect("fetch companion");

    if let Some(row) = comp_row {
        let ended_at: Option<String> = row.try_get("ended_at").expect("get");
        assert!(
            ended_at.is_some(),
            "companion ended_at must be set after session end"
        );
    }
}
