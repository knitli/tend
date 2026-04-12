//! T033: `project_update` contract tests.

use agentui_workbench::error::ErrorCode;
use agentui_workbench::model::ProjectId;
use agentui_workbench::project::ProjectService;

/// Happy path: update display_name and verify the change.
#[tokio::test]
async fn update_display_name() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project = ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("old"))
        .await
        .expect("register");

    let updated = ProjectService::update(&state.db, project.id, Some("new-name"), None)
        .await
        .expect("update should succeed");

    assert_eq!(updated.display_name, "new-name");
    assert_eq!(updated.id, project.id);
}

/// NOT_FOUND: updating a non-existent project returns the correct error.
#[tokio::test]
async fn update_not_found() {
    let state = crate::common::mock_state().await;
    let err = ProjectService::update(&state.db, ProjectId::new(999_999), Some("nope"), None)
        .await
        .expect_err("should fail for non-existent project");

    assert_eq!(err.code, ErrorCode::NotFound);
}
