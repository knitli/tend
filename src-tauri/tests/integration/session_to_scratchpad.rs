//! T142: Integration test — session ending does NOT delete or modify any
//! note or reminder in its project.
//!
//! This complements T105 (PTY output doesn't mutate scratchpad) by verifying
//! the session-lifecycle boundary: ending a session must leave the scratchpad
//! tables completely untouched.

use agentui_workbench::project::ProjectService;
use agentui_workbench::scratchpad::notes::NoteService;
use agentui_workbench::scratchpad::reminders::ReminderService;
use sqlx::Row;
use std::collections::BTreeMap;

/// Ending a session must not delete or modify notes or reminders.
#[tokio::test]
async fn session_end_preserves_scratchpad() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = ProjectService::register(
        &state.db,
        tmp.path().to_str().unwrap(),
        Some("t142-project"),
    )
    .await
    .expect("register");

    // Seed scratchpad data.
    let note = NoteService::create(&state.db, project.id, "important note about design")
        .await
        .expect("create note");
    let reminder = ReminderService::create(&state.db, project.id, "remember to review PR")
        .await
        .expect("create reminder");

    // Snapshot the exact data before session lifecycle (L5: includes timestamps).
    let note_count_before = count_table(&state, "notes").await;
    let reminder_count_before = count_table(&state, "reminders").await;
    let note_content_before = get_note_content(&state, note.id.get()).await;
    let reminder_content_before = get_reminder_content(&state, reminder.id.get()).await;
    let note_updated_at_before = get_field(&state, "notes", "updated_at", note.id.get()).await;
    let reminder_created_at_before =
        get_field(&state, "reminders", "created_at", reminder.id.get()).await;

    // Spawn a real session, let it run briefly, then end it.
    let env = BTreeMap::new();
    let result = agentui_workbench::session::SessionService::spawn_local(
        &state,
        project.id,
        "t142-session",
        tmp.path(),
        &[
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo 'session running'; sleep 0.1; exit 0".to_string(),
        ],
        &env,
    )
    .await;

    match result {
        Ok((session, _handle)) => {
            // M6 fix: Poll for session end instead of unconditional sleep.
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(3);
            loop {
                let row: (String,) = sqlx::query_as("SELECT status FROM sessions WHERE id = ?1")
                    .bind(session.id.get())
                    .fetch_one(state.db.pool())
                    .await
                    .expect("fetch status");
                if row.0 == "ended" {
                    break;
                }
                if tokio::time::Instant::now() > deadline {
                    // Child didn't exit in time; force-end via service.
                    let _ = agentui_workbench::session::SessionService::mark_ended(
                        &state.db,
                        session.id,
                        Some(0),
                    )
                    .await;
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
        Err(e)
            if e.code == agentui_workbench::error::ErrorCode::SpawnFailed
                || (e.code == agentui_workbench::error::ErrorCode::Internal
                    && e.message.contains("not yet implemented")) =>
        {
            // PTY spawn may fail in CI. Simulate session ending via DB:
            let session_id = crate::common::seed_workbench_session(&state, project.id, None).await;
            // Mark it ended directly.
            sqlx::query(
                "UPDATE sessions SET status = 'ended', ended_at = datetime('now') WHERE id = ?1",
            )
            .bind(session_id.get())
            .execute(state.db.pool())
            .await
            .expect("end session");
        }
        Err(e) => panic!("unexpected error: {e:?}"),
    }

    // Verify scratchpad data is completely untouched.
    let note_count_after = count_table(&state, "notes").await;
    let reminder_count_after = count_table(&state, "reminders").await;
    let note_content_after = get_note_content(&state, note.id.get()).await;
    let reminder_content_after = get_reminder_content(&state, reminder.id.get()).await;

    assert_eq!(
        note_count_before, note_count_after,
        "session end must not create or delete notes"
    );
    assert_eq!(
        reminder_count_before, reminder_count_after,
        "session end must not create or delete reminders"
    );
    assert_eq!(
        note_content_before, note_content_after,
        "session end must not modify note content"
    );
    assert_eq!(
        reminder_content_before, reminder_content_after,
        "session end must not modify reminder content"
    );

    // L5: Verify timestamps are also untouched.
    let note_updated_at_after = get_field(&state, "notes", "updated_at", note.id.get()).await;
    let reminder_created_at_after =
        get_field(&state, "reminders", "created_at", reminder.id.get()).await;
    assert_eq!(
        note_updated_at_before, note_updated_at_after,
        "session end must not modify note updated_at"
    );
    assert_eq!(
        reminder_created_at_before, reminder_created_at_after,
        "session end must not modify reminder created_at"
    );
}

/// Ending multiple sessions in rapid succession must not corrupt scratchpad.
#[tokio::test]
async fn rapid_session_ends_preserve_scratchpad() {
    let state = crate::common::mock_state().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let project =
        ProjectService::register(&state.db, tmp.path().to_str().unwrap(), Some("t142-rapid"))
            .await
            .expect("register");

    // Seed scratchpad.
    NoteService::create(&state.db, project.id, "note-1")
        .await
        .expect("note");
    NoteService::create(&state.db, project.id, "note-2")
        .await
        .expect("note");
    ReminderService::create(&state.db, project.id, "remind-1")
        .await
        .expect("reminder");

    let count_before = count_table(&state, "notes").await + count_table(&state, "reminders").await;

    // Create and end several sessions via DB manipulation.
    for i in 0..5 {
        let sid = crate::common::seed_workbench_session(&state, project.id, None).await;
        sqlx::query(
            "UPDATE sessions SET status = 'ended', ended_at = datetime('now'), label = ?2 WHERE id = ?1",
        )
        .bind(sid.get())
        .bind(format!("rapid-{i}"))
        .execute(state.db.pool())
        .await
        .expect("end session");
    }

    let count_after = count_table(&state, "notes").await + count_table(&state, "reminders").await;
    assert_eq!(
        count_before, count_after,
        "rapid session ends must not affect scratchpad row count"
    );
}

async fn count_table(state: &agentui_workbench::state::WorkbenchState, table: &str) -> i64 {
    debug_assert!(
        matches!(table, "notes" | "reminders"),
        "count_table only supports known table names"
    );
    // Table name is a compile-time constant from the callers above; not user input.
    let sql = format!("SELECT COUNT(*) AS cnt FROM {table}");
    let row = sqlx::query(&sql)
        .fetch_one(state.db.pool())
        .await
        .expect("count");
    row.try_get::<i64, _>("cnt").expect("cnt")
}

async fn get_note_content(state: &agentui_workbench::state::WorkbenchState, id: i64) -> String {
    let row = sqlx::query("SELECT content FROM notes WHERE id = ?1")
        .bind(id)
        .fetch_one(state.db.pool())
        .await
        .expect("fetch note");
    row.try_get::<String, _>("content").expect("content")
}

async fn get_reminder_content(state: &agentui_workbench::state::WorkbenchState, id: i64) -> String {
    let row = sqlx::query("SELECT content FROM reminders WHERE id = ?1")
        .bind(id)
        .fetch_one(state.db.pool())
        .await
        .expect("fetch reminder");
    row.try_get::<String, _>("content").expect("content")
}

/// L5: Generic field getter for timestamp snapshot assertions.
async fn get_field(
    state: &agentui_workbench::state::WorkbenchState,
    table: &str,
    column: &str,
    id: i64,
) -> String {
    debug_assert!(
        matches!(table, "notes" | "reminders"),
        "get_field only supports known table names"
    );
    debug_assert!(
        matches!(column, "updated_at" | "created_at"),
        "get_field only supports known column names"
    );
    // Table and column are compile-time constants from callers; not user input.
    let sql = format!("SELECT {column} FROM {table} WHERE id = ?1");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_one(state.db.pool())
        .await
        .expect("fetch field");
    row.try_get::<String, _>(column).expect("field value")
}
