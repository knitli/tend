//! Tauri command handlers for project management.
//!
//! T044: `project_list`, `project_register`, `project_update`, `project_archive`,
//! `project_unarchive` — each delegating to `ProjectService`.

use crate::error::WorkbenchError;
use crate::model::{ProjectId, ProjectSettings};
use crate::project::ProjectService;
use crate::state::WorkbenchState;
use serde::Deserialize;
use tauri::State;

/// Args for `project_list`.
#[derive(Deserialize)]
pub struct ProjectListArgs {
    /// Include archived projects in the response.
    #[serde(default)]
    pub include_archived: bool,
}

/// List registered projects.
#[tauri::command]
pub async fn project_list(
    state: State<'_, WorkbenchState>,
    args: ProjectListArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let projects = ProjectService::list(&state.db, args.include_archived).await?;
    Ok(serde_json::json!({ "projects": projects }))
}

/// Args for `project_register`.
#[derive(Deserialize)]
pub struct ProjectRegisterArgs {
    /// Path to the project directory.
    pub path: String,
    /// Optional display name (defaults to directory basename).
    pub display_name: Option<String>,
}

/// Register a new project.
#[tauri::command]
pub async fn project_register(
    state: State<'_, WorkbenchState>,
    args: ProjectRegisterArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project =
        ProjectService::register(&state.db, &args.path, args.display_name.as_deref()).await?;
    Ok(serde_json::json!({ "project": project }))
}

/// Args for `project_update`.
#[derive(Deserialize)]
pub struct ProjectUpdateArgs {
    /// Project id.
    pub id: i64,
    /// New display name.
    pub display_name: Option<String>,
    /// New settings.
    pub settings: Option<ProjectSettings>,
}

/// Update project metadata.
#[tauri::command]
pub async fn project_update(
    state: State<'_, WorkbenchState>,
    args: ProjectUpdateArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project = ProjectService::update(
        &state.db,
        ProjectId::new(args.id),
        args.display_name.as_deref(),
        args.settings.as_ref(),
    )
    .await?;
    Ok(serde_json::json!({ "project": project }))
}

/// Args wrapping a single project id.
#[derive(Deserialize)]
pub struct ProjectIdArg {
    /// Project id.
    pub id: i64,
}

/// Archive (soft-delete) a project.
#[tauri::command]
pub async fn project_archive(
    state: State<'_, WorkbenchState>,
    args: ProjectIdArg,
) -> Result<serde_json::Value, WorkbenchError> {
    ProjectService::archive(&state.db, ProjectId::new(args.id)).await?;
    Ok(serde_json::json!({}))
}

/// Unarchive a project.
#[tauri::command]
pub async fn project_unarchive(
    state: State<'_, WorkbenchState>,
    args: ProjectIdArg,
) -> Result<serde_json::Value, WorkbenchError> {
    let project = ProjectService::unarchive(&state.db, ProjectId::new(args.id)).await?;
    Ok(serde_json::json!({ "project": project }))
}
