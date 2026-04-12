//! Tauri command handlers for companion terminals.
//!
//! T094: `companion_send_input`, `companion_resize`, `companion_respawn`.

use crate::companion::CompanionService;
use crate::error::{ErrorCode, WorkbenchError};
use crate::model::SessionId;
use crate::state::WorkbenchState;
use serde::Deserialize;
use tauri::State;

/// Args for `companion_send_input`.
#[derive(Deserialize)]
pub struct CompanionSendInputArgs {
    /// The session id whose companion to write to.
    pub session_id: i64,
    /// Bytes to write (UTF-8 string).
    pub bytes: String,
}

/// Write bytes to a companion terminal's PTY stdin.
#[tauri::command]
pub async fn companion_send_input(
    state: State<'_, WorkbenchState>,
    args: CompanionSendInputArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);

    let companions = state.live_companions.read().await;
    let handle = companions.get(&session_id).ok_or_else(|| {
        WorkbenchError::new(
            ErrorCode::NotFound,
            format!("no live companion for session {session_id}"),
        )
    })?;

    handle.write(args.bytes.as_bytes())?;
    Ok(serde_json::json!({}))
}

/// Args for `companion_resize`.
#[derive(Deserialize)]
pub struct CompanionResizeArgs {
    /// The session id whose companion to resize.
    pub session_id: i64,
    /// New column count.
    pub cols: u16,
    /// New row count.
    pub rows: u16,
}

/// Resize a companion terminal's PTY.
#[tauri::command]
pub async fn companion_resize(
    state: State<'_, WorkbenchState>,
    args: CompanionResizeArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);

    let companions = state.live_companions.read().await;
    let handle = companions.get(&session_id).ok_or_else(|| {
        WorkbenchError::new(
            ErrorCode::NotFound,
            format!("no live companion for session {session_id}"),
        )
    })?;

    handle.resize(args.cols, args.rows)?;
    Ok(serde_json::json!({}))
}

/// Args for `companion_respawn`.
#[derive(Deserialize)]
pub struct CompanionRespawnArgs {
    /// The session id whose companion to respawn.
    pub session_id: i64,
}

/// Forcibly recreate a companion terminal.
#[tauri::command]
pub async fn companion_respawn(
    state: State<'_, WorkbenchState>,
    args: CompanionRespawnArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let session_id = SessionId::new(args.session_id);
    let companion = CompanionService::respawn(&state, session_id).await?;
    Ok(serde_json::json!({ "companion": companion }))
}
