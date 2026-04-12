//! T109: OverviewService — cross-project reminder rollup.

use crate::db::Database;
use crate::error::WorkbenchResult;
use crate::model::{Project, ProjectId, Reminder, Timestamp};
use crate::scratchpad::reminders::parse_reminder_row;
use sqlx::Row;

/// A group in the cross-project overview.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OverviewGroup {
    /// The project.
    pub project: Project,
    /// Open reminders for this project, ordered created_at DESC.
    pub open_reminders: Vec<Reminder>,
}

/// Overview service — stateless, operates on the shared DB.
pub struct OverviewService;

impl OverviewService {
    /// Get the cross-project overview: open reminders grouped by project,
    /// ordered by project display_name, then reminder created_at DESC.
    /// Excludes archived projects.
    pub async fn overview(db: &Database) -> WorkbenchResult<Vec<OverviewGroup>> {
        // Fetch all non-archived projects that have open reminders.
        let rows = sqlx::query(
            "SELECT r.id, r.project_id, r.content, r.state, r.created_at, r.done_at, \
                    p.id AS p_id, p.canonical_path, p.display_name, p.added_at, \
                    p.archived_at, p.settings_json \
             FROM reminders r \
             JOIN projects p ON p.id = r.project_id \
             WHERE r.state = 'open' AND p.archived_at IS NULL \
             ORDER BY p.display_name ASC, r.created_at DESC",
        )
        .fetch_all(db.pool())
        .await?;

        let mut groups: Vec<OverviewGroup> = Vec::new();
        let mut current_project_id: Option<i64> = None;

        for row in &rows {
            let p_id: i64 = row.try_get("p_id")?;

            if current_project_id != Some(p_id) {
                current_project_id = Some(p_id);
                groups.push(OverviewGroup {
                    project: parse_project_row(row)?,
                    open_reminders: Vec::new(),
                });
            }

            let reminder = parse_reminder_row(row)?;
            if let Some(group) = groups.last_mut() {
                group.open_reminders.push(reminder);
            }
        }

        Ok(groups)
    }
}

fn parse_project_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Project> {
    let id: i64 = row.try_get("p_id")?;
    let canonical_path: String = row.try_get("canonical_path")?;
    let display_name: String = row.try_get("display_name")?;
    let added_at: String = row.try_get("added_at")?;
    let archived_at: Option<String> = row.try_get("archived_at")?;
    let settings_json: String = row.try_get("settings_json")?;

    Ok(Project {
        id: ProjectId::new(id),
        canonical_path: std::path::PathBuf::from(canonical_path),
        display_name,
        added_at: parse_ts(&added_at)?,
        last_active_at: None, // Not needed for overview display.
        archived_at: archived_at.as_deref().map(parse_ts).transpose()?,
        settings: serde_json::from_str(&settings_json).unwrap_or_default(),
    })
}

fn parse_ts(s: &str) -> WorkbenchResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| {
            crate::error::WorkbenchError::internal(format!("invalid timestamp '{s}': {e}"))
        })
}
