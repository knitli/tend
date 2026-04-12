//! Session reaper — listens for Ended events and updates the DB.
//!
//! When a child process exits, the supervisor emits `SessionEventEnvelope::Ended`.
//! This task subscribes to that bus and calls `SessionService::mark_ended` to
//! persist the status transition and remove the live handle.

use crate::companion::CompanionService;
use crate::session::SessionService;
use crate::state::{SessionEventEnvelope, WorkbenchState};
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
                    warn!("session reaper lagged by {n} messages");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!("event bus closed, reaper stopping");
                    break;
                }
            }
        }
    });
}
