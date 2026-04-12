//! T107: NoteService — CRUD for per-project notes.

use crate::db::Database;
use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{Note, NoteId, ProjectId, Timestamp};
use chrono::Utc;
use sqlx::Row;

/// Note service — stateless, operates on the shared DB.
pub struct NoteService;

impl NoteService {
    /// List notes for a project, ordered by created_at DESC.
    ///
    /// Supports cursor-based pagination: `cursor` is the `created_at` ISO-8601
    /// timestamp of the last note on the previous page.
    pub async fn list(
        db: &Database,
        project_id: ProjectId,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> WorkbenchResult<(Vec<Note>, Option<String>)> {
        let limit = limit.unwrap_or(50).min(200);
        // Fetch one extra to detect if there's a next page.
        let fetch_limit = limit + 1;

        let rows = if let Some(cursor) = cursor {
            sqlx::query(
                "SELECT id, project_id, content, created_at, updated_at \
                 FROM notes WHERE project_id = ?1 AND created_at < ?2 \
                 ORDER BY created_at DESC LIMIT ?3",
            )
            .bind(project_id.get())
            .bind(cursor)
            .bind(fetch_limit)
            .fetch_all(db.pool())
            .await?
        } else {
            sqlx::query(
                "SELECT id, project_id, content, created_at, updated_at \
                 FROM notes WHERE project_id = ?1 \
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .bind(project_id.get())
            .bind(fetch_limit)
            .fetch_all(db.pool())
            .await?
        };

        let has_next = rows.len() as i64 > limit;
        let rows = if has_next {
            &rows[..limit as usize]
        } else {
            &rows
        };

        let notes: Vec<Note> = rows
            .iter()
            .map(parse_note_row)
            .collect::<Result<_, _>>()?;
        let next_cursor = if has_next {
            notes.last().map(|n| n.created_at.to_rfc3339())
        } else {
            None
        };

        Ok((notes, next_cursor))
    }

    /// Create a new note. Rejects empty/whitespace-only content with CONTENT_EMPTY.
    pub async fn create(
        db: &Database,
        project_id: ProjectId,
        content: &str,
    ) -> WorkbenchResult<Note> {
        let content = content.trim();
        if content.is_empty() {
            return Err(WorkbenchError::new(
                ErrorCode::ContentEmpty,
                "note content must not be empty",
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
            "INSERT INTO notes (project_id, content, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(project_id.get())
        .bind(content)
        .bind(&now_str)
        .bind(&now_str)
        .execute(db.pool())
        .await?;

        Ok(Note {
            id: NoteId::new(result.last_insert_rowid()),
            project_id,
            content: content.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Update a note's content.
    pub async fn update(db: &Database, id: NoteId, content: &str) -> WorkbenchResult<Note> {
        let content = content.trim();
        if content.is_empty() {
            return Err(WorkbenchError::new(
                ErrorCode::ContentEmpty,
                "note content must not be empty",
            ));
        }

        let now = Utc::now().to_rfc3339();

        let result = sqlx::query("UPDATE notes SET content = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(content)
            .bind(&now)
            .bind(id.get())
            .execute(db.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("note {id}")));
        }

        // Fetch the updated row.
        let row = sqlx::query(
            "SELECT id, project_id, content, created_at, updated_at FROM notes WHERE id = ?1",
        )
        .bind(id.get())
        .fetch_one(db.pool())
        .await?;

        parse_note_row(&row)
    }

    /// Delete a note.
    pub async fn delete(db: &Database, id: NoteId) -> WorkbenchResult<()> {
        let result = sqlx::query("DELETE FROM notes WHERE id = ?1")
            .bind(id.get())
            .execute(db.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("note {id}")));
        }

        Ok(())
    }
}

fn parse_note_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Note> {
    let id: i64 = row.try_get("id")?;
    let project_id: i64 = row.try_get("project_id")?;
    let content: String = row.try_get("content")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    Ok(Note {
        id: NoteId::new(id),
        project_id: ProjectId::new(project_id),
        content,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

fn parse_ts(s: &str) -> WorkbenchResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| WorkbenchError::internal(format!("invalid timestamp '{s}': {e}")))
}
