//! Project registration, lookup, archive/unarchive.
//!
//! T042: `ProjectService` — canonicalizes paths, detects duplicates, manages the
//! lifecycle of registered projects. All queries go through the shared
//! `WorkbenchState` database handle.

pub mod watcher;

use crate::db::Database;
use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{Project, ProjectId, ProjectSettings, Timestamp};
use chrono::Utc;
use sqlx::Row;
use std::path::Path;
use tracing::info;

/// Project service — stateless, operates on the shared DB.
pub struct ProjectService;

impl ProjectService {
    /// Register a new project at `path`.
    ///
    /// - Canonicalizes `path` (must exist, must be a directory).
    /// - If a non-archived project with the same canonical path already exists,
    ///   returns `ALREADY_REGISTERED` with the existing project.
    /// - Otherwise inserts a new row.
    pub async fn register(
        db: &Database,
        path: &str,
        display_name: Option<&str>,
    ) -> WorkbenchResult<Project> {
        let raw = Path::new(path);

        // Canonicalize — also proves the path exists on disk.
        let canonical = std::fs::canonicalize(raw).map_err(|_| {
            WorkbenchError::new(
                ErrorCode::PathNotFound,
                format!("path does not exist: {path}"),
            )
        })?;

        if !canonical.is_dir() {
            return Err(WorkbenchError::new(
                ErrorCode::PathNotADirectory,
                format!("path is not a directory: {}", canonical.display()),
            ));
        }

        let canonical_str = canonical.to_string_lossy().to_string();
        let default_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();
        let name = display_name.unwrap_or(default_name.as_str());

        // Check for existing non-archived project with same canonical path.
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM projects WHERE canonical_path = ?1 AND archived_at IS NULL",
        )
        .bind(&canonical_str)
        .fetch_optional(db.pool())
        .await?;

        if let Some((id,)) = existing {
            return Err(WorkbenchError::with_details(
                ErrorCode::AlreadyRegistered,
                "project already registered",
                serde_json::json!({ "project_id": id }),
            ));
        }

        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO projects (canonical_path, display_name, added_at, settings_json)
            VALUES (?1, ?2, ?3, '{}')
            RETURNING id
            "#,
        )
        .bind(&canonical_str)
        .bind(name)
        .bind(&now_str)
        .fetch_one(db.pool())
        .await?;

        let project = Project {
            id: ProjectId::new(row.0),
            canonical_path: canonical,
            display_name: name.to_string(),
            added_at: now,
            last_active_at: None,
            archived_at: None,
            settings: ProjectSettings::default(),
        };

        info!(
            project_id = %project.id,
            path = %project.canonical_path.display(),
            "registered project"
        );

        Ok(project)
    }

    /// Ensure a project exists for the given path, creating one if needed.
    /// Used by the daemon `register_session` handler (T051).
    pub async fn ensure_exists(db: &Database, path: &str) -> WorkbenchResult<Project> {
        match Self::register(db, path, None).await {
            Ok(project) => Ok(project),
            Err(e) if e.code == ErrorCode::AlreadyRegistered => {
                // Extract project_id from details.
                let id = e
                    .details
                    .as_ref()
                    .and_then(|d| d.get("project_id"))
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        WorkbenchError::internal("ALREADY_REGISTERED missing project_id in details")
                    })?;
                Self::get_by_id(db, ProjectId::new(id)).await
            }
            Err(e) => Err(e),
        }
    }

    /// List projects with optional include_archived.
    pub async fn list(db: &Database, include_archived: bool) -> WorkbenchResult<Vec<Project>> {
        let sql = if include_archived {
            "SELECT id, canonical_path, display_name, added_at, last_active_at, archived_at, settings_json FROM projects ORDER BY display_name"
        } else {
            "SELECT id, canonical_path, display_name, added_at, last_active_at, archived_at, settings_json FROM projects WHERE archived_at IS NULL ORDER BY display_name"
        };

        let rows = sqlx::query(sql).fetch_all(db.pool()).await?;

        rows.iter()
            .map(row_to_project)
            .collect::<Result<Vec<_>, _>>()
    }

    /// Get a project by id.
    pub async fn get_by_id(db: &Database, id: ProjectId) -> WorkbenchResult<Project> {
        let row = sqlx::query(
            "SELECT id, canonical_path, display_name, added_at, last_active_at, archived_at, settings_json FROM projects WHERE id = ?1",
        )
        .bind(id.get())
        .fetch_optional(db.pool())
        .await?
        .ok_or_else(|| WorkbenchError::not_found(format!("project {id}")))?;

        row_to_project(&row)
    }

    /// Update display_name and/or settings.
    pub async fn update(
        db: &Database,
        id: ProjectId,
        display_name: Option<&str>,
        settings: Option<&ProjectSettings>,
    ) -> WorkbenchResult<Project> {
        // Verify exists first.
        Self::get_by_id(db, id).await?;

        if let Some(name) = display_name {
            sqlx::query("UPDATE projects SET display_name = ?1 WHERE id = ?2")
                .bind(name)
                .bind(id.get())
                .execute(db.pool())
                .await?;
        }

        if let Some(settings) = settings {
            let json = serde_json::to_string(settings)?;
            sqlx::query("UPDATE projects SET settings_json = ?1 WHERE id = ?2")
                .bind(&json)
                .bind(id.get())
                .execute(db.pool())
                .await?;
        }

        Self::get_by_id(db, id).await
    }

    /// Archive (soft-delete) a project.
    pub async fn archive(db: &Database, id: ProjectId) -> WorkbenchResult<()> {
        let project = Self::get_by_id(db, id).await?;
        if project.archived_at.is_some() {
            // Already archived — idempotent.
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();

        // End all live sessions for this project.
        sqlx::query(
            r#"
            UPDATE sessions
            SET status = 'ended', ended_at = ?1, error_reason = 'project_archived', pid = NULL
            WHERE project_id = ?2 AND status IN ('working','idle','needs_input') AND ended_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(id.get())
        .execute(db.pool())
        .await?;

        // M3: Clear open alerts for all sessions belonging to this project.
        sqlx::query(
            r#"
            UPDATE alerts SET acknowledged_at = ?1, cleared_by = 'session_ended'
            WHERE session_id IN (SELECT id FROM sessions WHERE project_id = ?2)
              AND acknowledged_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(id.get())
        .execute(db.pool())
        .await?;

        sqlx::query("UPDATE projects SET archived_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(id.get())
            .execute(db.pool())
            .await?;

        info!(%id, "archived project");
        Ok(())
    }

    /// Unarchive a project.
    pub async fn unarchive(db: &Database, id: ProjectId) -> WorkbenchResult<Project> {
        let project = Self::get_by_id(db, id).await?;
        if project.archived_at.is_none() {
            return Err(WorkbenchError::new(
                ErrorCode::NotArchived,
                format!("project {id} is not archived"),
            ));
        }

        // Verify the path still exists.
        if !project.canonical_path.exists() {
            return Err(WorkbenchError::new(
                ErrorCode::PathNotFound,
                format!(
                    "original path {} no longer exists",
                    project.canonical_path.display()
                ),
            ));
        }

        sqlx::query("UPDATE projects SET archived_at = NULL WHERE id = ?1")
            .bind(id.get())
            .execute(db.pool())
            .await?;

        Self::get_by_id(db, id).await
    }
}

fn row_to_project(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Project> {
    let id: i64 = row.try_get("id")?;
    let canonical_path: String = row.try_get("canonical_path")?;
    let display_name: String = row.try_get("display_name")?;
    let added_at: String = row.try_get("added_at")?;
    let last_active_at: Option<String> = row.try_get("last_active_at")?;
    let archived_at: Option<String> = row.try_get("archived_at")?;
    let settings_json: String = row.try_get("settings_json")?;

    Ok(Project {
        id: ProjectId::new(id),
        canonical_path: canonical_path.into(),
        display_name,
        added_at: parse_timestamp(&added_at)?,
        last_active_at: last_active_at.as_deref().map(parse_timestamp).transpose()?,
        archived_at: archived_at.as_deref().map(parse_timestamp).transpose()?,
        settings: serde_json::from_str(&settings_json).unwrap_or_default(),
    })
}

fn parse_timestamp(s: &str) -> WorkbenchResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| WorkbenchError::internal(format!("invalid timestamp '{s}': {e}")))
}
