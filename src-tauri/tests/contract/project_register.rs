//! T031: `project_register` contract tests.

use tend_workbench::error::ErrorCode;
use tend_workbench::project::{COLOR_PALETTE, ProjectService};

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
    let err = ProjectService::register(&state.db, "/tmp/tend-does-not-exist-xyz123", None)
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

/// Spec §1.2: `project_register` auto-assigns a palette colour to the new
/// project. The colour is derived from `(id - 1) % 12` so the first project
/// gets the first palette entry.
#[tokio::test]
async fn register_assigns_palette_colour() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("create temp dir");
    let path_str = tmp.path().to_str().expect("utf-8 path");

    let project = ProjectService::register(&state.db, path_str, Some("coloured"))
        .await
        .expect("register should succeed");

    let color = project
        .settings
        .color
        .as_deref()
        .expect("auto-assigned colour");
    // The first project (id = 1) maps to palette[0].
    assert_eq!(color, COLOR_PALETTE[0]);
}

/// Spec §1.2: sequential project registrations cycle through the 12-colour
/// palette. Registering three projects yields three distinct palette entries.
#[tokio::test]
async fn register_cycles_palette_across_projects() {
    let state = crate::common::mock_state().await;

    let mut colours = Vec::new();
    let mut _keepalive = Vec::new();
    for i in 0..3 {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let path_str = tmp.path().to_str().expect("utf-8 path").to_string();
        let project = ProjectService::register(&state.db, &path_str, Some(&format!("p{i}")))
            .await
            .expect("register should succeed");
        colours.push(
            project
                .settings
                .color
                .clone()
                .expect("auto-assigned colour"),
        );
        // Keep the tempdir alive so the path stays valid for the rest of the
        // test — ProjectService::register canonicalises via std::fs::canonicalize
        // which fails if the directory is dropped.
        _keepalive.push(tmp);
    }

    // Three new rows → palette indices 0, 1, 2 (the in-memory DB starts empty).
    assert_eq!(colours[0], COLOR_PALETTE[0]);
    assert_eq!(colours[1], COLOR_PALETTE[1]);
    assert_eq!(colours[2], COLOR_PALETTE[2]);

    // All three colours are distinct (sanity: palette entries are unique).
    assert_ne!(colours[0], colours[1]);
    assert_ne!(colours[1], colours[2]);
    assert_ne!(colours[0], colours[2]);
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
