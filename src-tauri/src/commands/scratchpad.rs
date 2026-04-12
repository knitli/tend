//! T110: Tauri command handlers for scratchpad (notes + reminders + overview).
//!
//! Each struct and command is named to match the contract in
//! `contracts/tauri-commands.md §4`.

use crate::error::WorkbenchError;
use crate::model::{NoteId, ProjectId, ReminderId, ReminderState};
use crate::scratchpad::{NoteService, OverviewService, ReminderService};
use crate::state::WorkbenchState;
use serde::Deserialize;
use tauri::State;

// ---- Notes ----

/// Args for `note_list`.
#[derive(Deserialize)]
pub struct NoteListArgs {
    /// Project id to list notes for.
    pub project_id: i64,
    /// Max results.
    #[serde(default)]
    pub limit: Option<i64>,
    /// Pagination cursor.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// List notes for a project.
#[tauri::command]
pub async fn note_list(
    state: State<'_, WorkbenchState>,
    args: NoteListArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let (notes, next_cursor) = NoteService::list(
        &state.db,
        ProjectId::new(args.project_id),
        args.limit,
        args.cursor.as_deref(),
    )
    .await?;
    Ok(serde_json::json!({ "notes": notes, "next_cursor": next_cursor }))
}

/// Args for `note_create`.
#[derive(Deserialize)]
pub struct NoteCreateArgs {
    /// Project id.
    pub project_id: i64,
    /// Note body.
    pub content: String,
}

/// Create a new note.
#[tauri::command]
pub async fn note_create(
    state: State<'_, WorkbenchState>,
    args: NoteCreateArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let note =
        NoteService::create(&state.db, ProjectId::new(args.project_id), &args.content).await?;
    Ok(serde_json::json!({ "note": note }))
}

/// Args for `note_update`.
#[derive(Deserialize)]
pub struct NoteUpdateArgs {
    /// Note id.
    pub id: i64,
    /// Updated content.
    pub content: String,
}

/// Update a note's content.
#[tauri::command]
pub async fn note_update(
    state: State<'_, WorkbenchState>,
    args: NoteUpdateArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let note = NoteService::update(&state.db, NoteId::new(args.id), &args.content).await?;
    Ok(serde_json::json!({ "note": note }))
}

/// Args for `note_delete`.
#[derive(Deserialize)]
pub struct NoteDeleteArgs {
    /// Note id.
    pub id: i64,
}

/// Delete a note.
#[tauri::command]
pub async fn note_delete(
    state: State<'_, WorkbenchState>,
    args: NoteDeleteArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    NoteService::delete(&state.db, NoteId::new(args.id)).await?;
    Ok(serde_json::json!({}))
}

// ---- Reminders ----

/// Args for `reminder_list`.
#[derive(Deserialize)]
pub struct ReminderListArgs {
    /// Filter by project.
    #[serde(default)]
    pub project_id: Option<i64>,
    /// Filter by state ("open" or "done").
    #[serde(default)]
    pub state: Option<String>,
    /// Max results.
    #[serde(default)]
    pub limit: Option<i64>,
    /// Pagination cursor.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// List reminders with optional filters.
#[tauri::command]
pub async fn reminder_list(
    state: State<'_, WorkbenchState>,
    args: ReminderListArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let project_id = args.project_id.map(ProjectId::new);
    let reminder_state = args
        .state
        .as_deref()
        .map(|s| s.parse::<ReminderState>())
        .transpose()
        .map_err(WorkbenchError::internal)?;

    let (reminders, next_cursor) = ReminderService::list(
        &state.db,
        project_id,
        reminder_state,
        args.limit,
        args.cursor.as_deref(),
    )
    .await?;
    Ok(serde_json::json!({ "reminders": reminders, "next_cursor": next_cursor }))
}

/// Args for `reminder_create`.
#[derive(Deserialize)]
pub struct ReminderCreateArgs {
    /// Project id.
    pub project_id: i64,
    /// Reminder body.
    pub content: String,
}

/// Create a new reminder.
#[tauri::command]
pub async fn reminder_create(
    state: State<'_, WorkbenchState>,
    args: ReminderCreateArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let reminder =
        ReminderService::create(&state.db, ProjectId::new(args.project_id), &args.content).await?;
    Ok(serde_json::json!({ "reminder": reminder }))
}

/// Args for `reminder_set_state`.
#[derive(Deserialize)]
pub struct ReminderSetStateArgs {
    /// Reminder id.
    pub id: i64,
    /// New state ("open" or "done").
    pub state: String,
}

/// Set the state of a reminder.
#[tauri::command]
pub async fn reminder_set_state(
    state: State<'_, WorkbenchState>,
    args: ReminderSetStateArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    let reminder_state: ReminderState = args.state.parse().map_err(WorkbenchError::internal)?;
    let reminder =
        ReminderService::set_state(&state.db, ReminderId::new(args.id), reminder_state).await?;
    Ok(serde_json::json!({ "reminder": reminder }))
}

/// Args for `reminder_delete`.
#[derive(Deserialize)]
pub struct ReminderDeleteArgs {
    /// Reminder id.
    pub id: i64,
}

/// Delete a reminder.
#[tauri::command]
pub async fn reminder_delete(
    state: State<'_, WorkbenchState>,
    args: ReminderDeleteArgs,
) -> Result<serde_json::Value, WorkbenchError> {
    ReminderService::delete(&state.db, ReminderId::new(args.id)).await?;
    Ok(serde_json::json!({}))
}

// ---- Overview ----

/// Get the cross-project overview of open reminders grouped by project.
#[tauri::command]
pub async fn cross_project_overview(
    state: State<'_, WorkbenchState>,
) -> Result<serde_json::Value, WorkbenchError> {
    let groups = OverviewService::overview(&state.db).await?;
    Ok(serde_json::json!({ "groups": groups }))
}
