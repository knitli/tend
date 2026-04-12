//! T101: `NoteService` contract tests.
//!
//! Exercises note CRUD: list, create, update, delete — including error paths
//! for CONTENT_EMPTY, PROJECT_NOT_FOUND, and NOT_FOUND.

use agentui_workbench::error::ErrorCode;
use agentui_workbench::model::{NoteId, ProjectId};
use agentui_workbench::scratchpad::notes::NoteService;

// ── note_list ────────────────────────────────────────────────────────────

/// An empty project returns an empty note list.
#[tokio::test]
async fn note_list_empty() {
    let state = crate::common::mock_state().await;
    let project = crate::common::seed_project(&state, "notes-empty").await;

    let (notes, cursor) = NoteService::list(&state.db, project, None, None)
        .await
        .expect("list should succeed on empty project");

    assert!(notes.is_empty(), "expected no notes, got {}", notes.len());
    assert!(cursor.is_none(), "cursor must be None for empty result");
}

/// After creating notes, list returns them in created_at DESC order.
#[tokio::test]
async fn note_list_after_creates() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("notes-list"),
    )
    .await
    .expect("register");

    let _n1 = NoteService::create(&state.db, project.id, "first note")
        .await
        .expect("create 1");
    let _n2 = NoteService::create(&state.db, project.id, "second note")
        .await
        .expect("create 2");

    let (notes, _cursor) = NoteService::list(&state.db, project.id, None, None)
        .await
        .expect("list");

    assert_eq!(notes.len(), 2, "expected 2 notes");
    // Most recent first.
    assert_eq!(notes[0].content, "second note");
    assert_eq!(notes[1].content, "first note");
}

// ── note_create ──────────────────────────────────────────────────────────

/// Happy path: create a note with valid content.
#[tokio::test]
async fn note_create_happy() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("notes-create"),
    )
    .await
    .expect("register");

    let note = NoteService::create(&state.db, project.id, "hello world")
        .await
        .expect("create");

    assert_eq!(note.content, "hello world");
    assert_eq!(note.project_id, project.id);
}

/// Whitespace-only content returns CONTENT_EMPTY.
#[tokio::test]
async fn note_create_content_empty_for_whitespace() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("notes-empty-content"),
    )
    .await
    .expect("register");

    let err = NoteService::create(&state.db, project.id, "   \n  ")
        .await
        .expect_err("whitespace-only must fail");

    assert_eq!(err.code, ErrorCode::ContentEmpty);
}

/// Creating a note for a non-existent project returns NOT_FOUND.
#[tokio::test]
async fn note_create_project_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = ProjectId::new(999_999);

    let err = NoteService::create(&state.db, bogus, "orphan note")
        .await
        .expect_err("bogus project must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}

// ── note_update ──────────────────────────────────────────────────────────

/// Happy path: update an existing note.
#[tokio::test]
async fn note_update_happy() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("notes-update"),
    )
    .await
    .expect("register");

    let note = NoteService::create(&state.db, project.id, "original")
        .await
        .expect("create");

    let updated = NoteService::update(&state.db, note.id, "revised")
        .await
        .expect("update");

    assert_eq!(updated.content, "revised");
    assert!(
        updated.updated_at >= note.updated_at,
        "updated_at must advance"
    );
}

/// Updating with whitespace-only content returns CONTENT_EMPTY.
#[tokio::test]
async fn note_update_content_empty() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("notes-update-empty"),
    )
    .await
    .expect("register");

    let note = NoteService::create(&state.db, project.id, "keep this")
        .await
        .expect("create");

    let err = NoteService::update(&state.db, note.id, "  ")
        .await
        .expect_err("whitespace update must fail");

    assert_eq!(err.code, ErrorCode::ContentEmpty);
}

/// Updating a non-existent note returns NOT_FOUND.
#[tokio::test]
async fn note_update_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = NoteId::new(999_999);

    let err = NoteService::update(&state.db, bogus, "ghost")
        .await
        .expect_err("bogus note must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}

// ── note_delete ──────────────────────────────────────────────────────────

/// Happy path: delete an existing note.
#[tokio::test]
async fn note_delete_happy() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = agentui_workbench::project::ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("notes-delete"),
    )
    .await
    .expect("register");

    let note = NoteService::create(&state.db, project.id, "to be deleted")
        .await
        .expect("create");

    NoteService::delete(&state.db, note.id)
        .await
        .expect("delete should succeed");

    // Verify it's gone.
    let (notes, _) = NoteService::list(&state.db, project.id, None, None)
        .await
        .expect("list");
    assert!(notes.is_empty(), "note should be deleted");
}

/// Deleting a non-existent note returns NOT_FOUND.
#[tokio::test]
async fn note_delete_not_found() {
    let state = crate::common::mock_state().await;
    let bogus = NoteId::new(999_999);

    let err = NoteService::delete(&state.db, bogus)
        .await
        .expect_err("bogus note delete must fail");

    assert_eq!(err.code, ErrorCode::NotFound);
}
