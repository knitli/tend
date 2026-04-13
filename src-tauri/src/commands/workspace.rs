//! T125: Workspace + layout Tauri commands.

use crate::error::WorkbenchResult;
use crate::model::{Layout, LayoutId, SessionId, WorkspaceState};
use crate::state::WorkbenchState;
use crate::workspace::WorkspaceService;
use crate::workspace::layouts::LayoutService;
use tauri::State;

/// Return the current workspace state for frontend hydration.
#[tauri::command]
pub async fn workspace_get(
    state: State<'_, WorkbenchState>,
) -> WorkbenchResult<WorkspaceGetResponse> {
    let ws = WorkspaceService::get(&state.db).await?;
    Ok(WorkspaceGetResponse { state: ws })
}

/// Persist workspace state. Backend debounces at 100 ms; frontend debounces
/// at 250 ms. Graceful shutdown flushes both layers synchronously.
#[tauri::command]
pub async fn workspace_save(
    state: State<'_, WorkbenchState>,
    args: WorkspaceSaveArgs,
) -> WorkbenchResult<()> {
    if let Some(ref debouncer) = state.workspace_debouncer {
        debouncer.save(args.state);
        Ok(())
    } else {
        // Fallback: direct write (tests or debouncer not yet initialized).
        WorkspaceService::save(&state.db, &args.state).await
    }
}

/// List all named layouts.
#[tauri::command]
pub async fn layout_list(state: State<'_, WorkbenchState>) -> WorkbenchResult<LayoutListResponse> {
    let layouts = LayoutService::list(&state.db).await?;
    Ok(LayoutListResponse { layouts })
}

/// Save a named layout snapshot. Pass `overwrite: true` to replace an
/// existing layout with the same name (H2 fix).
#[tauri::command]
pub async fn layout_save(
    state: State<'_, WorkbenchState>,
    args: LayoutSaveArgs,
) -> WorkbenchResult<LayoutSaveResponse> {
    let layout = LayoutService::save(
        &state.db,
        &args.name,
        &args.state,
        args.overwrite.unwrap_or(false),
    )
    .await?;
    Ok(LayoutSaveResponse { layout })
}

/// Restore a named layout. Returns the state and any missing session ids.
#[tauri::command]
pub async fn layout_restore(
    state: State<'_, WorkbenchState>,
    args: LayoutRestoreArgs,
) -> WorkbenchResult<LayoutRestoreResponse> {
    let (ws, missing) = LayoutService::restore(&state, args.id).await?;
    Ok(LayoutRestoreResponse {
        state: ws,
        missing_sessions: missing,
    })
}

/// Delete a named layout.
#[tauri::command]
pub async fn layout_delete(
    state: State<'_, WorkbenchState>,
    args: LayoutDeleteArgs,
) -> WorkbenchResult<()> {
    LayoutService::delete(&state.db, args.id).await
}

// ── Request / Response shapes ────────────────────────────────────────

/// Response for `workspace_get`.
#[derive(serde::Serialize)]
pub struct WorkspaceGetResponse {
    /// The current workspace state.
    state: WorkspaceState,
}

/// Args for `workspace_save`.
#[derive(serde::Deserialize)]
pub struct WorkspaceSaveArgs {
    /// The state to persist.
    state: WorkspaceState,
}

/// Response for `layout_list`.
#[derive(serde::Serialize)]
pub struct LayoutListResponse {
    /// All saved layouts.
    layouts: Vec<Layout>,
}

/// Args for `layout_save`.
#[derive(serde::Deserialize)]
pub struct LayoutSaveArgs {
    /// Layout name (must be unique unless overwrite is true).
    name: String,
    /// Workspace state snapshot.
    state: WorkspaceState,
    /// If true, overwrite an existing layout with the same name.
    overwrite: Option<bool>,
}

/// Response for `layout_save`.
#[derive(serde::Serialize)]
pub struct LayoutSaveResponse {
    /// The created layout.
    layout: Layout,
}

/// Args for `layout_restore`.
#[derive(serde::Deserialize)]
pub struct LayoutRestoreArgs {
    /// Layout id to restore.
    id: LayoutId,
}

/// Response for `layout_restore`.
#[derive(serde::Serialize)]
pub struct LayoutRestoreResponse {
    /// The restored workspace state.
    state: WorkspaceState,
    /// Session ids referenced in the layout that are no longer alive.
    missing_sessions: Vec<SessionId>,
}

/// Args for `layout_delete`.
#[derive(serde::Deserialize)]
pub struct LayoutDeleteArgs {
    /// Layout id to delete.
    id: LayoutId,
}
