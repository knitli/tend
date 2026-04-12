//! T032: `project_list` contract tests.

use agentui_workbench::project::ProjectService;

/// Register two projects, list with include_archived=false, get both.
#[tokio::test]
async fn list_returns_active_projects() {
    let state = crate::common::mock_state().await;
    let tmp_a = tempfile::tempdir().expect("create temp dir A");
    let tmp_b = tempfile::tempdir().expect("create temp dir B");

    ProjectService::register(&state.db, tmp_a.path().to_str().unwrap(), Some("alpha"))
        .await
        .expect("register A");
    ProjectService::register(&state.db, tmp_b.path().to_str().unwrap(), Some("beta"))
        .await
        .expect("register B");

    let projects = ProjectService::list(&state.db, false)
        .await
        .expect("list should succeed");

    assert_eq!(projects.len(), 2, "should list both active projects");
}

/// Archive one project, list with include_archived=false gets 1,
/// include_archived=true gets 2.
#[tokio::test]
async fn list_respects_include_archived() {
    let state = crate::common::mock_state().await;
    let tmp_a = tempfile::tempdir().expect("create temp dir A");
    let tmp_b = tempfile::tempdir().expect("create temp dir B");

    let project_a =
        ProjectService::register(&state.db, tmp_a.path().to_str().unwrap(), Some("alpha"))
            .await
            .expect("register A");
    ProjectService::register(&state.db, tmp_b.path().to_str().unwrap(), Some("beta"))
        .await
        .expect("register B");

    // Archive project A.
    ProjectService::archive(&state.db, project_a.id)
        .await
        .expect("archive A");

    let active = ProjectService::list(&state.db, false)
        .await
        .expect("list active");
    assert_eq!(active.len(), 1, "only non-archived project should show");
    assert_eq!(active[0].display_name, "beta");

    let all = ProjectService::list(&state.db, true)
        .await
        .expect("list all");
    assert_eq!(
        all.len(),
        2,
        "both projects should show with include_archived=true"
    );
}
