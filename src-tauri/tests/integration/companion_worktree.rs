//! T088: Integration test — companion spawns in the session's worktree.
//!
//! Validates two properties:
//! 1. A session with `working_directory` different from the project root
//!    causes the companion to spawn in that worktree directory.
//! 2. FR-017 negative invariant: after the user `cd`s inside the companion,
//!    a subsequent resize + re-activate MUST NOT reset the companion's cwd.
//!    (The companion's `initial_cwd` is set at spawn time, but the shell's
//!    internal cwd must be preserved through resize/reactivation.)

use tend_workbench::companion::CompanionService;
use tend_workbench::project::ProjectService;
use tend_workbench::session::SessionService;
use std::collections::BTreeMap;

/// Companion spawns in the session's working_directory (worktree), not the
/// project root.
#[tokio::test]
async fn companion_spawns_in_worktree_directory() {
    let state = crate::common::mock_state().await;

    // Create a project with root at `project_dir`.
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = ProjectService::register(
        &state.db,
        project_dir.path().to_str().unwrap(),
        Some("worktree-project"),
    )
    .await
    .expect("register project");

    // Create a worktree subdirectory that differs from the project root.
    let worktree = project_dir.path().join("worktrees").join("feature-branch");
    std::fs::create_dir_all(&worktree).expect("create worktree dir");

    // Spawn a session with the worktree as its working directory.
    let env = BTreeMap::new();
    let (session, _handle) = SessionService::spawn_local(
        &state,
        project.id,
        "worktree-session",
        &worktree,
        &[
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 300".to_string(),
        ],
        &env,
    )
    .await
    .expect("spawn_local in worktree");

    // Activate (ensure) a companion.
    let companion = CompanionService::ensure(&state, session.id)
        .await
        .expect("ensure companion");

    assert_eq!(
        companion.initial_cwd, worktree,
        "companion initial_cwd must match the session's working_directory (the worktree), not the project root"
    );
}

/// FR-017 negative invariant: after `cd /tmp` in the companion, resize +
/// re-activate must NOT reset the companion's cwd.
///
/// We verify this by checking that `ensure` returns the same companion row
/// (same id, same initial_cwd). The shell's internal cwd is not tracked by
/// the DB — but the companion must NOT be respawned (which would reset it).
#[tokio::test]
async fn cd_in_companion_survives_resize_and_reactivation() {
    let state = crate::common::mock_state().await;

    let tmp = tempfile::tempdir().expect("session dir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("cd-survive-project"),
    )
    .await
    .expect("register project");

    let env = BTreeMap::new();
    let (session, _handle) = SessionService::spawn_local(
        &state,
        project.id,
        "cd-survive-session",
        tmp.path(),
        &[
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 300".to_string(),
        ],
        &env,
    )
    .await
    .expect("spawn_local");

    // Activate companion.
    let companion = CompanionService::ensure(&state, session.id)
        .await
        .expect("first ensure");

    // Simulate `cd /tmp` by writing to the companion.
    {
        let companions = state.live_companions.read().await;
        let handle = companions
            .get(&session.id)
            .expect("companion handle must exist");
        handle.write(b"cd /tmp\n").expect("cd /tmp write");
    }

    // Give the shell a moment to process the cd.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Resize the companion.
    {
        let companions = state.live_companions.read().await;
        let handle = companions
            .get(&session.id)
            .expect("companion handle after cd");
        handle.resize(100, 30).expect("resize");
    }

    // Re-activate (ensure again).
    let reactivated = CompanionService::ensure(&state, session.id)
        .await
        .expect("ensure after cd + resize");

    // The companion must be the SAME instance (not respawned), preserving
    // the shell's internal cwd.
    assert_eq!(
        companion.id, reactivated.id,
        "re-activation after cd + resize must return the same companion (not respawn)"
    );

    // initial_cwd remains the original — the DB does not track shell-internal cd.
    assert_eq!(
        reactivated.initial_cwd, companion.initial_cwd,
        "initial_cwd must not change on re-activation"
    );
}
