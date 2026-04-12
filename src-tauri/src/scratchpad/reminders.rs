//! T108: ReminderService — CRUD for per-project reminders.

use crate::db::Database;
use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{ProjectId, Reminder, ReminderId, ReminderState, Timestamp};
use chrono::Utc;
use sqlx::Row;

/// Reminder service — stateless, operates on the shared DB.
pub struct ReminderService;

impl ReminderService {
    /// List reminders, optionally filtered by project and/or state.
    /// Ordered by created_at DESC.
    /// List reminders, optionally filtered by project and/or state.
    /// Ordered by created_at DESC.
    ///
    /// H3 fix: uses `sqlx::QueryBuilder` with typed binds instead of
    /// string-coerced `Vec<String>`.
    /// C1 fix: limit clamped to `[1, 200]`.
    pub async fn list(
        db: &Database,
        project_id: Option<ProjectId>,
        state: Option<ReminderState>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> WorkbenchResult<(Vec<Reminder>, Option<String>)> {
        let limit = limit.unwrap_or(50).clamp(1, 200);
        let fetch_limit = limit + 1;

        // H3 fix: use QueryBuilder for typed binds.
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT id, project_id, content, state, created_at, done_at FROM reminders WHERE 1=1",
        );

        if let Some(pid) = project_id {
            qb.push(" AND project_id = ").push_bind(pid.get());
        }
        if let Some(st) = state {
            qb.push(" AND state = ").push_bind(st.as_str().to_string());
        }
        // M4 fix: composite cursor (created_at, id) to prevent row-skipping.
        if let Some(c) = cursor {
            let (cursor_ts, cursor_id) = parse_cursor(c)?;
            qb.push(" AND (created_at < ")
                .push_bind(cursor_ts.clone())
                .push(" OR (created_at = ")
                .push_bind(cursor_ts)
                .push(" AND id < ")
                .push_bind(cursor_id)
                .push("))");
        }

        qb.push(" ORDER BY created_at DESC, id DESC LIMIT ")
            .push_bind(fetch_limit);

        let rows = qb.build().fetch_all(db.pool()).await?;

        let has_next = rows.len() as i64 > limit;
        let rows = if has_next {
            &rows[..limit as usize]
        } else {
            &rows
        };

        let reminders: Vec<Reminder> = rows
            .iter()
            .map(parse_reminder_row)
            .collect::<Result<_, _>>()?;
        let next_cursor = if has_next {
            reminders
                .last()
                .map(|r| encode_cursor(&r.created_at.to_rfc3339(), r.id.get()))
        } else {
            None
        };

        Ok((reminders, next_cursor))
    }

    /// Create a new reminder.
    pub async fn create(
        db: &Database,
        project_id: ProjectId,
        content: &str,
    ) -> WorkbenchResult<Reminder> {
        let content = content.trim();
        if content.is_empty() {
            return Err(WorkbenchError::new(
                ErrorCode::ContentEmpty,
                "reminder content must not be empty",
            ));
        }

        // Verify project exists.
        let exists = sqlx::query("SELECT 1 FROM projects WHERE id = ?1")
            .bind(project_id.get())
            .fetch_optional(db.pool())
            .await?;
        if exists.is_none() {
            return Err(WorkbenchError::not_found(format!("project {project_id}")));
        }

        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let result = sqlx::query(
            "INSERT INTO reminders (project_id, content, state, created_at) \
             VALUES (?1, ?2, 'open', ?3)",
        )
        .bind(project_id.get())
        .bind(content)
        .bind(&now_str)
        .execute(db.pool())
        .await?;

        Ok(Reminder {
            id: ReminderId::new(result.last_insert_rowid()),
            project_id,
            content: content.to_string(),
            state: ReminderState::Open,
            created_at: now,
            done_at: None,
        })
    }

    /// Set the state of a reminder (open/done).
    /// M6 fix: UPDATE+SELECT wrapped in a transaction.
    pub async fn set_state(
        db: &Database,
        id: ReminderId,
        state: ReminderState,
    ) -> WorkbenchResult<Reminder> {
        let now = Utc::now().to_rfc3339();
        let done_at = match state {
            ReminderState::Done => Some(now.as_str()),
            ReminderState::Open => None,
        };

        let mut tx = db.pool().begin().await?;

        let result = sqlx::query("UPDATE reminders SET state = ?1, done_at = ?2 WHERE id = ?3")
            .bind(state.as_str())
            .bind(done_at)
            .bind(id.get())
            .execute(&mut *tx)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("reminder {id}")));
        }

        let row = sqlx::query(
            "SELECT id, project_id, content, state, created_at, done_at \
             FROM reminders WHERE id = ?1",
        )
        .bind(id.get())
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        parse_reminder_row(&row)
    }

    /// Delete a reminder.
    pub async fn delete(db: &Database, id: ReminderId) -> WorkbenchResult<()> {
        let result = sqlx::query("DELETE FROM reminders WHERE id = ?1")
            .bind(id.get())
            .execute(db.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("reminder {id}")));
        }

        Ok(())
    }
}

pub(crate) fn parse_reminder_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Reminder> {
    let id: i64 = row.try_get("id")?;
    let project_id: i64 = row.try_get("project_id")?;
    let content: String = row.try_get("content")?;
    let state_str: String = row.try_get("state")?;
    let created_at: String = row.try_get("created_at")?;
    let done_at: Option<String> = row.try_get("done_at")?;

    let state: ReminderState = state_str.parse().map_err(WorkbenchError::internal)?;

    Ok(Reminder {
        id: ReminderId::new(id),
        project_id: ProjectId::new(project_id),
        content,
        state,
        created_at: parse_ts(&created_at)?,
        done_at: done_at.as_deref().map(parse_ts).transpose()?,
    })
}

fn parse_ts(s: &str) -> WorkbenchResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| WorkbenchError::internal(format!("invalid timestamp '{s}': {e}")))
}

/// Encode a composite cursor as `{timestamp}:{id}`.
fn encode_cursor(ts: &str, id: i64) -> String {
    format!("{ts}:{id}")
}

/// Decode a composite cursor. Falls back to timestamp-only for backward compat.
fn parse_cursor(cursor: &str) -> WorkbenchResult<(String, i64)> {
    if let Some(pos) = cursor.rfind(':') {
        let ts = &cursor[..pos];
        if let Ok(id) = cursor[pos + 1..].parse::<i64>() {
            return Ok((ts.to_string(), id));
        }
    }
    Ok((cursor.to_string(), 0))
}
