//! T123: LayoutService — CRUD for named workspace layouts.

use crate::db::Database;
use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{Layout, LayoutId, SessionId, WorkspaceState};
use crate::state::WorkbenchState as AppState;
use chrono::Utc;
use sqlx::Row;

/// Layout service — stateless for list/save/delete, needs AppState for restore
/// (to check which sessions are still alive).
pub struct LayoutService;

impl LayoutService {
    /// List all saved layouts, ordered by name ASC.
    pub async fn list(db: &Database) -> WorkbenchResult<Vec<Layout>> {
        let rows = sqlx::query(
            "SELECT id, name, payload_json, created_at, updated_at \
             FROM layouts ORDER BY name ASC",
        )
        .fetch_all(db.pool())
        .await?;

        rows.iter().map(parse_layout_row).collect()
    }

    /// Save a named layout. Returns `NAME_TAKEN` if the name already exists
    /// and `overwrite` is false. When `overwrite` is true, an existing layout
    /// with the same name is updated in place.
    ///
    /// H1 fix: atomic INSERT with UNIQUE catch (no SELECT-then-INSERT race).
    /// H2 fix: `overwrite` parameter enables the UI confirm-then-replace flow.
    pub async fn save(
        db: &Database,
        name: &str,
        state: &WorkspaceState,
        overwrite: bool,
    ) -> WorkbenchResult<Layout> {
        let name = name.trim();
        if name.is_empty() {
            return Err(WorkbenchError::new(
                ErrorCode::ContentEmpty,
                "layout name must not be empty",
            ));
        }
        if name.len() > 255 {
            return Err(WorkbenchError::new(
                ErrorCode::ContentEmpty,
                "layout name must not exceed 255 characters",
            ));
        }

        let json =
            serde_json::to_string(state).map_err(|e| WorkbenchError::internal(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        if overwrite {
            // Upsert: try UPDATE first, then INSERT if no row matched.
            let updated = sqlx::query(
                "UPDATE layouts SET payload_json = ?1, updated_at = ?2 WHERE name = ?3",
            )
            .bind(&json)
            .bind(&now)
            .bind(name)
            .execute(db.pool())
            .await?;

            if updated.rows_affected() > 0 {
                // Fetch the updated row.
                let row = sqlx::query(
                    "SELECT id, name, payload_json, created_at, updated_at \
                     FROM layouts WHERE name = ?1",
                )
                .bind(name)
                .fetch_one(db.pool())
                .await?;
                return parse_layout_row(&row);
            }
            // Fall through to INSERT if the name didn't exist.
        }

        // Atomic INSERT — catch UNIQUE violation as NAME_TAKEN.
        let result = sqlx::query(
            "INSERT INTO layouts (name, payload_json, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?3)",
        )
        .bind(name)
        .bind(&json)
        .bind(&now)
        .execute(db.pool())
        .await;

        match result {
            Ok(r) => Ok(Layout {
                id: LayoutId::new(r.last_insert_rowid()),
                name: name.to_string(),
                payload: state.clone(),
                created_at: parse_ts(&now)?,
                updated_at: parse_ts(&now)?,
            }),
            Err(sqlx::Error::Database(ref e)) if e.is_unique_violation() => {
                Err(WorkbenchError::new(
                    ErrorCode::NameTaken,
                    format!("layout name '{name}' is already in use"),
                ))
            }
            Err(e) => Err(WorkbenchError::from(e)),
        }
    }

    /// Restore a layout by id. Returns the workspace state and a list of
    /// session ids referenced in the state that are no longer alive in the
    /// live-sessions map.
    pub async fn restore(
        app_state: &AppState,
        id: LayoutId,
    ) -> WorkbenchResult<(WorkspaceState, Vec<SessionId>)> {
        let row = sqlx::query(
            "SELECT id, name, payload_json, created_at, updated_at \
             FROM layouts WHERE id = ?1",
        )
        .bind(id.get())
        .fetch_optional(app_state.db.pool())
        .await?;

        let row = row.ok_or_else(|| WorkbenchError::not_found(format!("layout {id}")))?;
        let layout = parse_layout_row(&row)?;

        // Check which referenced sessions are still alive.
        let live = app_state.live_sessions.read().await;
        let mut missing = Vec::new();

        if let Some(sid) = layout.payload.focused_session_id {
            if !live.contains_key(&sid) {
                missing.push(sid);
            }
        }

        Ok((layout.payload, missing))
    }

    /// Delete a layout by id.
    pub async fn delete(db: &Database, id: LayoutId) -> WorkbenchResult<()> {
        let result = sqlx::query("DELETE FROM layouts WHERE id = ?1")
            .bind(id.get())
            .execute(db.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("layout {id}")));
        }

        Ok(())
    }
}

fn parse_layout_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Layout> {
    let id: i64 = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let payload_json: String = row.try_get("payload_json")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    let payload: WorkspaceState = match serde_json::from_str(&payload_json) {
        Ok(ws) => ws,
        Err(e) => {
            tracing::warn!(
                error = %e,
                layout_id = id,
                raw_json = %payload_json,
                "layout payload deserialization failed, returning default"
            );
            WorkspaceState::default()
        }
    };

    Ok(Layout {
        id: LayoutId::new(id),
        name,
        payload,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

fn parse_ts(s: &str) -> WorkbenchResult<crate::model::Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| WorkbenchError::internal(format!("invalid timestamp '{s}': {e}")))
}
