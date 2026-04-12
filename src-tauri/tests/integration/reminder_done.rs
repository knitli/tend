//! T106: Integration test — done reminders in overview vs list.
//!
//! A done reminder must be excluded from the cross-project overview but still
//! retrievable via `ReminderService::list` with `state = Done`.

use agentui_workbench::model::ReminderState;
use agentui_workbench::project::ProjectService;
use agentui_workbench::scratchpad::overview::OverviewService;
use agentui_workbench::scratchpad::reminders::ReminderService;

/// Done reminder excluded from overview but retrievable via list(state=done).
#[tokio::test]
async fn done_reminder_excluded_from_overview_but_retrievable() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-done-integ"),
    )
    .await
    .expect("register");

    // Create two reminders: one stays open, one is marked done.
    let _open = ReminderService::create(&state.db, project.id, "still open")
        .await
        .expect("create open");
    let done = ReminderService::create(&state.db, project.id, "will be done")
        .await
        .expect("create done");

    ReminderService::set_state(&state.db, done.id, ReminderState::Done)
        .await
        .expect("mark done");

    // --- Overview: done reminder excluded ---
    let groups = OverviewService::overview(&state.db)
        .await
        .expect("overview");

    assert_eq!(groups.len(), 1, "one project group (has open reminders)");
    assert_eq!(
        groups[0].open_reminders.len(),
        1,
        "only the open reminder appears in overview"
    );
    assert_eq!(groups[0].open_reminders[0].content, "still open");

    // --- List with state=Done: done reminder retrievable ---
    let (done_list, _) = ReminderService::list(
        &state.db,
        Some(project.id),
        Some(ReminderState::Done),
        None,
        None,
    )
    .await
    .expect("list done");

    assert_eq!(done_list.len(), 1);
    assert_eq!(done_list[0].content, "will be done");
    assert_eq!(done_list[0].state, ReminderState::Done);
    assert!(
        done_list[0].done_at.is_some(),
        "done_at must be populated for done reminders"
    );
}
