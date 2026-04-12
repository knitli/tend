//! T122: Workspace persistence — auto-save/restore of workspace state.

pub mod layouts;

use crate::db::Database;
use crate::error::WorkbenchResult;
use crate::model::WorkspaceState;
use chrono::Utc;
use sqlx::Row;

/// Workspace service — stateless, operates on the shared DB.
pub struct WorkspaceService;

impl WorkspaceService {
    /// Read the current workspace state. Returns `WorkspaceState::default()` if
    /// no row exists yet (first launch).
    pub async fn get(db: &Database) -> WorkbenchResult<WorkspaceState> {
        let row = sqlx::query("SELECT payload_json FROM workspace_state WHERE id = 1")
            .fetch_optional(db.pool())
            .await?;

        match row {
            Some(r) => {
                let json: String = r.try_get("payload_json")?;
                let state: WorkspaceState =
                    serde_json::from_str(&json).unwrap_or_default();
                Ok(state)
            }
            None => Ok(WorkspaceState::default()),
        }
    }

    /// Persist workspace state. Uses INSERT OR REPLACE on the single-row table.
    pub async fn save(db: &Database, state: &WorkspaceState) -> WorkbenchResult<()> {
        let json = serde_json::to_string(state)
            .map_err(|e| crate::error::WorkbenchError::internal(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT OR REPLACE INTO workspace_state (id, payload_json, saved_at) \
             VALUES (1, ?1, ?2)",
        )
        .bind(&json)
        .bind(&now)
        .execute(db.pool())
        .await?;

        Ok(())
    }
}
