//! Tauri command handlers for session management.
//!
//! T049: `session_list` and `session_spawn` (GUI-initiated).
//! T093: `session_send_input`, `session_resize`, `session_end` (US3, stubs for now).

use crate::error::{ErrorCode, WorkbenchError};
use crate::model::ProjectId;
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
