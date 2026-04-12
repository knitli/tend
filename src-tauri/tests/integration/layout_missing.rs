//! T121: Integration test — layout_restore with a dead session reference
//! reports it in missing_sessions.

use agentui_workbench::model::WorkspaceState;
use agentui_workbench::workspace::layouts::LayoutService;

/// Restoring a layout that references an ended session includes that id in
/// missing_sessions so the UI can show a "not running" badge.
#[tokio::test]
async fn layout_restore_reports_missing_dead_sessions() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "layout-missing").await;

    // Create a session with a dead pid.
    let dead_sid = crate::common::seed_workbench_session(&state, project_id, Some(999_999)).await;

    // Save a layout referencing that session.
    let ws = WorkspaceState {
        focused_session_id: Some(dead_sid),
        active_project_ids: vec![project_id],
        ..Default::default()
    };
    let layout = LayoutService::save(&state.db, "missing-test", &ws, false)
        .await
        .expect("save");

    // Restore — the dead session is NOT in live_sessions, so it should be reported.
    let (_restored, missing) = LayoutService::restore(&state, layout.id)
        .await
        .expect("restore");

    assert_eq!(
        missing,
        vec![dead_sid],
        "dead session should be in missing_sessions"
    );
}

/// Restoring a layout with a live session in the map does not report it missing.
#[tokio::test]
async fn layout_restore_live_session_not_missing() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "layout-live").await;

    let live_sid =
        crate::common::seed_workbench_session(&state, project_id, Some(std::process::id() as i64))
            .await;

    // Install a live handle.
    {
        let handle = agentui_workbench::session::live::LiveSessionHandle::attached_mirror(live_sid);
        state.live_sessions.write().await.insert(live_sid, handle);
    }

    let ws = WorkspaceState {
        focused_session_id: Some(live_sid),
        ..Default::default()
    };
    let layout = LayoutService::save(&state.db, "live-test", &ws, false)
        .await
        .expect("save");

    let (_restored, missing) = LayoutService::restore(&state, layout.id)
        .await
        .expect("restore");

    assert!(
        missing.is_empty(),
        "live session should not be reported as missing"
    );
}
