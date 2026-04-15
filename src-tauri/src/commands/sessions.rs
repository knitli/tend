//! Tauri command handlers for session management.
//!
//! T049: `session_list` and `session_spawn` (GUI-initiated).
//! T092: `session_activate` (US3).
//! T093: `session_send_input`, `session_resize`, `session_end` (US3).

use crate::companion::CompanionService;
use crate::error::{ErrorCode, WorkbenchError};
use crate::model::{ProjectId, SessionId};
use crate::session::SessionService;
use crate::session::live::KillSignal;
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
    /// Initial PTY column count. Frontend passes the target pane's measured
    /// width so the child process is spawned at the right size — avoids the
    /// "initial banner rendered at 80x24 then resized" flash.
    #[serde(default)]
    pub cols: Option<u16>,
    /// Initial PTY row count. See [`cols`](Self::cols).
    #[serde(default)]
    pub rows: Option<u16>,
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

    // Build the spawn environment by layering:
    //   1. Captured user shell env (PATH, HOME, NVM/PYENV, etc. from ~/.zshrc).
    //   2. Per-spawn overrides from args.env (explicit beats inherited).
    let mut env: BTreeMap<String, String> = state
        .shell_env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(overrides) = args.env {
        for (k, v) in overrides {
            env.insert(k, v);
        }
    }

    let label = args.label.unwrap_or_else(|| "session".to_string());

    // Initial PTY size: prefer client-provided measurements when present,
    // but default conservatively (80x24). A default that overshoots the
    // real pane causes TUI alt-screen content to render past the visible
    // xterm buffer; it's safer to undershoot and let xterm's onResize
    // upscale on mount.
    let cols = args.cols.filter(|c| *c >= 20).unwrap_or(80);
    let rows = args.rows.filter(|r| *r >= 5).unwrap_or(24);

    // spawn_local inserts the handle into live_sessions before starting
    // supervisor tasks, so no separate insert is needed here.
    let (session, _handle) = SessionService::spawn_local(
        &state,
        project_id,
        &label,
        &cwd,
        &args.command,
        &env,
        cols,
        rows,
    )
    .await?;

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

/// Args for `session_set_focus`.
#[derive(Deserialize)]
pub struct SessionSetFocusArgs {
    /// Session id to focus, or null/omitted for "no focus".
    #[serde(default)]
    pub session_id: Option<i64>,
}

/// Set the currently-focused session. The event bridge forwards raw PTY
/// output (`session:event` and `companion:output`) only for the focused
/// session — non-focused sessions' bytes are still captured in the replay
/// buffer but not serialized across Tauri IPC, saving significant CPU when
/// multiple TUIs are running.
///
/// Pass `null`/omit `session_id` for overview/empty states.
#[tauri::command]
pub async fn session_set_focus(
    state: State<'_, WorkbenchState>,
    args: SessionSetFocusArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    use std::sync::atomic::Ordering;
    state
        .focused_session_id
        .store(args.session_id.unwrap_or(0), Ordering::Release);
    Ok(serde_json::json!({}))
}

/// Args for `session_read_backlog`.
#[derive(Deserialize)]
pub struct SessionReadBacklogArgs {
    /// Session id.
    pub session_id: i64,
}

/// Return the session's raw-byte replay backlog so a late-attaching pane can
/// restore the initial screen state instead of displaying a blank terminal
/// while the agent re-renders incrementally.
///
/// The backlog is bounded (see [`crate::session::replay::REPLAY_CAP`]).
/// For mirror / wrapper-owned sessions there is no backend PTY, so the
/// response is empty.
#[tauri::command]
pub async fn session_read_backlog(
    state: State<'_, WorkbenchState>,
    args: SessionReadBacklogArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    use base64::Engine;

    let session_id = SessionId::new(args.session_id);
    let snapshot = {
        let live = state.live_sessions.read().await;
        match live.get(&session_id) {
            Some(handle) => handle.replay.lock().await.snapshot(),
            None => Vec::new(),
        }
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(&snapshot);
    Ok(serde_json::json!({ "bytes": encoded }))
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
