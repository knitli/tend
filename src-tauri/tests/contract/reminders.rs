//! T102: `ReminderService` contract tests.
//!
//! Exercises reminder CRUD: list (with state filter), create, set_state, delete —
//! including error paths for CONTENT_EMPTY and NOT_FOUND.

use agentui_workbench::error::ErrorCode;
use agentui_workbench::model::{ReminderId, ReminderState};
use agentui_workbench::scratchpad::reminders::ReminderService;

// ── reminder_list ────────────────────────────────────────────────────────

/// An empty project returns an empty reminder list.
#[tokio::test]
async fn reminder_list_empty() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "rem-empty").await;

    let (reminders, cursor) = ReminderService::list(&state.db, Some(project), None, None, None)
        .await
        .expect("list should succeed on empty project");

    assert!(reminders.is_empty());
    assert!(cursor.is_none());
}

/// Filtering by state = Open returns only open reminders.
#[tokio::test]
async fn reminder_list_state_filter_open() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-filter"),
    )
    .await
    .expect("register");

    let r1 = ReminderService::create(&state.db, project.id, "open one")
        .await
        .expect("create r1");
    let _r2 = ReminderService::create(&state.db, project.id, "done one")
        .await
        .expect("create r2");

    // Mark r2 done.
    ReminderService::set_state(&state.db, _r2.id, ReminderState::Done)
        .await
        .expect("set done");

    // Filter open only.
    let (open, _) = ReminderService::list(
        &state.db,
        Some(project.id),
        Some(ReminderState::Open),
        None,
        None,
    )
    .await
    .expect("list open");

    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, r1.id);
}

/// Filtering by state = Done returns only done reminders.
#[tokio::test]
async fn reminder_list_state_filter_done() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-filter-done"),
    )
    .await
    .expect("register");

    let _r1 = ReminderService::create(&state.db, project.id, "open one")
        .await
        .expect("create r1");
    let r2 = ReminderService::create(&state.db, project.id, "done one")
        .await
        .expect("create r2");

    ReminderService::set_state(&state.db, r2.id, ReminderState::Done)
        .await
        .expect("set done");

    let (done, _) = ReminderService::list(
        &state.db,
        Some(project.id),
        Some(ReminderState::Done),
        None,
        None,
    )
    .await
    .expect("list done");

    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, r2.id);
}

// ── reminder_create ──────────────────────────────────────────────────────

/// Happy path: create a reminder.
#[tokio::test]
async fn reminder_create_happy() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-create"),
    )
    .await
    .expect("register");

    let rem = ReminderService::create(&state.db, project.id, "buy milk")
        .await
        .expect("create");

    assert_eq!(rem.content, "buy milk");
    assert_eq!(rem.state, ReminderState::Open);
    assert!(rem.done_at.is_none());
}

/// Whitespace-only content returns CONTENT_EMPTY.
#[tokio::test]
async fn reminder_create_content_empty() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-empty-content"),
    )
    .await
    .expect("register");

    let err = ReminderService::create(&state.db, project.id, "   ")
        .await
        .expect_err("whitespace must fail");

    assert_eq!(err.code, ErrorCode::ContentEmpty);
}

// ── reminder_set_state ───────────────────────────────────────────────────

/// Open -> Done sets done_at.
#[tokio::test]
async fn reminder_set_state_open_to_done() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-state-done"),
    )
    .await
    .expect("register");

    let rem = ReminderService::create(&state.db, project.id, "finish task")
        .await
        .expect("create");
    assert!(rem.done_at.is_none(), "new reminder has no done_at");

    let done = ReminderService::set_state(&state.db, rem.id, ReminderState::Done)
        .await
        .expect("set done");

    assert_eq!(done.state, ReminderState::Done);
    assert!(
        done.done_at.is_some(),
        "done_at must be set after marking done"
    );
}

/// Done -> Open clears done_at.
#[tokio::test]
async fn reminder_set_state_done_to_open() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-state-reopen"),
    )
    .await
    .expect("register");

    let rem = ReminderService::create(&state.db, project.id, "reopenable")
        .await
        .expect("create");

    // Mark done then reopen.
    ReminderService::set_state(&state.db, rem.id, ReminderState::Done)
        .await
        .expect("set done");

    let reopened = ReminderService::set_state(&state.db, rem.id, ReminderState::Open)
        .await
        .expect("set open");

    assert_eq!(reopened.state, ReminderState::Open);
    assert!(
        reopened.done_at.is_none(),
        "done_at must be cleared after reopening"
    );
}

// ── reminder_delete ──────────────────────────────────────────────────────

/// Happy path: delete an existing reminder.
#[tokio::test]
async fn reminder_delete_happy() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("rem-delete"),
    )
    .await
    .expect("register");

    let rem = ReminderService::create(&state.db, project.id, "deleteme")
        .await
        .expect("create");

    ReminderService::delete(&state.db, rem.id)
        .await
        .expect("delete should succeed");

    // Verify gone.
    let (list, _) = ReminderService::list(&state.db, Some(project.id), None, None, None)
        .await
        .expect("list");
    assert!(list.is_empty());
}

/// Deleting a non-existent reminder returns NOT_FOUND.
#[tokio::test]
async fn reminder_delete_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = ReminderId::new(999_999);

    let err = ReminderService::delete(&state.db, bogus)
        .await
        .expect_err("bogus reminder delete must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}

// ── M1: reminder_set_state NOT_FOUND ────────────────────────────────────

#[tokio::test]
async fn reminder_set_state_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = ReminderId::new(999_999);

    let err = ReminderService::set_state(&state.db, bogus, ReminderState::Done)
        .await
        .expect_err("set_state on nonexistent reminder must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}

// ── M2: reminder_create PROJECT_NOT_FOUND ───────────────────────────────

#[tokio::test]
async fn reminder_create_project_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = agentui_workbench::model::ProjectId::new(999_999);

    let err = ReminderService::create(&state.db, bogus, "test reminder")
        .await
        .expect_err("create with bogus project must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}
