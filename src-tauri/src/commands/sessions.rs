//! Tauri command handlers for session management.
//!
//! T049: `session_list` and `session_spawn` (GUI-initiated).
//! T092: `session_activate` (US3).
//! T093: `session_send_input`, `session_resize`, `session_end` (US3).

use crate::companion::CompanionService;
use crate::error::{ErrorCode, WorkbenchError};
use crate::model::{ProjectId, SessionId};
use crate::session::live::KillSignal;
use crate::session::SessionService;
use crate::state::WorkbenchState;
use serde::Deserialize;
use std::collections::BTreeMap;
use tauri::State;

/// Args for `session_list`.
#[derive(Deserialize)]
pub struct SessionListArgs {
    /// Filter by project id.
    pub project_id: Option<i64>,
    /// Include ended sessions.
    #[serde(default)]
    pub include_ended: bool,
}

/// List sessions with optional filters.
#[tauri::command]
pub async fn session_list(
    state: State<'_, WorkbenchState>,
    args: SessionListArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project_id = args.project_id.map(ProjectId::new);
    let sessions = SessionService::list(&state, project_id, args.include_ended).await?;
    Ok(serde_json::json!({ "sessions": sessions }))
}

/// Args for `session_spawn`.
#[derive(Deserialize)]
pub struct SessionSpawnArgs {
    /// Project id to attach the session to.
    pub project_id: i64,
    /// Command to spawn.
    pub command: Vec<String>,
    /// Optional label.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional working directory (defaults to project root).
    #[serde(default)]
    pub working_directory: Option<String>,
    /// Optional extra environment variables.
    #[serde(default)]
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// Spawn a new session (workbench-owned).
#[tauri::command]
pub async fn session_spawn(
    state: State<'_, WorkbenchState>,
    args: SessionSpawnArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project_id = ProjectId::new(args.project_id);

    // Verify project exists and is not archived.
    let project = crate::project::ProjectService::get_by_id(&state.db, project_id).await?;
    if project.archived_at.is_some() {
        return Err(WorkbenchError::new(
            ErrorCode::ProjectArchived,
            format!("project {} is archived", project_id),
        ));
    }

    // Resolve working directory.
    let cwd = match &args.working_directory {
        Some(dir) => {
            let p = std::path::Path::new(dir);
            if !p.is_dir() {
                return Err(WorkbenchError::new(
                    ErrorCode::WorkingDirectoryInvalid,
                    format!("working directory does not exist: {dir}"),
                ));
            }
            p.to_path_buf()
        }
        None => project.canonical_path.clone(),
    };

    // Convert env HashMap to BTreeMap for Pty::spawn.
    let env: BTreeMap<String, String> = args.env.unwrap_or_default().into_iter().collect();

    let label = args.label.unwrap_or_else(|| "session".to_string());

    // spawn_local inserts the handle into live_sessions before starting
    // supervisor tasks, so no separate insert is needed here.
    let (session, _handle) =
        SessionService::spawn_local(&state, project_id, &label, &cwd, &args.command, &env).await?;

    Ok(serde_json::json!({ "session": session }))
}

/// Args for `session_activate`.
#[derive(Deserialize)]
pub struct SessionActivateArgs {
    /// Session id to activate.
    pub session_id: i64,
}

/// Activate a session — brings it to the foreground and ensures a companion
/// terminal exists.
#[tauri::command]
pub async fn session_activate(
    state: State<'_, WorkbenchState>,
    args: SessionActivateArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);

    // H5 fix: direct lookup instead of O(n) list scan.
    let summary = SessionService::get_by_id(&state, session_id).await?;

    if summary.session.status == crate::model::SessionStatus::Ended
        || summary.session.status == crate::model::SessionStatus::Error
    {
        return Err(WorkbenchError::new(
            ErrorCode::SessionEnded,
            format!("session {session_id} has ended"),
        ));
    }

    // Ensure companion terminal exists.
    let companion = CompanionService::ensure(&state, session_id).await?;

    Ok(serde_json::json!({
        "session": summary.session,
        "companion": companion,
    }))
}

/// Args for `session_send_input`.
#[derive(Deserialize)]
pub struct SessionSendInputArgs {
    /// Session id.
    pub session_id: i64,
    /// Bytes to write (UTF-8 string).
    pub bytes: String,
}

/// Write bytes to a workbench-owned session's PTY stdin.
#[tauri::command]
pub async fn session_send_input(
    state: State<'_, WorkbenchState>,
    args: SessionSendInputArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);

    // Ownership check — rejects wrapper-owned sessions with SESSION_READ_ONLY.
    SessionService::require_workbench_owned(&state.db, session_id).await?;

    let live = state.live_sessions.read().await;
    let handle = live.get(&session_id).ok_or_else(|| {
        WorkbenchError::new(
            ErrorCode::SessionEnded,
            format!("session {session_id} not live"),
        )
    })?;

    // H6: The contract says "base64 or plain UTF-8". The frontend always sends
    // plain UTF-8 from xterm.js onData(). We treat the field as plain UTF-8;
    // base64 encoding is reserved for a future binary-input extension.
    handle.write(args.bytes.as_bytes())?;
    Ok(serde_json::json!({}))
}

/// Args for `session_resize`.
#[derive(Deserialize)]
pub struct SessionResizeArgs {
    /// Session id.
    pub session_id: i64,
    /// New column count.
    pub cols: u16,
    /// New row count.
    pub rows: u16,
}

/// Resize a workbench-owned session's PTY.
#[tauri::command]
pub async fn session_resize(
    state: State<'_, WorkbenchState>,
    args: SessionResizeArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);

    SessionService::require_workbench_owned(&state.db, session_id).await?;

    let live = state.live_sessions.read().await;
    let handle = live.get(&session_id).ok_or_else(|| {
        WorkbenchError::new(
            ErrorCode::SessionEnded,
            format!("session {session_id} not live"),
        )
    })?;

    handle.resize(args.cols, args.rows)?;
    Ok(serde_json::json!({}))
}

/// Args for `session_end`.
#[derive(Deserialize)]
pub struct SessionEndArgs {
    /// Session id.
    pub session_id: i64,
    /// Signal to send (default TERM).
    #[serde(default)]
    pub signal: Option<String>,
}

/// End a workbench-owned session by sending a signal to the child process.
#[tauri::command]
pub async fn session_end(
    state: State<'_, WorkbenchState>,
    args: SessionEndArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);

    SessionService::require_workbench_owned(&state.db, session_id).await?;

    let live = state.live_sessions.read().await;
    let handle = live.get(&session_id).ok_or_else(|| {
        WorkbenchError::new(
            ErrorCode::SessionEnded,
            format!("session {session_id} not live"),
        )
    })?;

    // H1 fix: map signal string to KillSignal enum.
    let signal = match args.signal.as_deref() {
        Some("KILL") => KillSignal::Kill,
        _ => KillSignal::Term, // default
    };
    handle.end(signal)?;

    // H5 fix: direct lookup. Note: the reaper marks the session ended
    // asynchronously, so the returned status reflects state at signal time.
    drop(live);
    let summary = SessionService::get_by_id(&state, session_id).await?;

    Ok(serde_json::json!({ "session": summary.session }))
}
