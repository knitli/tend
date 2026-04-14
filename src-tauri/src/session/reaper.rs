//! Session reaper — listens for Ended events and updates the DB.
//!
//! When a child process exits, the supervisor emits `SessionEventEnvelope::Ended`.
//! This task subscribes to that bus and calls `SessionService::mark_ended` to
//! persist the status transition and remove the live handle.

use crate::companion::CompanionService;
use crate::model::SessionId;
use crate::session::SessionService;
use crate::state::{SessionEventEnvelope, WorkbenchState};
use sysinfo::{Pid, System};
use tracing::{debug, info, warn};

/// Spawn the reaper task. It runs for the lifetime of the app and handles
/// session cleanup when child processes exit.
pub fn spawn_reaper(state: WorkbenchState) {
    let mut rx = state.event_bus.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(SessionEventEnvelope::Ended { session_id, code }) => {
                    // Update the DB row.
                    if let Err(e) = SessionService::mark_ended(&state.db, session_id, code).await {
                        warn!(%session_id, %e, "reaper: failed to mark session ended");
                    } else {
                        info!(%session_id, ?code, "reaper: session marked ended in DB");
                    }

                    // Remove from live sessions.
                    state.live_sessions.write().await.remove(&session_id);

                    // C2 fix: kill and clean up companion terminal for this session.
                    CompanionService::cleanup_for_session(&state, session_id).await;
                }
                Ok(_) => {
                    // Ignore non-Ended events.
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("session reaper lagged by {n} messages, running reconciliation scan");
                    reconcile_live_sessions(&state).await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!("event bus closed, reaper stopping");
                    break;
                }
            }
        }
    });
}

/// Scan all live session handles and reap any whose process is no longer alive.
///
/// Called after the reaper detects a broadcast lag to recover Ended events that
/// were dropped from the channel.
async fn reconcile_live_sessions(state: &WorkbenchState) {
    // Collect session IDs that are currently live.
    let session_ids: Vec<SessionId> = {
        let live = state.live_sessions.read().await;
        live.keys().copied().collect()
    };

    if session_ids.is_empty() {
        return;
    }

    // Look up each session's PID from the DB and check liveness.
    let mut system = System::new();
    system.refresh_processes();

    let mut reaped: u32 = 0;
    for session_id in session_ids {
        let pid: Option<i64> = sqlx::query_scalar("SELECT pid FROM sessions WHERE id = ?1")
            .bind(session_id.get())
            .fetch_optional(state.db.pool())
            .await
            .ok()
            .flatten();

        let alive = match pid {
            Some(p) if p > 0 => {
                let syspid = Pid::from_u32(p as u32);
                system.process(syspid).is_some()
            }
            _ => false,
        };

        if !alive {
            info!(%session_id, ?pid, "reconciliation: process dead, reaping session");
            if let Err(e) = SessionService::mark_ended(&state.db, session_id, None).await {
                warn!(%session_id, %e, "reconciliation: failed to mark session ended");
            }
            state.live_sessions.write().await.remove(&session_id);
            CompanionService::cleanup_for_session(state, session_id).await;
            reaped += 1;
        }
    }

    info!(reaped, "reconciliation scan complete");
}
