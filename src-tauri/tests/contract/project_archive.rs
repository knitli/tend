//! T034: `project_archive` / `project_unarchive` contract tests.

use tend_workbench::error::ErrorCode;
use tend_workbench::project::ProjectService;

/// Archive and unarchive happy path.
#[tokio::test]
async fn archive_and_unarchive() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project =
        ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("archivable"))
            .await
            .expect("register");

    // Archive.
    ProjectService::archive(&state.db, project.id)
        .await
        .expect("archive should succeed");

    let archived = ProjectService::get_by_id(&state.db, project.id)
        .await
        .expect("get_by_id");
    assert!(archived.archived_at.is_some(), "project should be archived");

    // Unarchive.
    let unarchived = ProjectService::unarchive(&state.db, project.id)
        .await
        .expect("unarchive should succeed");
    assert!(
        unarchived.archived_at.is_none(),
        "project should be unarchived"
    );
}

/// Unarchive a non-archived project returns NOT_ARCHIVED.
#[tokio::test]
async fn unarchive_not_archived() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project = ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("active"))
        .await
        .expect("register");

    let err = ProjectService::unarchive(&state.db, project.id)
        .await
        .expect_err("should fail on non-archived project");

    assert_eq!(err.code, ErrorCode::NotArchived);
}

/// Scratchpad rows survive archival: insert a note before archive,
/// verify it's still there after.
#[tokio::test]
async fn scratchpad_survives_archival() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project =
        ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("with-notes"))
            .await
            .expect("register");

    // Insert a scratchpad note directly.
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO notes (project_id, content, created_at, updated_at)
        VALUES (?1, 'important note', ?2, ?2)
        "#,
    )
    .bind(project.id.get())
    .bind(&now)
    .execute(state.db.pool())
    .await
    .expect("insert note");

    // Archive the project.
    ProjectService::archive(&state.db, project.id)
        .await
        .expect("archive");

    // The note should still exist.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE project_id = ?1")
        .bind(project.id.get())
        .fetch_one(state.db.pool())
        .await
        .expect("count notes");

    assert_eq!(count.0, 1, "note should survive project archival");
}
