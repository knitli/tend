//! T119: `LayoutService` contract tests.
//!
//! Exercises layout_list, layout_save (+ NAME_TAKEN + overwrite), layout_restore
//! (returns missing_sessions for dead refs), and layout_delete.

use tend_workbench::error::ErrorCode;
use tend_workbench::model::{LayoutId, ProjectId, SessionId, WorkspaceState};
use tend_workbench::workspace::layouts::LayoutService;

/// Empty list when no layouts exist.
#[tokio::test]
async fn layout_list_empty() {
    let state = crate::common::mock_state().await;

    let layouts = LayoutService::list(&state.db).await.expect("list");

    assert!(layouts.is_empty());
}

/// Save a layout, then list returns it.
#[tokio::test]
async fn layout_save_and_list() {
    let state = crate::common::mock_state().await;

    let ws = WorkspaceState {
        pane_layout: "split".into(),
        ..Default::default()
    };

    let layout = LayoutService::save(&state.db, "my layout", &ws, false)
        .await
        .expect("save");

    assert_eq!(layout.name, "my layout");
    assert_eq!(layout.payload, ws);

    let layouts = LayoutService::list(&state.db).await.expect("list");
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].name, "my layout");
}

/// Duplicate name returns NAME_TAKEN when overwrite is false.
#[tokio::test]
async fn layout_save_name_taken() {
    let state = crate::common::mock_state().await;

    let ws = WorkspaceState::default();

    LayoutService::save(&state.db, "dup", &ws, false)
        .await
        .expect("first save");

    let err = LayoutService::save(&state.db, "dup", &ws, false)
        .await
        .expect_err("duplicate must fail");

    assert_eq!(err.code, ErrorCode::NameTaken);
}

/// Empty/whitespace-only name returns CONTENT_EMPTY (M5).
#[tokio::test]
async fn layout_save_rejects_empty_name() {
    let state = crate::common::mock_state().await;
    let ws = WorkspaceState::default();

    let err = LayoutService::save(&state.db, "   ", &ws, false)
        .await
        .expect_err("whitespace-only name must fail");

    assert_eq!(err.code, ErrorCode::ContentEmpty);
}

/// Name exceeding 255 chars returns CONTENT_EMPTY (M7).
#[tokio::test]
async fn layout_save_rejects_long_name() {
    let state = crate::common::mock_state().await;
    let ws = WorkspaceState::default();
    let long_name = "a".repeat(256);

    let err = LayoutService::save(&state.db, &long_name, &ws, false)
        .await
        .expect_err("long name must fail");

    assert_eq!(err.code, ErrorCode::ContentEmpty);
}

/// Duplicate name with overwrite=true updates the existing layout (H2).
#[tokio::test]
async fn layout_save_overwrite_replaces_existing() {
    let state = crate::common::mock_state().await;

    let ws1 = WorkspaceState {
        pane_layout: "split".into(),
        ..Default::default()
    };
    let original = LayoutService::save(&state.db, "overwrite-test", &ws1, false)
        .await
        .expect("first save");

    let ws2 = WorkspaceState {
        pane_layout: "agent_only".into(),
        ..Default::default()
    };
    let updated = LayoutService::save(&state.db, "overwrite-test", &ws2, true)
        .await
        .expect("overwrite save");

    assert_eq!(updated.id, original.id, "id must be preserved on overwrite");
    assert_eq!(updated.payload.pane_layout, "agent_only");

    // Only one layout should exist.
    let layouts = LayoutService::list(&state.db).await.expect("list");
    assert_eq!(layouts.len(), 1);
}

/// Restore returns the layout state + empty missing_sessions when all sessions
/// referenced in the state are alive.
#[tokio::test]
async fn layout_restore_happy() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "layout-restore").await;
    let session_id =
        crate::common::seed_workbench_session(&state, project_id, Some(std::process::id() as i64))
            .await;

    // Install a live session handle so the restore can find it.
    {
        let handle = tend_workbench::session::live::LiveSessionHandle::attached_mirror(session_id);
        state.live_sessions.write().await.insert(session_id, handle);
    }

    let ws = WorkspaceState {
        focused_session_id: Some(session_id),
        active_project_ids: vec![project_id],
        ..Default::default()
    };
    let layout = LayoutService::save(&state.db, "test-restore", &ws, false)
        .await
        .expect("save");

    let (restored, missing) = LayoutService::restore(&state, layout.id)
        .await
        .expect("restore");

    assert_eq!(restored, ws);
    assert!(missing.is_empty(), "no sessions should be missing");
}

/// Restore with a dead session reference reports it in missing_sessions.
#[tokio::test]
async fn layout_restore_with_missing_sessions() {
    let state = crate::common::mock_state().await;

    let ws = WorkspaceState {
        focused_session_id: Some(SessionId::new(999)),
        active_project_ids: vec![ProjectId::new(1)],
        ..Default::default()
    };
    let layout = LayoutService::save(&state.db, "dead-ref", &ws, false)
        .await
        .expect("save");

    let (_restored, missing) = LayoutService::restore(&state, layout.id)
        .await
        .expect("restore");

    assert_eq!(missing, vec![SessionId::new(999)]);
}

/// Delete a layout.
#[tokio::test]
async fn layout_delete_happy() {
    let state = crate::common::mock_state().await;

    let layout = LayoutService::save(&state.db, "del", &WorkspaceState::default(), false)
        .await
        .expect("save");

    LayoutService::delete(&state.db, layout.id)
        .await
        .expect("delete");

    let layouts = LayoutService::list(&state.db).await.expect("list");
    assert!(layouts.is_empty());
}

/// Deleting a non-existent layout returns NOT_FOUND.
#[tokio::test]
async fn layout_delete_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = LayoutId::new(999);

    let err = LayoutService::delete(&state.db, bogus)
        .await
        .expect_err("bogus layout delete must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}

/// Restore a non-existent layout returns NOT_FOUND.
#[tokio::test]
async fn layout_restore_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = LayoutId::new(999);

    let err = LayoutService::restore(&state, bogus)
        .await
        .expect_err("bogus layout restore must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}
