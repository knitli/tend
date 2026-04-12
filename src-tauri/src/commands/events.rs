//! Event bridge: state.event_bus → AppHandle::emit.
//!
//! T053: a tokio task that subscribes to the broadcast bus and forwards each
//! envelope as a Tauri event so the Svelte frontend can receive typed payloads.

use crate::state::{SessionEventEnvelope, WorkbenchState};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{debug, warn};

/// Spawn the event bridge task. Called once from `run()` after the Tauri app
/// starts. The task runs for the lifetime of the app.
pub fn spawn_event_bridge(app: AppHandle, state: &WorkbenchState) {
    let mut rx = state.event_bus.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(envelope) => emit_envelope(&app, envelope),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("event bridge lagged by {n} messages");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!("event bus closed, stopping bridge");
                    break;
                }
            }
        }
    });
}

fn emit_envelope(app: &AppHandle, envelope: SessionEventEnvelope) {
    match envelope {
        SessionEventEnvelope::Spawned { session_id } => {
            let _ = app.emit(
                "session:spawned",
                SessionSpawnedPayload {
                    session_id: session_id.get(),
                },
            );
        }
        SessionEventEnvelope::Ended { session_id, code } => {
            let _ = app.emit(
                "session:ended",
                SessionEndedPayload {
                    session_id: session_id.get(),
                    code,
                },
            );
        }
        SessionEventEnvelope::Output { session_id, bytes } => {
            let _ = app.emit(
                "session:event",
                SessionEventPayload {
                    session_id: session_id.get(),
                    bytes,
                },
            );
        }
        SessionEventEnvelope::AlertRaised { alert } => {
            let _ = app.emit("alert:raised", &alert);
        }
        SessionEventEnvelope::AlertCleared { alert_id, by } => {
            let _ = app.emit(
                "alert:cleared",
                AlertClearedPayload {
                    alert_id: alert_id.get(),
                    by: format!("{by:?}"),
                },
            );
        }
        SessionEventEnvelope::ProjectPathMissing { project_id } => {
            let _ = app.emit(
                "project:path_missing",
                ProjectPathPayload {
                    project_id: project_id.get(),
                },
            );
        }
        SessionEventEnvelope::ProjectPathRestored { project_id } => {
            let _ = app.emit(
                "project:path_restored",
                ProjectPathPayload {
                    project_id: project_id.get(),
                },
            );
        }
    }
}

#[derive(Clone, Serialize)]
struct SessionSpawnedPayload {
    session_id: i64,
}

#[derive(Clone, Serialize)]
struct SessionEndedPayload {
    session_id: i64,
    code: Option<i32>,
}

#[derive(Clone, Serialize)]
struct SessionEventPayload {
    session_id: i64,
    bytes: Vec<u8>,
}

#[derive(Clone, Serialize)]
struct AlertClearedPayload {
    alert_id: i64,
    by: String,
}

#[derive(Clone, Serialize)]
struct ProjectPathPayload {
    project_id: i64,
}
