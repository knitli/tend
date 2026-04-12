//! T103: `OverviewService` contract tests.
//!
//! Exercises the cross-project overview: grouping by project display_name,
//! excluding done reminders, excluding archived projects, and empty state.

use agentui_workbench::model::ReminderState;
use agentui_workbench::project::ProjectService;
use agentui_workbench::scratchpad::overview::OverviewService;
use agentui_workbench::scratchpad::reminders::ReminderService;

/// Cross-project overview returns groups ordered by project display_name.
#[tokio::test]
async fn cross_project_overview_ordered_by_display_name() {
    let state = crate::common::mock_state().await;

    // Create two projects: "Zeta" and "Alpha" (reversed alphabetical order).
    let tmp_z = tempfile::tempdir().expect("tempdir");
    let proj_z = ProjectService::register(&state.db, tmp_z.path().to_str().unwrap(), Some("Zeta"))
        .await
        .expect("register Zeta");

    let tmp_a = tempfile::tempdir().expect("tempdir");
    let proj_a = ProjectService::register(&state.db, tmp_a.path().to_str().unwrap(), Some("Alpha"))
        .await
        .expect("register Alpha");

    // Add open reminders to both.
    ReminderService::create(&state.db, proj_z.id, "zeta task")
        .await
        .expect("create zeta reminder");
    ReminderService::create(&state.db, proj_a.id, "alpha task")
        .await
        .expect("create alpha reminder");

    let groups = OverviewService::overview(&state.db)
        .await
        .expect("overview");

    assert_eq!(groups.len(), 2, "expected 2 project groups");
    assert_eq!(groups[0].project.display_name, "Alpha");
    assert_eq!(groups[1].project.display_name, "Zeta");
}

/// Done reminders are excluded from the overview.
#[tokio::test]
async fn cross_project_overview_excludes_done_reminders() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("done-exclude"),
    )
    .await
    .expect("register");

    let rem = ReminderService::create(&state.db, project.id, "will be done")
        .await
        .expect("create");
    ReminderService::set_state(&state.db, rem.id, ReminderState::Done)
        .await
        .expect("mark done");

    let groups = OverviewService::overview(&state.db)
        .await
        .expect("overview");

    // Project has no open reminders, so it should not appear.
    assert!(
        groups.is_empty(),
        "project with only done reminders must not appear in overview"
    );
}

/// Archived projects are excluded from the overview.
#[tokio::test]
async fn cross_project_overview_excludes_archived_projects() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("archived-exclude"),
    )
    .await
    .expect("register");

    ReminderService::create(&state.db, project.id, "open reminder")
        .await
        .expect("create");

    // Archive the project.
    ProjectService::archive(&state.db, project.id)
        .await
        .expect("archive");

    let groups = OverviewService::overview(&state.db)
        .await
        .expect("overview");

    assert!(
        groups.is_empty(),
        "archived project must not appear in overview"
    );
}

/// Overview returns empty when no open reminders exist.
#[tokio::test]
async fn cross_project_overview_empty_when_no_open_reminders() {
    let state = crate::common::mock_state().await;

    let groups = OverviewService::overview(&state.db)
        .await
        .expect("overview");

    assert!(groups.is_empty(), "overview must be empty with no data");
}

/// L3: Within a group, reminders are ordered created_at DESC.
#[tokio::test]
async fn cross_project_overview_per_group_ordering() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "ordering-test").await;

    // Create 3 reminders with small delays so timestamps differ.
    let mut ids = Vec::new();
    for i in 0..3 {
        let r = ReminderService::create(&state.db, project_id, &format!("reminder {i}"))
            .await
            .expect("create reminder");
        ids.push(r.id.get());
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let groups = OverviewService::overview(&state.db)
        .await
        .expect("overview");

    assert_eq!(groups.len(), 1);
    let reminders = &groups[0].open_reminders;
    assert_eq!(reminders.len(), 3);

    // created_at DESC means the last-created reminder should be first.
    assert_eq!(reminders[0].id.get(), ids[2], "newest should be first");
    assert_eq!(reminders[1].id.get(), ids[1], "middle should be second");
    assert_eq!(reminders[2].id.get(), ids[0], "oldest should be last");
}
