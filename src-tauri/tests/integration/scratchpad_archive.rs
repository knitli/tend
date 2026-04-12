//! T104: Integration test — scratchpad survives project archive/unarchive.
//!
//! Archiving a project should NOT delete its notes or reminders. After
//! unarchiving, the scratchpad must be fully intact.

use agentui_workbench::project::ProjectService;
use agentui_workbench::scratchpad::notes::NoteService;
use agentui_workbench::scratchpad::reminders::ReminderService;

/// Archive project -> notes/reminders still queryable; unarchive -> intact.
#[tokio::test]
async fn archive_preserves_scratchpad() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("archive-scratch"),
    )
    .await
    .expect("register");

    // Seed some notes and reminders.
    NoteService::create(&state.db, project.id, "note A")
        .await
        .expect("note A");
    NoteService::create(&state.db, project.id, "note B")
        .await
        .expect("note B");
    ReminderService::create(&state.db, project.id, "reminder X")
        .await
        .expect("reminder X");

    // --- Archive ---
    ProjectService::archive(&state.db, project.id)
        .await
        .expect("archive");

    // Notes are still queryable while archived.
    let (notes, _) = NoteService::list(&state.db, project.id, None, None)
        .await
        .expect("list notes after archive");
    assert_eq!(notes.len(), 2, "notes must survive archive");

    // Reminders are still queryable while archived.
    let (reminders, _) = ReminderService::list(&state.db, Some(project.id), None, None, None)
        .await
        .expect("list reminders after archive");
    assert_eq!(reminders.len(), 1, "reminders must survive archive");

    // --- Unarchive ---
    ProjectService::unarchive(&state.db, project.id)
        .await
        .expect("unarchive");

    // Verify scratchpad is intact after unarchive.
    let (notes, _) = NoteService::list(&state.db, project.id, None, None)
        .await
        .expect("list notes after unarchive");
    assert_eq!(notes.len(), 2, "notes must survive unarchive");

    let (reminders, _) = ReminderService::list(&state.db, Some(project.id), None, None, None)
        .await
        .expect("list reminders after unarchive");
    assert_eq!(reminders.len(), 1, "reminders must survive unarchive");
}
