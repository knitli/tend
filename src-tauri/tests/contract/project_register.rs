//! T031: `project_register` contract tests.

use agentui_workbench::error::ErrorCode;
use agentui_workbench::project::ProjectService;

/// Happy path: register a real temp directory, assert the returned project
/// has the correct canonical path and display name.
#[tokio::test]
async fn register_happy_path() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let path_str = tmp.path().to_str().expect("utf-8 path");

    let project = ProjectService::register(&state.db, path_str, Some("test-project"))
        .await
        .expect("register should succeed");

    assert_eq!(project.display_name, "test-project");
    // Canonical path should resolve to the same directory.
    let canonical = std::fs::canonicalize(tmp.path()).expect("canonicalize");
    assert_eq!(project.canonical_path, canonical);
    assert!(project.archived_at.is_none());
}

/// PATH_NOT_FOUND: registering a non-existent path returns the correct error.
#[tokio::test]
async fn register_path_not_found() {
    let state = crate::common::mock_state().await;
    let err = ProjectService::register(&state.db, "/tmp/agentui-does-not-exist-xyz123", None)
        .await
        .expect_err("should fail for non-existent path");

    assert_eq!(err.code, ErrorCode::PathNotFound);
}

/// PATH_NOT_A_DIRECTORY: registering a file (not a directory) returns the
/// correct error.
#[tokio::test]
async fn register_path_not_a_directory() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    let path_str = tmp.path().to_str().expect("utf-8 path");

    let err = ProjectService::register(&state.db, path_str, None)
        .await
        .expect_err("should fail for a file path");

    assert_eq!(err.code, ErrorCode::PathNotADirectory);
}

/// ALREADY_REGISTERED: registering the same path twice returns the correct error.
#[tokio::test]
async fn register_already_registered() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let path_str = tmp.path().to_str().expect("utf-8 path");

    // First registration succeeds.
    let _project = ProjectService::register(&state.db, path_str, Some("first"))
        .await
        .expect("first register should succeed");

    // Second registration of the same path should fail.
    let err = ProjectService::register(&state.db, path_str, Some("second"))
        .await
        .expect_err("second register should fail");

    assert_eq!(err.code, ErrorCode::AlreadyRegistered);
}
