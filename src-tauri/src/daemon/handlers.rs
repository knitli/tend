//! Daemon IPC request dispatch.
//!
//! T050-T052: Real implementations for `hello`, `register_session`,
//! `update_status`, `heartbeat`, and `end_session`.

use crate::model::{SessionId, SessionStatus, StatusSource};
use crate::project::ProjectService;
use crate::session::SessionService;
use crate::state::WorkbenchState;
use tend_protocol::{
    error as protocol_error, ErrorCode, Request, Response, SessionStatusWire, PROTOCOL_VERSION,
};
use tracing::{debug, info, warn};

/// Server version string embedded from Cargo.toml.
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dispatch a daemon IPC request to its handler.
pub async fn dispatch(req: Request, state: &WorkbenchState) -> Response {
    match req {
        Request::Hello {
            client,
            client_version,
            protocol_version,
        } => handle_hello(client, client_version, protocol_version).await,

        Request::RegisterSession {
            project_path,
            label,
            working_directory,
            command,
            pid,
            metadata,
        } => {
            handle_register_session(
                state,
                project_path,
                label,
                working_directory,
                command,
                pid,
                metadata,
            )
            .await
        }

        Request::UpdateStatus {
            session_id,
            status,
            reason,
            summary,
        } => handle_update_status(state, session_id, status, reason, summary).await,

        Request::Heartbeat { session_id } => handle_heartbeat(state, session_id).await,

        Request::EndSession {
            session_id,
            exit_code,
        } => handle_end_session(state, session_id, exit_code).await,
    }
}

/// T050: Hello → Welcome with version check.
async fn handle_hello(client: String, client_version: String, protocol_version: u32) -> Response {
    debug!(
        client = %client,
        client_version = %client_version,
        protocol_version,
        "daemon hello received"
    );

    if protocol_version != PROTOCOL_VERSION {
        return protocol_error(
            ErrorCode::ProtocolError,
            format!(
                "protocol version mismatch: client={protocol_version}, server={PROTOCOL_VERSION}"
            ),
        );
    }

    Response::Welcome {
        server_version: SERVER_VERSION.to_string(),
        protocol_version: PROTOCOL_VERSION,
    }
}

/// T051: RegisterSession — canonicalize project, create session with wrapper ownership.
async fn handle_register_session(
    state: &WorkbenchState,
    project_path: String,
    label: Option<String>,
    working_directory: Option<String>,
    command: Option<Vec<String>>,
    pid: i32,
    metadata: Option<serde_json::Value>,
) -> Response {
    // Ensure the project exists (creates it if unknown).
    let project = match ProjectService::ensure_exists(&state.db, &project_path).await {
        Ok(p) => p,
        Err(e) => return Response::from(e),
    };

    let working_dir = working_directory.unwrap_or_else(|| project_path.clone());

    // Build metadata JSON.
    let meta = metadata.unwrap_or_else(|| serde_json::json!({}));
    let meta_with_command = if let Some(cmd) = &command {
        let mut m = meta.as_object().cloned().unwrap_or_default();
        m.insert("command".to_string(), serde_json::json!(cmd));
        serde_json::Value::Object(m)
    } else {
        meta
    };

    // Create session row via SessionService.
    let session = match SessionService::create_from_ipc(
        &state.db,
        project.id,
        label.as_deref(),
        &working_dir,
        pid,
        &meta_with_command,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => return Response::from(e),
    };

    // Install an attached-mirror handle (no PTY — wrapper owns it).
    let handle = crate::session::live::LiveSessionHandle::attached_mirror(session.id);
    state.live_sessions.write().await.insert(session.id, handle);

    // Broadcast session:spawned event with full session record.
    let _ = state
        .event_bus
        .send(crate::state::SessionEventEnvelope::Spawned {
            session: session.clone(),
        });

    info!(
        session_id = %session.id,
        project_id = %project.id,
        pid,
        "registered wrapper session via daemon IPC"
    );

    Response::SessionRegistered {
        session_id: session.id.get(),
        project_id: project.id.get(),
    }
}

/// T052: UpdateStatus — validate status, update row, broadcast events.
///
/// Now emits alert:raised / alert:cleared events based on the status change
/// result from SessionService (T072 integration).
async fn handle_update_status(
    state: &WorkbenchState,
    session_id: i64,
    status: SessionStatusWire,
    reason: Option<String>,
    summary: Option<String>,
) -> Response {
    let sid = SessionId::new(session_id);

    let new_status = match status {
        SessionStatusWire::Working => SessionStatus::Working,
        SessionStatusWire::Idle => SessionStatus::Idle,
        SessionStatusWire::NeedsInput => SessionStatus::NeedsInput,
    };

    match SessionService::set_status(
        &state.db,
        sid,
        new_status,
        StatusSource::Ipc,
        reason.as_deref(),
    )
    .await
    {
        Ok(change) => {
            // Emit alert events based on the status change.
            if let Some(alert) = change.raised_alert {
                let _ = state
                    .event_bus
                    .send(crate::state::SessionEventEnvelope::AlertRaised { alert });
            }
            for alert_id in change.cleared_alert_ids {
                let _ = state
                    .event_bus
                    .send(crate::state::SessionEventEnvelope::AlertCleared {
                        alert_id,
                        by: crate::model::AlertClearedBy::SessionResumed,
                    });
            }

            // Push cooperative status update to the live session's monitor.
            if let Some(handle) = state.live_sessions.read().await.get(&sid) {
                let _ = handle
                    .send_ipc_status(crate::session::status::StatusUpdate {
                        status: new_status,
                        reason: reason.clone(),
                    })
                    .await;

                // T135/US4: If the cooperative update includes a summary string,
                // override the heuristic-derived activity summary.
                if let Some(ref s) = summary {
                    handle.activity.lock().await.override_with(s);
                }
            }

            Response::Ack
        }
        Err(e) => Response::from(e),
    }
}

/// T052: Heartbeat — update last_heartbeat_at.
async fn handle_heartbeat(state: &WorkbenchState, session_id: i64) -> Response {
    let sid = SessionId::new(session_id);
    let now = chrono::Utc::now().to_rfc3339();

    match sqlx::query("UPDATE sessions SET last_heartbeat_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(sid.get())
        .execute(state.db.pool())
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                return protocol_error(
                    ErrorCode::NotFound,
                    format!("session {session_id} not found"),
                );
            }
            Response::Ack
        }
        Err(e) => {
            warn!("heartbeat update failed: {e}");
            protocol_error(ErrorCode::Internal, format!("heartbeat failed: {e}"))
        }
    }
}

/// T052: EndSession — mark session as ended with exit code.
async fn handle_end_session(
    state: &WorkbenchState,
    session_id: i64,
    exit_code: Option<i32>,
) -> Response {
    let sid = SessionId::new(session_id);

    match SessionService::mark_ended(&state.db, sid, exit_code).await {
        Ok(_) => {
            // Remove from live sessions.
            state.live_sessions.write().await.remove(&sid);

            // Broadcast session:ended event.
            let _ = state
                .event_bus
                .send(crate::state::SessionEventEnvelope::Ended {
                    session_id: sid,
                    code: exit_code,
                });

            info!(%session_id, ?exit_code, "session ended via daemon IPC");
            Response::Ack
        }
        Err(e) => Response::from(e),
    }
}
