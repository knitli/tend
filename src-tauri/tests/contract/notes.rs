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

// ── M3: pagination cursor tests ─────────────────────────────────────────

#[tokio::test]
async fn note_list_pagination_with_cursor() {
    let state = crate::common::mock_state().await;
    let project_id = crate::common::seed_project(&state, "pagination-test").await;

    // Create 5 notes.
    for i in 0..5 {
        NoteService::create(&state.db, project_id, &format!("note {i}"))
            .await
            .expect("create note");
        // Small delay to ensure distinct timestamps.
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // First page: limit 2.
    let (page1, cursor1) = NoteService::list(&state.db, project_id, Some(2), None)
        .await
        .expect("list page 1");
    assert_eq!(page1.len(), 2, "first page should have 2 notes");
    assert!(cursor1.is_some(), "should have a next_cursor");

    // Second page using cursor.
    let (page2, cursor2) = NoteService::list(&state.db, project_id, Some(2), cursor1.as_deref())
        .await
        .expect("list page 2");
    assert_eq!(page2.len(), 2, "second page should have 2 notes");
    assert!(cursor2.is_some(), "should have another next_cursor");

    // Third page — only 1 remaining.
    let (page3, cursor3) = NoteService::list(&state.db, project_id, Some(2), cursor2.as_deref())
        .await
        .expect("list page 3");
    assert_eq!(page3.len(), 1, "third page should have 1 note");
    assert!(cursor3.is_none(), "last page should have no next_cursor");

    // Verify no duplicates across pages.
    let all_ids: Vec<i64> = page1
        .iter()
        .chain(page2.iter())
        .chain(page3.iter())
        .map(|n| n.id.get())
        .collect();
    let unique: std::collections::HashSet<i64> = all_ids.iter().copied().collect();
    assert_eq!(all_ids.len(), unique.len(), "no duplicates across pages");
    assert_eq!(all_ids.len(), 5, "all 5 notes returned across pages");
}
