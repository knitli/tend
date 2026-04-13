//! Shared test helpers for `src-tauri` integration + contract tests.
//!
//! T027: per-test-file convention note.
//!
//! **IMPORTANT** — Rust integration tests in `tests/` compile each top-level
//! file as its own binary, and `tests/common/mod.rs` is NOT automatically
//! picked up by the sibling files. Every contract / integration test file
//! that wants to use these helpers MUST begin with:
//!
//! ```ignore
//! mod common;
//! ```
//!
//! That resolves to `tests/common/mod.rs` via `rustc`'s module system when
//! the test file lives at `tests/whatever.rs`. Nested tests that live at
//! `tests/subdir/whatever.rs` should instead use
//! `#[path = "../common/mod.rs"] mod common;` — skip this and you will hit
//! "unresolved import" on the second file added.
//!
//! Helpers provided:
//!   * `temp_db()` — isolated in-memory sqlite pool + migrations applied
//!   * `temp_socket_path()` — a unique unix socket path under `std::env::temp_dir`
//!   * `mock_state()` — a `WorkbenchState` built around `temp_db()`
//!   * `seed_wrapper_session(…)` — inserts a wrapper-owned session row for the
//!     ownership-aware contract tests (T035, T086)

#![allow(dead_code)] // Shared helpers; not every test file uses every helper.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tend_workbench::db::Database;
use tend_workbench::model::{ProjectId, SessionId};
use tend_workbench::state::WorkbenchState;

/// Monotonic counter for unique ephemeral paths.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Create an isolated in-memory database with migrations applied.
pub async fn temp_db() -> Database {
    Database::open_in_memory()
        .await
        .expect("open_in_memory + migrate")
}

/// Build a `WorkbenchState` around a fresh in-memory DB.
pub async fn mock_state() -> WorkbenchState {
    let db = temp_db().await;
    WorkbenchState::new(db)
}

/// Return a unique unix-domain socket path under `std::env::temp_dir`.
pub fn temp_socket_path() -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("tend-test-{pid}-{seq}.sock"))
}

/// Insert a bare-bones project row so session seeders have a parent row.
pub async fn seed_project(state: &WorkbenchState, display_name: &str) -> ProjectId {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO projects (canonical_path, display_name, added_at, settings_json)
        VALUES (?1, ?2, ?3, '{}')
        RETURNING id
        "#,
    )
    .bind(format!("/tmp/tend-test-{display_name}"))
    .bind(display_name)
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert project");
    ProjectId::new(row.0)
}

/// Insert a wrapper-owned session row. Used by the ownership-aware contract
/// tests in T035, T086, and by T025b to prove reconcile_and_reattach handles
/// both branches.
pub async fn seed_wrapper_session(
    state: &WorkbenchState,
    project_id: ProjectId,
    pid: Option<i64>,
) -> SessionId {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO sessions (
            project_id, label, pid, status, status_source, ownership,
            started_at, last_activity_at, metadata_json, working_directory
        )
        VALUES (?1, 'seeded wrapper', ?2, 'working', 'ipc', 'wrapper', ?3, ?3, '{}', '/tmp')
        RETURNING id
        "#,
    )
    .bind(project_id.get())
    .bind(pid)
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert wrapper session");
    SessionId::new(row.0)
}

/// Insert a workbench-owned session row for tests that need the other branch.
pub async fn seed_workbench_session(
    state: &WorkbenchState,
    project_id: ProjectId,
    pid: Option<i64>,
) -> SessionId {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO sessions (
            project_id, label, pid, status, status_source, ownership,
            started_at, last_activity_at, metadata_json, working_directory
        )
        VALUES (?1, 'seeded workbench', ?2, 'working', 'ipc', 'workbench', ?3, ?3, '{}', '/tmp')
        RETURNING id
        "#,
    )
    .bind(project_id.get())
    .bind(pid)
    .bind(&now)
    .fetch_one(state.db.pool())
    .await
    .expect("insert workbench session");
    SessionId::new(row.0)
}
