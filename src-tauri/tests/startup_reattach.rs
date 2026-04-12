//! T025b: hard gate against the reconcile_and_reattach ordering bug.
//!
//! 1. Spawn a real long-running child (`sleep 600`).
//! 2. Write a `sessions` row with that pid, `ownership = 'wrapper'`,
//!    `status = 'working'`.
//! 3. Run `reconcile_and_reattach` against a fresh state pointing at the
//!    same DB.
//! 4. Assert the row is still in a live status (working / idle) and that
//!    `state.live_sessions` contains a handle for it.
//! 5. SIGKILL the child, write another row with its defunct pid, re-run
//!    reconcile, assert the second row transitioned to `ended` with
//!    `error_reason = 'workbench_restart'`.
//!
//! If reconcile had been written as the buggy "mark-stale-first,
//! reattach-second" ordering, step 4 would fail because the first pass
//! would mark the live row ended before the second pass could reattach it.

mod common;

use agentui_workbench::session::recovery::reconcile_and_reattach;
use std::process::{Command, Stdio};
use std::time::Duration;

#[tokio::test]
async fn reconcile_reattaches_live_and_ends_dead() {
    let state = common::mock_state().await;
    let project_id = common::seed_project(&state, "reattach-test").await;

    // 1. Spawn a live child. We use `sleep 600` on unix and skip on windows.
    #[cfg(not(unix))]
    {
        eprintln!("T025b is unix-only; skipping on non-unix");
        return;
    }

    #[cfg(unix)]
    {
        let mut live_child = Command::new("sleep")
            .arg("600")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep 600");

        let live_pid = live_child.id() as i64;

        // 2. Insert a wrapper-owned row pointing at that live pid.
        let live_session = common::seed_wrapper_session(&state, project_id, Some(live_pid)).await;

        // 3. Run reconcile.
        let report = reconcile_and_reattach(&state).await.expect("reconcile ok");

        // 4. Assertions: the live row was reattached.
        assert!(
            report.reattached.contains(&live_session),
            "expected reattached to include {live_session:?}, got {:?}",
            report.reattached
        );
        assert!(
            !report.ended.contains(&live_session),
            "live session MUST NOT be ended"
        );

        // The row is still in a live status.
        let status: (String,) = sqlx::query_as("SELECT status FROM sessions WHERE id = ?1")
            .bind(live_session.get())
            .fetch_one(state.db.pool())
            .await
            .expect("status query");
        assert!(
            matches!(status.0.as_str(), "working" | "idle" | "needs_input"),
            "expected live status, got {}",
            status.0
        );

        // And the live-session handle is installed.
        let live_sessions = state.live_sessions.read().await;
        assert!(
            live_sessions.contains_key(&live_session),
            "expected live_sessions to contain a handle for the reattached session"
        );
        drop(live_sessions);

        // 5. SIGKILL the child and give the OS a moment to reap it, then
        //    seed another row with the now-defunct pid. sysinfo should report
        //    it as gone on the next refresh.
        live_child.kill().expect("kill sleep");
        live_child.wait().expect("wait sleep");

        // Give the OS a moment to finish reaping the child before the next
        // sysinfo refresh runs inside reconcile_and_reattach.
        std::thread::sleep(Duration::from_millis(50));

        let dead_session = common::seed_wrapper_session(&state, project_id, Some(live_pid)).await;

        // Re-run reconcile. This is an additional pass, not a re-entry of the
        // first one — the first one already installed a handle for the live
        // row, which is fine.
        let report2 = reconcile_and_reattach(&state)
            .await
            .expect("reconcile ok 2");
        assert!(
            report2.ended.contains(&dead_session),
            "expected dead session to be ended, got ended={:?}",
            report2.ended
        );

        let (status, reason): (String, Option<String>) =
            sqlx::query_as("SELECT status, error_reason FROM sessions WHERE id = ?1")
                .bind(dead_session.get())
                .fetch_one(state.db.pool())
                .await
                .expect("status query for dead");
        assert_eq!(status, "ended", "dead row must be ended");
        assert_eq!(
            reason.as_deref(),
            Some("workbench_restart"),
            "dead row must carry error_reason = workbench_restart"
        );
    }
}
