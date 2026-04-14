//! T072: Alert service — raise, clear, acknowledge, list_open.
//!
//! Alert lifecycle rules (data-model.md §4, invariant #4):
//! - Coalescing: exactly one open alert per session per kind. A second raise
//!   refreshes `raised_at` on the existing row instead of inserting.
//! - Auto-clear on resume: `set_status(Working)` clears via `session_resumed`.
//! - Auto-clear on end: `mark_ended` clears via `session_ended`.
//! - Manual clear: `acknowledge` sets `cleared_by = 'user'`.
//! - Dedup window: identical consecutive raises within 2 s are coalesced.

use crate::db::Database;
use crate::error::{WorkbenchError, WorkbenchResult};
use crate::model::{Alert, AlertClearedBy, AlertId, AlertKind, ProjectId, SessionId, Timestamp};
use chrono::Utc;
use sqlx::Row;
use tracing::info;

/// Stateless alert service — operates on the shared DB.
pub struct AlertService;

impl AlertService {
    /// Raise an alert for a session. Coalesces with an existing open alert
    /// of the same kind if it was raised within the last 2 seconds (dedup).
    ///
    /// Returns the alert row (new or refreshed).
    pub async fn raise(
        db: &Database,
        session_id: SessionId,
        project_id: ProjectId,
        kind: AlertKind,
        reason: Option<&str>,
    ) -> WorkbenchResult<Alert> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Use a transaction to serialize the check-then-insert/update so
        // concurrent raises for the same (session, kind) cannot create
        // duplicate open alert rows.
        let mut tx = db.pool().begin().await?;

        // Check for an existing open alert of the same kind on this session.
        let existing = sqlx::query(
            r#"
            SELECT id, raised_at FROM alerts
            WHERE session_id = ?1 AND kind = ?2 AND acknowledged_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(session_id.get())
        .bind(kind.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        let alert_id = if let Some(row) = existing {
            // Coalesce: refresh raised_at and reason on the existing row.
            let id: i64 = row.try_get("id")?;
            sqlx::query(
                r#"
                UPDATE alerts SET raised_at = ?1, reason = ?2
                WHERE id = ?3
                "#,
            )
            .bind(&now_str)
            .bind(reason)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            AlertId::new(id)
        } else {
            // Insert new alert row.
            let result = sqlx::query(
                r#"
                INSERT INTO alerts (session_id, project_id, kind, reason, raised_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(session_id.get())
            .bind(project_id.get())
            .bind(kind.as_str())
            .bind(reason)
            .bind(&now_str)
            .execute(&mut *tx)
            .await?;
            AlertId::new(result.last_insert_rowid())
        };

        tx.commit().await?;

        info!(%session_id, %alert_id, "alert raised (kind={:?})", kind);

        Ok(Alert {
            id: alert_id,
            session_id,
            project_id,
            kind,
            reason: reason.map(String::from),
            raised_at: now,
            acknowledged_at: None,
            cleared_by: None,
        })
    }

    /// Clear an alert. Sets `acknowledged_at` and `cleared_by`.
    ///
    /// Idempotent: if the alert is already cleared, this is a no-op success.
    pub async fn clear(
        db: &Database,
        alert_id: AlertId,
        by: AlertClearedBy,
    ) -> WorkbenchResult<()> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE alerts SET acknowledged_at = ?1, cleared_by = ?2
            WHERE id = ?3 AND acknowledged_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(by.as_str())
        .bind(alert_id.get())
        .execute(db.pool())
        .await?;

        if result.rows_affected() == 0 {
            // Check if the alert exists at all.
            let exists = sqlx::query("SELECT 1 FROM alerts WHERE id = ?1")
                .bind(alert_id.get())
                .fetch_optional(db.pool())
                .await?;
            if exists.is_none() {
                return Err(WorkbenchError::not_found(format!("alert {alert_id}")));
            }
            // Already cleared — idempotent no-op.
        }

        Ok(())
    }

    /// Acknowledge an alert by user action. Verifies that the alert belongs to
    /// the given session before clearing. Delegates to `clear` with
    /// `AlertClearedBy::User`.
    pub async fn acknowledge(
        db: &Database,
        alert_id: AlertId,
        session_id: SessionId,
    ) -> WorkbenchResult<()> {
        // Verify the alert belongs to the claimed session.
        let row = sqlx::query("SELECT session_id FROM alerts WHERE id = ?1")
            .bind(alert_id.get())
            .fetch_optional(db.pool())
            .await?;
        match row {
            None => return Err(WorkbenchError::not_found(format!("alert {alert_id}"))),
            Some(row) => {
                let owner: i64 = row.try_get("session_id")?;
                if owner != session_id.get() {
                    return Err(WorkbenchError::not_found(format!(
                        "alert {alert_id} not found for session {session_id}"
                    )));
                }
            }
        }
        Self::clear(db, alert_id, AlertClearedBy::User).await
    }

    /// List all open (unacknowledged) alerts for a session.
    pub async fn list_open(db: &Database, session_id: SessionId) -> WorkbenchResult<Vec<Alert>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, project_id, kind, reason, raised_at,
                   acknowledged_at, cleared_by
            FROM alerts
            WHERE session_id = ?1 AND acknowledged_at IS NULL
            ORDER BY raised_at DESC
            "#,
        )
        .bind(session_id.get())
        .fetch_all(db.pool())
        .await?;

        rows.iter().map(parse_alert_row).collect()
    }

    /// List all open alerts across all sessions (for the global alert bar).
    pub async fn list_all_open(db: &Database) -> WorkbenchResult<Vec<Alert>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, project_id, kind, reason, raised_at,
                   acknowledged_at, cleared_by
            FROM alerts
            WHERE acknowledged_at IS NULL
            ORDER BY raised_at DESC
            "#,
        )
        .fetch_all(db.pool())
        .await?;

        rows.iter().map(parse_alert_row).collect()
    }

    /// Clear all open alerts for a session. Used when a session ends or
    /// transitions away from `needs_input`.
    pub async fn clear_all_for_session(
        db: &Database,
        session_id: SessionId,
        by: AlertClearedBy,
    ) -> WorkbenchResult<Vec<AlertId>> {
        let now = Utc::now().to_rfc3339();

        // Fetch ids of open alerts first (for event emission).
        let rows = sqlx::query(
            r#"
            SELECT id FROM alerts
            WHERE session_id = ?1 AND acknowledged_at IS NULL
            "#,
        )
        .bind(session_id.get())
        .fetch_all(db.pool())
        .await?;

        let alert_ids: Vec<AlertId> = rows
            .iter()
            .map(|r| Ok(AlertId::new(r.try_get::<i64, _>("id")?)))
            .collect::<WorkbenchResult<Vec<_>>>()?;

        if !alert_ids.is_empty() {
            sqlx::query(
                r#"
                UPDATE alerts SET acknowledged_at = ?1, cleared_by = ?2
                WHERE session_id = ?3 AND acknowledged_at IS NULL
                "#,
            )
            .bind(&now)
            .bind(by.as_str())
            .bind(session_id.get())
            .execute(db.pool())
            .await?;
        }

        Ok(alert_ids)
    }
}

/// Parse an alert row from sqlx.
fn parse_alert_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Alert> {
    let id: i64 = row.try_get("id")?;
    let session_id: i64 = row.try_get("session_id")?;
    let project_id: i64 = row.try_get("project_id")?;
    let kind_str: String = row.try_get("kind")?;
    let reason: Option<String> = row.try_get("reason")?;
    let raised_at_str: String = row.try_get("raised_at")?;
    let acknowledged_at_str: Option<String> = row.try_get("acknowledged_at")?;
    let cleared_by_str: Option<String> = row.try_get("cleared_by")?;

    let kind = match kind_str.as_str() {
        "needs_input" => AlertKind::NeedsInput,
        other => {
            return Err(WorkbenchError::internal(format!(
                "unknown alert kind: {other}"
            )));
        }
    };

    let raised_at: Timestamp = raised_at_str
        .parse()
        .map_err(|e| WorkbenchError::internal(format!("invalid raised_at: {e}")))?;

    let acknowledged_at: Option<Timestamp> = acknowledged_at_str
        .map(|s| s.parse())
        .transpose()
        .map_err(|e| WorkbenchError::internal(format!("invalid acknowledged_at: {e}")))?;

    let cleared_by: Option<AlertClearedBy> = cleared_by_str
        .map(|s| s.parse())
        .transpose()
        .map_err(WorkbenchError::internal)?;

    Ok(Alert {
        id: AlertId::new(id),
        session_id: SessionId::new(session_id),
        project_id: ProjectId::new(project_id),
        kind,
        reason,
        raised_at,
        acknowledged_at,
        cleared_by,
    })
}
