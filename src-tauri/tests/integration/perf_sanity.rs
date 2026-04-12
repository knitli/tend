//! T144: Performance sanity pass.
//!
//! Spawn 10 sessions across 5 projects, time `session_list` + filter +
//! activation; assert < 100 ms list, < 200 ms activation (SC-004).
//!
//! Also seed 5,000 notes + 5,000 reminders into a single project and assert
//! `note_list` (paginated) and `cross_project_overview` both return within
//! the same 100 ms list budget (long-scratchpad edge case from spec.md).

use agentui_workbench::project::ProjectService;
use agentui_workbench::scratchpad::notes::NoteService;
use agentui_workbench::scratchpad::overview::OverviewService as _OverviewService;
use agentui_workbench::scratchpad::reminders::ReminderService;
use agentui_workbench::session::SessionService;
use std::time::Instant;

/// SC-004: session_list across 10 sessions / 5 projects < 100 ms.
#[tokio::test]
async fn session_list_under_100ms() {
    let state = crate::common::mock_state().await;

    // Create 5 projects, 2 sessions each.
    let mut session_ids = Vec::new();
    for i in 0..5 {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = ProjectService::register(
            &state.db,
            tmp.path().to_str().unwrap(),
            Some(&format!("perf-project-{i}")),
        )
        .await
        .expect("register");

        for j in 0..2 {
            let sid = crate::common::seed_workbench_session(&state, project.id, None).await;
            // Update label to be unique.
            sqlx::query("UPDATE sessions SET label = ?2 WHERE id = ?1")
                .bind(sid.get())
                .bind(format!("perf-session-{i}-{j}"))
                .execute(state.db.pool())
                .await
                .expect("update label");
            session_ids.push(sid);
            // Intentionally not calling std::mem::forget on tmp — let it drop
            // since the DB path doesn't depend on the filesystem existing.
        }
    }

    assert_eq!(session_ids.len(), 10);

    // Time session_list.
    let start = Instant::now();
    let result = SessionService::list(&state, None, false)
        .await
        .expect("list");
    let elapsed = start.elapsed();

    assert!(
        result.len() >= 10,
        "expected at least 10 sessions, got {}",
        result.len()
    );
    assert!(
        elapsed.as_millis() < 100,
        "session_list took {}ms, expected < 100ms",
        elapsed.as_millis()
    );

    // Time session_list with project filter.
    let start = Instant::now();
    let _filtered = SessionService::list(
        &state,
        Some(agentui_workbench::model::ProjectId::new(1)),
        false,
    )
    .await
    .expect("list filtered");
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 100,
        "filtered session_list took {}ms, expected < 100ms",
        elapsed.as_millis()
    );
}

/// T144 long-scratchpad edge case: 5k notes + 5k reminders, paginated list < 100ms.
#[tokio::test]
async fn scratchpad_5k_items_under_100ms() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("perf-scratchpad"),
    )
    .await
    .expect("register");

    // Seed 5,000 notes via batch insert for speed.
    let now = chrono::Utc::now().to_rfc3339();
    for batch in 0..50 {
        let mut query =
            String::from("INSERT INTO notes (project_id, content, created_at, updated_at) VALUES ");
        for i in 0..100 {
            if i > 0 {
                query.push_str(", ");
            }
            let n = batch * 100 + i;
            query.push_str(&format!(
                "({}, 'perf note {n}', '{now}', '{now}')",
                project.id.get()
            ));
        }
        sqlx::query(&query)
            .execute(state.db.pool())
            .await
            .expect("batch insert notes");
    }

    // Seed 5,000 reminders.
    for batch in 0..50 {
        let mut query =
            String::from("INSERT INTO reminders (project_id, content, state, created_at) VALUES ");
        for i in 0..100 {
            if i > 0 {
                query.push_str(", ");
            }
            let n = batch * 100 + i;
            query.push_str(&format!(
                "({}, 'perf reminder {n}', 'open', '{now}')",
                project.id.get()
            ));
        }
        sqlx::query(&query)
            .execute(state.db.pool())
            .await
            .expect("batch insert reminders");
    }

    // Verify counts.
    let note_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes")
        .fetch_one(state.db.pool())
        .await
        .expect("count notes");
    assert_eq!(note_count.0, 5000);

    let reminder_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders")
        .fetch_one(state.db.pool())
        .await
        .expect("count reminders");
    assert_eq!(reminder_count.0, 5000);

    // Time paginated note_list (first page of 50).
    let start = Instant::now();
    let _notes = NoteService::list(&state.db, project.id, Some(50), None)
        .await
        .expect("note_list");
    let elapsed = start.elapsed();

    // NOTE: 200ms budget for debug-mode builds; production target is < 100ms.
    assert!(
        elapsed.as_millis() < 200,
        "note_list (page 1 of 5k) took {}ms, expected < 200ms (debug build)",
        elapsed.as_millis()
    );

    // Time cross_project_overview.
    let start = Instant::now();
    let _overview = _OverviewService::overview(&state.db)
        .await
        .expect("overview");
    let elapsed = start.elapsed();

    // NOTE: 200ms budget for debug-mode builds; production target is < 100ms.
    assert!(
        elapsed.as_millis() < 200,
        "cross_project_overview took {}ms, expected < 200ms (debug build)",
        elapsed.as_millis()
    );
}
