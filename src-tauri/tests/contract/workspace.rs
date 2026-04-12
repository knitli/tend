//! T118: `WorkspaceService` contract tests.
//!
//! Exercises workspace_get returning a hydrateable state that is roundtrippable
//! with workspace_save.

use agentui_workbench::model::WorkspaceState;
use agentui_workbench::workspace::WorkspaceService;

/// Default workspace state when no row exists yet.
#[tokio::test]
async fn workspace_get_returns_default_when_empty() {
    let state = crate::common::mock_state().await;

    let ws = WorkspaceService::get(&state.db).await.expect("get");

    assert_eq!(ws, WorkspaceState::default());
    assert_eq!(ws.version, 1);
    assert!(ws.active_project_ids.is_empty());
    assert!(ws.focused_session_id.is_none());
    assert_eq!(ws.pane_layout, "split");
}

/// Roundtrip: save then get returns the exact same state.
#[tokio::test]
async fn workspace_save_then_get_roundtrips() {
    let state = crate::common::mock_state().await;

    let mut ws = WorkspaceState::default();
    ws.active_project_ids = vec![
        agentui_workbench::model::ProjectId::new(1),
        agentui_workbench::model::ProjectId::new(4),
    ];
    ws.focused_session_id = Some(agentui_workbench::model::SessionId::new(12));
    ws.pane_layout = "agent_only".to_string();
    ws.ui
        .insert("sidebar_width".to_string(), serde_json::json!(280));
    ws.ui
        .insert("scratchpad_visible".to_string(), serde_json::json!(true));

    WorkspaceService::save(&state.db, &ws)
        .await
        .expect("save");

    let loaded = WorkspaceService::get(&state.db).await.expect("get");

    assert_eq!(loaded, ws);
}

/// Overwriting: a second save replaces the previous state.
#[tokio::test]
async fn workspace_save_overwrites_previous() {
    let state = crate::common::mock_state().await;

    let ws1 = WorkspaceState {
        pane_layout: "split".into(),
        ..Default::default()
    };
    WorkspaceService::save(&state.db, &ws1)
        .await
        .expect("save 1");

    let ws2 = WorkspaceState {
        pane_layout: "agent_only".into(),
        ..Default::default()
    };
    WorkspaceService::save(&state.db, &ws2)
        .await
        .expect("save 2");

    let loaded = WorkspaceService::get(&state.db).await.expect("get");
    assert_eq!(loaded.pane_layout, "agent_only");
}
