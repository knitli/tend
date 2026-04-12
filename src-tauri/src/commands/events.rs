//! Event bridge: state.event_bus → AppHandle::emit.
//!
//! T053: a tokio task that subscribes to the broadcast bus and forwards each
//! envelope as a Tauri event so the Svelte frontend can receive typed payloads.

use crate::db::Database;
use crate::model::Session;
use crate::state::{SessionEventEnvelope, WorkbenchState};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{debug, warn};

/// Spawn the event bridge task. Called once from `run()` after the Tauri app
/// starts. The task runs for the lifetime of the app.
pub fn spawn_event_bridge(app: AppHandle, state: &WorkbenchState) {
    let mut rx = state.event_bus.subscribe();
    let db = state.db.clone();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(envelope) => emit_envelope(&app, &db, envelope).await,
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

async fn emit_envelope(app: &AppHandle, db: &Database, envelope: SessionEventEnvelope) {
    match envelope {
        SessionEventEnvelope::Spawned { session } => {
            let _ = app.emit("session:spawned", SessionSpawnedPayload { session });
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
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let _ = app.emit(
                "session:event",
                SessionEventPayload {
                    session_id: session_id.get(),
                    bytes: encoded,
                },
            );
        }
        SessionEventEnvelope::AlertRaised { ref alert } => {
            let _ = app.emit("alert:raised", &alert);
            // T077: dispatch OS notification for raised alerts.
            crate::notifications::dispatch::dispatch_alert(db, app, alert).await;
        }
        SessionEventEnvelope::AlertCleared { alert_id, by } => {
            let _ = app.emit(
                "alert:cleared",
                AlertClearedPayload {
                    alert_id: alert_id.get(),
                    by: by.as_str().to_owned(),
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
        SessionEventEnvelope::CompanionSpawned {
            session_id,
            companion,
        } => {
            // H2 fix: emit full CompanionTerminal record, not just id+pid.
            let _ = app.emit(
                "companion:spawned",
                CompanionSpawnedPayload {
                    session_id: session_id.get(),
                    companion,
                },
            );
        }
        SessionEventEnvelope::CompanionOutput { session_id, bytes } => {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let _ = app.emit(
                "companion:output",
                CompanionOutputPayload {
                    session_id: session_id.get(),
                    bytes: encoded,
                },
            );
        }
    }
}

#[derive(Clone, Serialize)]
struct SessionSpawnedPayload {
    session: Session,
}

#[derive(Clone, Serialize)]
struct SessionEndedPayload {
    session_id: i64,
    code: Option<i32>,
}

#[derive(Clone, Serialize)]
struct SessionEventPayload {
    session_id: i64,
    /// Base64-encoded PTY output bytes.
    bytes: String,
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

#[derive(Clone, Serialize)]
struct CompanionSpawnedPayload {
    session_id: i64,
    /// Full CompanionTerminal record (H2 fix).
    companion: crate::model::CompanionTerminal,
}

#[derive(Clone, Serialize)]
struct CompanionOutputPayload {
    session_id: i64,
    bytes: String,
}
