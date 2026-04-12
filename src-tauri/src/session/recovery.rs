//! Crash-recovery pass.
//!
//! T025: `reconcile_and_reattach` — single merged pass. Ordering invariant:
//! this is ONE pass, not two. Splitting into "mark-stale-first,
//! reattach-second" was the ordering bug flagged in spec-panel round 1.
//!
//! For each `sessions` row where `status IN ('working','idle','needs_input')`
//! AND `ended_at IS NULL`:
//!
//!   * If pid is null OR the pid is not alive per `sysinfo`, mark the row
//!     `ended` with `error = 'workbench_restart'` in the same transaction.
//!
//!   * If the pid IS alive, create an **attached-mirror** LiveSessionHandle
//!     (no PTY ownership — the original master fd does not survive workbench
//!     restart in v1, so the handle is read-only regardless of ownership).
//!     Install it in `state.live_sessions`. Row status is preserved (may be
//!     idle/working/needs_input).
//!
//! T124 (folded into T025): also emit `session:spawned` events on
//! `state.event_bus` for each reattached session so the US6 frontend
//! hydration path sees them as "running, reattached."

use crate::error::WorkbenchResult;
use crate::model::SessionId;
use crate::state::{LiveSessionHandle, SessionEventEnvelope, WorkbenchState};
use chrono::Utc;
use sqlx::Row;
use sysinfo::{Pid, System};
use tracing::{info, warn};

/// Report of what the reconcile pass did. Returned so callers (T025b) can
/// assert both branches were exercised.
#[derive(Debug, Default, Clone)]
pub struct ReconcileReport {
    /// Session ids whose live pid survived the restart.
    pub reattached: Vec<SessionId>,
    /// Session ids that were marked `ended` because their pid was gone.
    pub ended: Vec<SessionId>,
}

/// Run the single-pass crash recovery. Called once from `run()` after DB open
/// and before Tauri's frontend is ready to call `session_list`.
pub async fn reconcile_and_reattach(state: &WorkbenchState) -> WorkbenchResult<ReconcileReport> {
    let mut report = ReconcileReport::default();
    let mut system = System::new();
    system.refresh_processes();

    let rows = sqlx::query(
        r#"
        SELECT id, pid
        FROM sessions
        WHERE status IN ('working','idle','needs_input')
          AND ended_at IS NULL
        "#,
    )
    .fetch_all(state.db.pool())
    .await?;

    for row in rows {
        let id: i64 = row.try_get("id")?;
        let pid: Option<i64> = row.try_get("pid")?;
        let session_id = SessionId::new(id);

        let alive = match pid {
            Some(p) if p > 0 => {
                // sysinfo's Pid type varies by platform; `Pid::from_u32` is
                // the portable constructor.
                let syspid = Pid::from_u32(p as u32);
                system.process(syspid).is_some()
            }
            _ => false,
        };

        if alive {
            // Reattach as an attached-mirror handle. Handle is a stub in
            // Phase 2 (T017); real LiveSessionHandle lands in T045. The
            // contract here is that the handle exists in state.live_sessions
            // so `session_list` can return `reattached_mirror = true`.
            let handle = LiveSessionHandle {};
            state
                .live_sessions
                .write()
                .await
                .insert(session_id, handle);
            report.reattached.push(session_id);

            // Broadcast session:spawned (T124 folded in). Ignored if no
            // subscribers yet — the event bridge task attaches later in run().
            let _ = state
                .event_bus
                .send(SessionEventEnvelope::Spawned { session_id });
            info!(%session_id, "reattached live session on workbench restart");
        } else {
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                r#"
                UPDATE sessions
                SET status = 'ended',
                    ended_at = ?1,
                    error_reason = 'workbench_restart',
                    pid = NULL
                WHERE id = ?2
                "#,
            )
            .bind(&now)
            .bind(id)
            .execute(state.db.pool())
            .await?;

            report.ended.push(session_id);
            warn!(
                %session_id,
                "marked session ended because its pid was gone on workbench restart"
            );
        }
    }

    Ok(report)
}
