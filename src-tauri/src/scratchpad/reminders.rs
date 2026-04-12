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
    pub async fn list(
        db: &Database,
        project_id: Option<ProjectId>,
        state: Option<ReminderState>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> WorkbenchResult<(Vec<Reminder>, Option<String>)> {
        let limit = limit.unwrap_or(50).min(200);
        let fetch_limit = limit + 1;

        // Build dynamic query based on filters.
        let mut sql = String::from(
            "SELECT id, project_id, content, state, created_at, done_at FROM reminders WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if let Some(pid) = project_id {
            binds.push(pid.get().to_string());
            sql.push_str(&format!(" AND project_id = ?{}", binds.len()));
        }
        if let Some(st) = state {
            binds.push(st.as_str().to_string());
            sql.push_str(&format!(" AND state = ?{}", binds.len()));
        }
        if let Some(c) = cursor {
            binds.push(c.to_string());
            sql.push_str(&format!(" AND created_at < ?{}", binds.len()));
        }

        binds.push(fetch_limit.to_string());
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{}", binds.len()));

        let mut query = sqlx::query(&sql);
        for b in &binds {
            query = query.bind(b);
        }

        let rows = query.fetch_all(db.pool()).await?;

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
            reminders.last().map(|r| r.created_at.to_rfc3339())
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

        let result = sqlx::query("UPDATE reminders SET state = ?1, done_at = ?2 WHERE id = ?3")
            .bind(state.as_str())
            .bind(done_at)
            .bind(id.get())
            .execute(db.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("reminder {id}")));
        }

        let row = sqlx::query(
            "SELECT id, project_id, content, state, created_at, done_at \
             FROM reminders WHERE id = ?1",
        )
        .bind(id.get())
        .fetch_one(db.pool())
        .await?;

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
