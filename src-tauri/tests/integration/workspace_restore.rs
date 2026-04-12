//! T120: Integration test — close-and-reopen simulation.
//!
//! Saves workspace state, simulates a restart by building a fresh
//! WorkbenchState from the same DB, runs crash recovery, and verifies
//! still-alive sessions reattach while dead pids are marked ended.

use agentui_workbench::model::WorkspaceState;
use agentui_workbench::session::recovery::reconcile_and_reattach;
use agentui_workbench::state::WorkbenchState as AppState;
use agentui_workbench::workspace::WorkspaceService;

/// Simulate workbench close + reopen with workspace persistence.
#[tokio::test]
async fn workspace_restore_after_crash_recovery() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "ws-restore").await;

    // Spawn a session with our own pid (always alive).
    let alive_sid =
        crate::common::seed_workbench_session(&state, project_id, Some(std::process::id() as i64))
            .await;
    // Spawn a session with a bogus dead pid.
    let dead_sid = crate::common::seed_workbench_session(&state, project_id, Some(999_999)).await;

    // Save workspace state referencing the alive session.
    let ws = WorkspaceState {
        focused_session_id: Some(alive_sid),
        active_project_ids: vec![project_id],
        ..Default::default()
    };
    WorkspaceService::save(&state.db, &ws)
        .await
        .expect("save workspace");

    // Simulate restart: create a new AppState with the same DB.
    let restarted = AppState::new(state.db.clone());

    // Run crash recovery.
    let report = reconcile_and_reattach(&restarted).await.expect("reconcile");

    // The alive session should be reattached.
    assert!(
        report.reattached.contains(&alive_sid),
        "alive session should be reattached"
    );
    // The dead session should be marked ended.
    assert!(
        report.ended.contains(&dead_sid),
        "dead session should be marked ended"
    );

    // Workspace state should still be loadable after restart.
    let loaded = WorkspaceService::get(&restarted.db)
        .await
        .expect("get workspace after restart");
    assert_eq!(loaded.focused_session_id, Some(alive_sid));

    // The reattached session should be in live_sessions.
    let live = restarted.live_sessions.read().await;
    assert!(
        live.contains_key(&alive_sid),
        "reattached session should be in live_sessions"
    );
    assert!(
        !live.contains_key(&dead_sid),
        "dead session should not be in live_sessions"
    );
}
