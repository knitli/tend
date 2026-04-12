//! Session service — list, spawn, and lifecycle management.
//!
//! T049/T051/T052: `SessionService` — the single service type for all session
//! lifecycle operations. Static methods operate on the shared `WorkbenchState`
//! and `Database`.

use crate::db::queries::require_found;
use crate::db::Database;
use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{
    Alert, AlertClearedBy, AlertId, AlertKind, Pid, ProjectId, Session, SessionId, SessionMetadata,
    SessionOwnership, SessionStatus, SessionSummary, StatusSource, Timestamp,
};
use crate::session::live::{spawn_live_session, LiveSessionHandle};
use crate::session::supervisor;
use crate::state::{SessionEventEnvelope, WorkbenchState};
use chrono::Utc;
use sqlx::Row;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::info;

/// Session service — stateless, operates on the shared DB + state.
pub struct SessionService;

impl SessionService {
    /// List sessions, optionally filtered by project and include_ended flag.
    ///
    /// The returned `SessionSummary` includes a LEFT-JOINed alert snapshot
    /// for status-alert consistency (round-1 #4).
    pub async fn list(
        state: &WorkbenchState,
        project_id: Option<ProjectId>,
        include_ended: bool,
    ) -> WorkbenchResult<Vec<SessionSummary>> {
        // M1: Use bind parameters instead of format-string interpolation.
        // M4: Use a correlated subquery for the alert JOIN to prevent duplicate rows.
        let base = r#"
            SELECT
                s.id, s.project_id, s.label, s.pid, s.status, s.status_source,
                s.ownership, s.started_at, s.ended_at, s.last_activity_at,
                s.last_heartbeat_at, s.metadata_json, s.working_directory,
                s.exit_code, s.error_reason,
                a.id AS alert_id, a.kind AS alert_kind, a.reason AS alert_reason,
                a.raised_at AS alert_raised_at, a.acknowledged_at AS alert_ack_at,
                a.cleared_by AS alert_cleared_by
            FROM sessions s
            LEFT JOIN alerts a ON a.id = (
                SELECT a2.id FROM alerts a2
                WHERE a2.session_id = s.id AND a2.acknowledged_at IS NULL
                ORDER BY a2.raised_at DESC LIMIT 1
            )
        "#;

        let rows = match (project_id, include_ended) {
            (Some(pid), false) => {
                let sql = format!(
                    "{base} WHERE s.project_id = ?1 AND s.status NOT IN ('ended','error') ORDER BY s.started_at DESC"
                );
                sqlx::query(&sql)
                    .bind(pid.get())
                    .fetch_all(state.db.pool())
                    .await?
            }
            (Some(pid), true) => {
                let sql = format!("{base} WHERE s.project_id = ?1 ORDER BY s.started_at DESC");
                sqlx::query(&sql)
                    .bind(pid.get())
                    .fetch_all(state.db.pool())
                    .await?
            }
            (None, false) => {
                let sql = format!(
                    "{base} WHERE s.status NOT IN ('ended','error') ORDER BY s.started_at DESC"
                );
                sqlx::query(&sql).fetch_all(state.db.pool()).await?
            }
            (None, true) => {
                let sql = format!("{base} ORDER BY s.started_at DESC");
                sqlx::query(&sql).fetch_all(state.db.pool()).await?
            }
        };

        let live_sessions = state.live_sessions.read().await;
        let mut summaries = Vec::with_capacity(rows.len());

        for row in rows {
            let session = parse_session_row(&row)?;
            let session_id = session.id;

            let alert = parse_alert_from_join(&row, session_id)?;

            let reattached_mirror = live_sessions
                .get(&session_id)
                .map(|h| h.is_mirror)
                .unwrap_or(false);

            summaries.push(SessionSummary {
                session,
                activity_summary: None, // US4
                alert,
                reattached_mirror,
            });
        }

        Ok(summaries)
    }

    /// Spawn a new workbench-owned session under a PTY.
    ///
    /// Creates the DB row, spawns the child via `Pty::spawn`, installs the
    /// `LiveSessionHandle`, starts the supervisor tasks, and returns
    /// `(Session, LiveSessionHandle)`.
    pub async fn spawn_local(
        state: &WorkbenchState,
        project_id: ProjectId,
        label: &str,
        cwd: &Path,
        command: &[String],
        env: &BTreeMap<String, String>,
    ) -> WorkbenchResult<(Session, LiveSessionHandle)> {
        // Validate working directory.
        if !cwd.is_dir() {
            return Err(WorkbenchError::new(
                ErrorCode::WorkingDirectoryInvalid,
                format!("working directory does not exist: {}", cwd.display()),
            ));
        }

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let metadata = SessionMetadata {
            command: Some(command.to_vec()),
            ..Default::default()
        };
        let metadata_json = serde_json::to_string(&metadata)?;
        let wd = cwd.to_string_lossy().to_string();

        // Insert the DB row first to get the session id.
        let result = sqlx::query(
            r#"
            INSERT INTO sessions (
                project_id, label, status, status_source, ownership,
                started_at, last_activity_at, metadata_json, working_directory
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(project_id.get())
        .bind(label)
        .bind(SessionStatus::Working.as_str())
        .bind(StatusSource::Heuristic.as_str())
        .bind(SessionOwnership::Workbench.as_str())
        .bind(&now_str)
        .bind(&now_str)
        .bind(&metadata_json)
        .bind(&wd)
        .execute(state.db.pool())
        .await?;

        let session_id = SessionId::new(result.last_insert_rowid());

        // Spawn the PTY with the real session id.
        // H2: If spawn or the subsequent pid UPDATE fails, mark the DB row
        // ended immediately so it doesn't remain orphaned as 'working'.
        let (actor, handle) = match spawn_live_session(session_id, command, cwd, env, 80, 24) {
            Ok(pair) => pair,
            Err(e) => {
                let _ = sqlx::query(
                    "UPDATE sessions SET status='ended', ended_at=?1, error_reason='spawn_failed' WHERE id=?2",
                )
                .bind(Utc::now().to_rfc3339())
                .bind(session_id.get())
                .execute(state.db.pool())
                .await;
                return Err(e);
            }
        };

        let pid = actor.pty.pid().map(|p| Pid(p as i32));

        // Update the pid in the DB now that we know it.
        if let Some(p) = pid {
            if let Err(e) = sqlx::query("UPDATE sessions SET pid = ?1 WHERE id = ?2")
                .bind(p.0 as i64)
                .bind(session_id.get())
                .execute(state.db.pool())
                .await
            {
                let _ = sqlx::query(
                    "UPDATE sessions SET status='ended', ended_at=?1, error_reason='pid_update_failed' WHERE id=?2",
                )
                .bind(Utc::now().to_rfc3339())
                .bind(session_id.get())
                .execute(state.db.pool())
                .await;
                return Err(e.into());
            }
        }

        // Install the handle BEFORE starting supervisor tasks so the reaper
        // cannot race ahead and find nothing in the map if the child exits fast
        // (H1: stale LiveSessionHandle race fix).
        state
            .live_sessions
            .write()
            .await
            .insert(session_id, handle.clone());

        // Build the session struct before starting tasks so we can emit it.
        let session = Session {
            id: session_id,
            project_id,
            label: label.to_string(),
            pid,
            status: SessionStatus::Working,
            status_source: StatusSource::Heuristic,
            ownership: SessionOwnership::Workbench,
            started_at: now,
            ended_at: None,
            last_activity_at: now,
            last_heartbeat_at: None,
            metadata,
            working_directory: cwd.to_path_buf(),
            exit_code: None,
            error_reason: None,
        };

        // Start supervisor tasks.
        let _task_handles = supervisor::spawn_session_tasks(actor, state);

        // Emit session:spawned with the full session record.
        let _ = state.event_bus.send(SessionEventEnvelope::Spawned {
            session: session.clone(),
        });

        info!(%session_id, %project_id, ?pid, "session spawned locally (workbench-owned)");
        Ok((session, handle))
    }

    /// Create a session row from a daemon IPC `register_session` request.
    /// The session is wrapper-owned (read-only from the workbench).
    pub async fn create_from_ipc(
        db: &Database,
        project_id: ProjectId,
        label: Option<&str>,
        working_directory: &str,
        pid: i32,
        metadata: &serde_json::Value,
    ) -> WorkbenchResult<Session> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let label = label.unwrap_or("cli-session");
        let metadata_json = serde_json::to_string(metadata)?;

        let result = sqlx::query(
            r#"
            INSERT INTO sessions (
                project_id, label, pid, status, status_source, ownership,
                started_at, last_activity_at, metadata_json, working_directory
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(project_id.get())
        .bind(label)
        .bind(pid as i64)
        .bind(SessionStatus::Working.as_str())
        .bind(StatusSource::Ipc.as_str())
        .bind(SessionOwnership::Wrapper.as_str())
        .bind(&now_str)
        .bind(&now_str)
        .bind(&metadata_json)
        .bind(working_directory)
        .execute(db.pool())
        .await?;

        let session_id = SessionId::new(result.last_insert_rowid());

        let parsed_metadata: SessionMetadata =
            serde_json::from_value(metadata.clone()).unwrap_or_default();

        info!(%session_id, %project_id, pid, "session registered from IPC (wrapper-owned)");

        Ok(Session {
            id: session_id,
            project_id,
            label: label.to_string(),
            pid: Some(Pid(pid)),
            status: SessionStatus::Working,
            status_source: StatusSource::Ipc,
            ownership: SessionOwnership::Wrapper,
            started_at: now,
            ended_at: None,
            last_activity_at: now,
            last_heartbeat_at: None,
            metadata: parsed_metadata,
            working_directory: PathBuf::from(working_directory),
            exit_code: None,
            error_reason: None,
        })
    }

    /// Update session status and source.
    ///
    /// When the new status is `NeedsInput`, an alert row is inserted (H6).
    /// When transitioning *away* from `NeedsInput`, open alerts are cleared (H7).
    pub async fn set_status(
        db: &Database,
        session_id: SessionId,
        status: SessionStatus,
        source: StatusSource,
        reason: Option<&str>,
    ) -> WorkbenchResult<()> {
        // Fetch old status and project_id so we can manage alerts.
        let row = sqlx::query("SELECT status, project_id FROM sessions WHERE id = ?1")
            .bind(session_id.get())
            .fetch_optional(db.pool())
            .await?;

        let row = require_found(row, format!("session {session_id}"))?;
        let old_status_str: String = row.try_get("status")?;
        let project_id: i64 = row.try_get("project_id")?;
        let old_status: SessionStatus = old_status_str.parse().map_err(WorkbenchError::internal)?;

        let result = sqlx::query(
            r#"
            UPDATE sessions
            SET status = ?1,
                status_source = ?2
            WHERE id = ?3
            "#,
        )
        .bind(status.as_str())
        .bind(source.as_str())
        .bind(session_id.get())
        .execute(db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("session {session_id}")));
        }

        // H7: Clear open alerts when transitioning away from NeedsInput.
        if old_status == SessionStatus::NeedsInput && status != SessionStatus::NeedsInput {
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                r#"
                UPDATE alerts SET acknowledged_at = ?1, cleared_by = 'session_resumed'
                WHERE session_id = ?2 AND acknowledged_at IS NULL
                "#,
            )
            .bind(&now)
            .bind(session_id.get())
            .execute(db.pool())
            .await?;
        }

        // H6: Create an alert row when transitioning to NeedsInput.
        if status == SessionStatus::NeedsInput {
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                r#"
                INSERT INTO alerts (session_id, project_id, kind, reason, raised_at)
                VALUES (?1, ?2, 'needs_input', ?3, ?4)
                "#,
            )
            .bind(session_id.get())
            .bind(project_id)
            .bind(reason)
            .bind(&now)
            .execute(db.pool())
            .await?;
        }

        Ok(())
    }

    /// Mark a session as ended with optional exit code.
    ///
    /// H5: exit 0 (or None) → status 'ended'; exit != 0 → status 'error'.
    /// H4: Idempotent — if the session is already ended/error the call is a
    /// no-op success, avoiding the double-mark race between `handle_end_session`
    /// and the reaper.
    pub async fn mark_ended(
        db: &Database,
        session_id: SessionId,
        exit_code: Option<i32>,
    ) -> WorkbenchResult<()> {
        let now = Utc::now().to_rfc3339();

        // H5: Compute status based on exit code per data-model state diagram.
        let status = match exit_code {
            Some(0) | None => "ended",
            Some(_) => "error",
        };

        // H4: Guard with `status NOT IN ('ended','error')` so a second call
        // is a harmless no-op instead of a NOT_FOUND error.
        let result = sqlx::query(
            r#"
            UPDATE sessions
            SET status = ?1,
                ended_at = ?2,
                exit_code = ?3,
                pid = NULL
            WHERE id = ?4 AND status NOT IN ('ended', 'error')
            "#,
        )
        .bind(status)
        .bind(&now)
        .bind(exit_code.map(|c| c as i64))
        .bind(session_id.get())
        .execute(db.pool())
        .await?;

        if result.rows_affected() == 0 {
            // Could be already ended (idempotent) or genuinely missing.
            // Check existence to distinguish.
            let exists = sqlx::query("SELECT 1 FROM sessions WHERE id = ?1")
                .bind(session_id.get())
                .fetch_optional(db.pool())
                .await?;
            if exists.is_none() {
                return Err(WorkbenchError::not_found(format!("session {session_id}")));
            }
            // Already ended — treat as success.
            info!(%session_id, "mark_ended: already ended, idempotent no-op");
            return Ok(());
        }

        // H7: Clear any open alerts for the ended session.
        sqlx::query(
            r#"
            UPDATE alerts SET acknowledged_at = ?1, cleared_by = 'session_ended'
            WHERE session_id = ?2 AND acknowledged_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(session_id.get())
        .execute(db.pool())
        .await?;

        info!(%session_id, ?exit_code, %status, "session marked ended");
        Ok(())
    }

    /// Touch the session's last-activity timestamp with a monotonic clamp:
    /// `last_activity_at = MAX(last_activity_at, candidate)`.
    ///
    /// This ensures a backward timestamp never rewinds `last_activity_at`
    /// (data-model.md invariant #8).
    pub async fn touch_activity(
        db: &Database,
        session_id: SessionId,
        candidate: Timestamp,
    ) -> WorkbenchResult<()> {
        let candidate_str = candidate.to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE sessions
            SET last_activity_at = MAX(last_activity_at, ?1)
            WHERE id = ?2
            "#,
        )
        .bind(&candidate_str)
        .bind(session_id.get())
        .execute(db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(WorkbenchError::not_found(format!("session {session_id}")));
        }

        Ok(())
    }

    /// Verify a session is workbench-owned. Returns `SESSION_READ_ONLY` for
    /// wrapper-owned sessions. Used by `session_send_input`,
    /// `session_resize`, and `session_end` Tauri commands.
    pub async fn require_workbench_owned(
        db: &Database,
        session_id: SessionId,
    ) -> WorkbenchResult<()> {
        let row = sqlx::query("SELECT ownership FROM sessions WHERE id = ?1")
            .bind(session_id.get())
            .fetch_optional(db.pool())
            .await?;

        let row = require_found(row, format!("session {session_id}"))?;
        let ownership: String = row.try_get("ownership")?;

        if ownership != SessionOwnership::Workbench.as_str() {
            return Err(WorkbenchError::session_read_only(session_id.get()));
        }

        Ok(())
    }
}

// ---- Row parsing helpers ----

/// Parse a `Session` from a sqlx row (works for both direct queries and JOINs).
pub(crate) fn parse_session_row(row: &sqlx::sqlite::SqliteRow) -> WorkbenchResult<Session> {
    let id: i64 = row.try_get("id")?;
    let project_id: i64 = row.try_get("project_id")?;
    let label: String = row.try_get("label")?;
    let pid: Option<i64> = row.try_get("pid")?;
    let status_str: String = row.try_get("status")?;
    let status_source_str: String = row.try_get("status_source")?;
    let ownership_str: String = row.try_get("ownership")?;
    let started_at_str: String = row.try_get("started_at")?;
    let ended_at_str: Option<String> = row.try_get("ended_at")?;
    let last_activity_at_str: String = row.try_get("last_activity_at")?;
    let last_heartbeat_at_str: Option<String> = row.try_get("last_heartbeat_at")?;
    let metadata_json: String = row.try_get("metadata_json")?;
    let working_directory_str: String = row.try_get("working_directory")?;
    let exit_code: Option<i64> = row.try_get("exit_code")?;
    let error_reason: Option<String> = row.try_get("error_reason")?;

    let status: SessionStatus = status_str.parse().map_err(WorkbenchError::internal)?;
    let status_source: StatusSource = status_source_str
        .parse()
        .map_err(WorkbenchError::internal)?;
    let ownership: SessionOwnership = ownership_str.parse().map_err(WorkbenchError::internal)?;

    Ok(Session {
        id: SessionId::new(id),
        project_id: ProjectId::new(project_id),
        label,
        pid: pid.map(|p| Pid(p as i32)),
        status,
        status_source,
        ownership,
        started_at: parse_timestamp(&started_at_str)?,
        ended_at: ended_at_str.as_deref().map(parse_timestamp).transpose()?,
        last_activity_at: parse_timestamp(&last_activity_at_str)?,
        last_heartbeat_at: last_heartbeat_at_str
            .as_deref()
            .map(parse_timestamp)
            .transpose()?,
        metadata: serde_json::from_str(&metadata_json).unwrap_or_default(),
        working_directory: PathBuf::from(working_directory_str),
        exit_code: exit_code.map(|c| c as i32),
        error_reason,
    })
}

/// Parse an optional `Alert` from the LEFT JOIN columns in a session list query.
fn parse_alert_from_join(
    row: &sqlx::sqlite::SqliteRow,
    session_id: SessionId,
) -> WorkbenchResult<Option<Alert>> {
    let alert_id: Option<i64> = row.try_get("alert_id")?;
    let alert_id = match alert_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let kind_str: String = row.try_get("alert_kind")?;
    let reason: Option<String> = row.try_get("alert_reason")?;
    let raised_at_str: String = row.try_get("alert_raised_at")?;
    let ack_at_str: Option<String> = row.try_get("alert_ack_at")?;
    let cleared_by_str: Option<String> = row.try_get("alert_cleared_by")?;
    let project_id: i64 = row.try_get("project_id")?;

    let kind = match kind_str.as_str() {
        "needs_input" => AlertKind::NeedsInput,
        other => {
            return Err(WorkbenchError::internal(format!(
                "unknown alert kind: {other}"
            )))
        }
    };

    let cleared_by = cleared_by_str
        .as_deref()
        .map(|s| {
            s.parse::<AlertClearedBy>()
                .map_err(WorkbenchError::internal)
        })
        .transpose()?;

    Ok(Some(Alert {
        id: AlertId::new(alert_id),
        session_id,
        project_id: ProjectId::new(project_id),
        kind,
        reason,
        raised_at: parse_timestamp(&raised_at_str)?,
        acknowledged_at: ack_at_str.as_deref().map(parse_timestamp).transpose()?,
        cleared_by,
    }))
}

/// Parse an RFC-3339 timestamp string into a `chrono::DateTime<Utc>`.
fn parse_timestamp(s: &str) -> WorkbenchResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| WorkbenchError::internal(format!("invalid timestamp '{s}': {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T048 inline unit-test: assert a backward timestamp does NOT rewind
    /// `last_activity_at` (data-model invariant #8 monotonic clamp).
    #[tokio::test]
    async fn test_touch_activity_monotonic_clamp() {
        let state = crate::test_state().await.expect("test state");

        // Insert a project first (sessions have FK to projects).
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO projects (canonical_path, display_name, added_at)
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind("/tmp/test-project")
        .bind("test-project")
        .bind(&now_str)
        .execute(state.db.pool())
        .await
        .expect("insert project");

        // Insert a session with a known last_activity_at.
        let t1 = chrono::DateTime::parse_from_rfc3339("2026-01-15T10:00:00+00:00")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let t1_str = t1.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO sessions (
                project_id, label, status, status_source, ownership,
                started_at, last_activity_at, metadata_json, working_directory
            ) VALUES (1, 'test', 'working', 'heuristic', 'workbench', ?1, ?1, '{}', '/tmp')
            "#,
        )
        .bind(&t1_str)
        .execute(state.db.pool())
        .await
        .expect("insert session");

        let sid = SessionId::new(1);

        // Touch with a LATER timestamp — should advance.
        let t2 = chrono::DateTime::parse_from_rfc3339("2026-01-15T10:05:00+00:00")
            .unwrap()
            .with_timezone(&chrono::Utc);
        SessionService::touch_activity(&state.db, sid, t2)
            .await
            .expect("touch forward");

        let row = sqlx::query("SELECT last_activity_at FROM sessions WHERE id = 1")
            .fetch_one(state.db.pool())
            .await
            .expect("fetch");
        let stored: String = row.try_get("last_activity_at").expect("get");
        let stored_ts = parse_timestamp(&stored).expect("parse");
        assert_eq!(stored_ts, t2, "forward touch should advance");

        // Touch with an EARLIER timestamp — should NOT rewind.
        let t0 = chrono::DateTime::parse_from_rfc3339("2026-01-15T09:00:00+00:00")
            .unwrap()
            .with_timezone(&chrono::Utc);
        SessionService::touch_activity(&state.db, sid, t0)
            .await
            .expect("touch backward");

        let row = sqlx::query("SELECT last_activity_at FROM sessions WHERE id = 1")
            .fetch_one(state.db.pool())
            .await
            .expect("fetch");
        let stored: String = row.try_get("last_activity_at").expect("get");
        let stored_ts = parse_timestamp(&stored).expect("parse");
        assert_eq!(
            stored_ts, t2,
            "backward touch must NOT rewind last_activity_at"
        );
    }
}
