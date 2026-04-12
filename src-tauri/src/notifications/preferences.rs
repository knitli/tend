//! T075: Notification preference service — get/set with per-project override
//! and global fallback.
//!
//! Lookup cascade: project-specific row → global default (project_id = NULL).
//! During quiet hours: suppress OS-level channel; in-app alert still shows.

use crate::db::Database;
use crate::error::WorkbenchResult;
use crate::model::{
    NotificationChannel, NotificationPreference, NotificationPreferenceId, ProjectId, QuietHours,
};
use chrono::Utc;
use sqlx::Row;
use tracing::info;

/// Stateless preference service — operates on the shared DB.
pub struct PreferenceService;

impl PreferenceService {
    /// Get the effective notification preference for a project.
    ///
    /// If `project_id` is `Some`, looks for a project-specific row first.
    /// Falls back to the global default (project_id IS NULL). If no global
    /// default exists, creates one with `[InApp, OsNotification]` channels.
    pub async fn get(
        db: &Database,
        project_id: Option<ProjectId>,
    ) -> WorkbenchResult<NotificationPreference> {
        // Try project-specific first.
        if let Some(pid) = project_id {
            let row = sqlx::query(
                r#"
                SELECT id, project_id, channels_json, quiet_hours, updated_at
                FROM notification_preferences
                WHERE project_id = ?1
                "#,
            )
            .bind(pid.get())
            .fetch_optional(db.pool())
            .await?;

            if let Some(row) = row {
                return parse_preference_row(&row);
            }
        }

        // Fall back to global default.
        let row = sqlx::query(
            r#"
            SELECT id, project_id, channels_json, quiet_hours, updated_at
            FROM notification_preferences
            WHERE project_id IS NULL
            "#,
        )
        .fetch_optional(db.pool())
        .await?;

        match row {
            Some(row) => parse_preference_row(&row),
            None => {
                // Bootstrap: create the global default row.
                Self::create_global_default(db).await
            }
        }
    }

    /// Set notification preferences. If `project_id` is `None`, sets the global
    /// default. Upserts: creates or updates the row for the given scope.
    pub async fn set(
        db: &Database,
        project_id: Option<ProjectId>,
        channels: &[NotificationChannel],
        quiet_hours: Option<&QuietHours>,
    ) -> WorkbenchResult<NotificationPreference> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let channels_json = serde_json::to_string(channels)?;
        let quiet_hours_json = quiet_hours.map(serde_json::to_string).transpose()?;
        let pid = project_id.map(|p| p.get());

        // Check for existing row.
        let existing = match pid {
            Some(p) => {
                sqlx::query("SELECT id FROM notification_preferences WHERE project_id = ?1")
                    .bind(p)
                    .fetch_optional(db.pool())
                    .await?
            }
            None => {
                sqlx::query("SELECT id FROM notification_preferences WHERE project_id IS NULL")
                    .fetch_optional(db.pool())
                    .await?
            }
        };

        let pref_id = if let Some(row) = existing {
            let id: i64 = row.try_get("id")?;
            sqlx::query(
                r#"
                UPDATE notification_preferences
                SET channels_json = ?1, quiet_hours = ?2, updated_at = ?3
                WHERE id = ?4
                "#,
            )
            .bind(&channels_json)
            .bind(&quiet_hours_json)
            .bind(&now_str)
            .bind(id)
            .execute(db.pool())
            .await?;
            NotificationPreferenceId::new(id)
        } else {
            let result = sqlx::query(
                r#"
                INSERT INTO notification_preferences (project_id, channels_json, quiet_hours, updated_at)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )
            .bind(pid)
            .bind(&channels_json)
            .bind(&quiet_hours_json)
            .bind(&now_str)
            .execute(db.pool())
            .await?;
            NotificationPreferenceId::new(result.last_insert_rowid())
        };

        info!(?project_id, "notification preference updated");

        Ok(NotificationPreference {
            id: pref_id,
            project_id,
            channels: channels.to_vec(),
            quiet_hours: quiet_hours.cloned(),
            updated_at: now,
        })
    }

    /// Create the global default preference row.
    async fn create_global_default(db: &Database) -> WorkbenchResult<NotificationPreference> {
        let defaults = vec![
            NotificationChannel::InApp,
            NotificationChannel::OsNotification,
        ];
        Self::set(db, None, &defaults, None).await
    }
}

impl PreferenceService {
    /// Resolve the effective notification channels for a project, taking
    /// quiet hours into account.
    ///
    /// Returns the channel list with OS/bell channels filtered out if we're
    /// currently in quiet hours. Silent mode returns an empty list.
    pub async fn resolve_channels(
        db: &Database,
        project_id: ProjectId,
    ) -> WorkbenchResult<Vec<NotificationChannel>> {
        let pref = Self::get(db, Some(project_id)).await?;

        // Silent mode suppresses everything.
        if pref.channels.contains(&NotificationChannel::Silent) {
            return Ok(Vec::new());
        }

        // Check quiet hours.
        let in_quiet = pref
            .quiet_hours
            .as_ref()
            .map(is_quiet_hours)
            .unwrap_or(false);

        if in_quiet {
            // During quiet hours, suppress OS and bell channels; keep InApp.
            Ok(pref
                .channels
                .into_iter()
                .filter(|c| {
                    !matches!(
                        c,
                        NotificationChannel::OsNotification | NotificationChannel::TerminalBell
                    )
                })
                .collect())
        } else {
            Ok(pref.channels)
        }
    }
}

/// Check if we are currently in quiet hours.
pub fn is_quiet_hours(quiet_hours: &QuietHours) -> bool {
    let now = chrono::Local::now().time();

    let start = match chrono::NaiveTime::parse_from_str(&quiet_hours.start, "%H:%M") {
        Ok(t) => t,
        Err(_) => return false,
    };
    let end = match chrono::NaiveTime::parse_from_str(&quiet_hours.end, "%H:%M") {
        Ok(t) => t,
        Err(_) => return false,
    };

    if start <= end {
        // Same-day range: e.g., 09:00 - 17:00.
        // End is exclusive at minute granularity: 17:00 means quiet until 16:59:59.
        now >= start && now < end
    } else {
        // Overnight range: e.g., 22:00 - 07:00.
        // End is exclusive: 07:00 means quiet until 06:59:59.
        now >= start || now < end
    }
}

/// Parse a preference row from sqlx.
fn parse_preference_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<NotificationPreference> {
    let id: i64 = row.try_get("id")?;
    let project_id: Option<i64> = row.try_get("project_id")?;
    let channels_json: String = row.try_get("channels_json")?;
    let quiet_hours_str: Option<String> = row.try_get("quiet_hours")?;
    let updated_at_str: String = row.try_get("updated_at")?;

    let channels: Vec<NotificationChannel> = serde_json::from_str(&channels_json)?;
    let quiet_hours: Option<QuietHours> = quiet_hours_str
        .map(|s| serde_json::from_str(&s))
        .transpose()?;
    let updated_at = updated_at_str
        .parse()
        .map_err(|e| crate::error::WorkbenchError::internal(format!("invalid updated_at: {e}")))?;

    Ok(NotificationPreference {
        id: NotificationPreferenceId::new(id),
        project_id: project_id.map(ProjectId::new),
        channels,
        quiet_hours,
        updated_at,
    })
}
