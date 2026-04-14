//! Workbench backend library — `run()` bootstraps the Tauri app and all the
//! backend services.
//!
//! T018 + T026 + T044/T049/T053: initialize tracing, open DB, crash recovery,
//! bind daemon, register Tauri commands, start event bridge.

#![warn(missing_docs)]

pub mod commands;
pub mod companion;
pub mod daemon;
pub mod db;
pub mod error;
pub mod model;
pub mod notifications;
pub mod project;
pub mod scratchpad;
pub mod session;
pub mod state;
pub mod workspace;

use crate::db::Database;
use crate::error::WorkbenchResult;
use crate::session::recovery::reconcile_and_reattach;
use crate::state::WorkbenchState;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initialize structured logging. Uses env-filter controlled by `AGENTUI_LOG`
/// (fallback: `info`). Pretty output in debug builds, JSON in release.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_env("AGENTUI_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    #[cfg(debug_assertions)]
    let layer = fmt::layer().pretty();

    #[cfg(not(debug_assertions))]
    let layer = fmt::layer().json();

    // Use try_init so tests can set up tracing without panicking if it's
    // already been installed by a previous test in the same process.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init();
}

/// Main entry point. Called from `main.rs`.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    // Build a runtime so we can open the DB + daemon before Tauri takes over.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let bootstrap: WorkbenchResult<(WorkbenchState, daemon::DaemonHandle)> = rt.block_on(async {
        info!("opening workbench database at default XDG path");
        let db = Database::open_default().await?;
        let mut state = WorkbenchState::new(db);

        // Crash recovery (T025): single merged pass. Must run BEFORE Tauri's
        // frontend is ready to call `session_list`.
        info!("running crash recovery pass");
        let report = reconcile_and_reattach(&state).await?;
        info!(
            reattached = report.reattached.len(),
            ended = report.ended.len(),
            "crash recovery complete"
        );

        // Daemon IPC socket (T022/T023).
        let daemon_handle = daemon::spawn_daemon(state.clone(), None).await?;
        info!(
            "daemon socket bound at {}",
            daemon_handle.socket_path.display()
        );

        // Session reaper — listens for child-exit events and updates the DB.
        session::reaper::spawn_reaper(state.clone());

        // C1: Initialize the workspace debouncer (100 ms coalescing).
        // Must be called inside the runtime block so tokio::spawn succeeds.
        state.init_debouncer();

        Ok((state, daemon_handle))
    });

    let (state, _daemon_handle) = match bootstrap {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("tend-workbench bootstrap failed: {e}");
            std::process::exit(1);
        }
    };

    let state_clone = state.clone();
    let shutdown_state = state.clone();

    // Tauri's `.setup()` and `.run()` callbacks execute on threads that do not
    // have a Tokio runtime entered. Capture a handle to our bootstrap runtime
    // so we can spawn onto it and block_on from those callbacks.
    let setup_rt_handle = rt.handle().clone();
    let shutdown_rt_handle = rt.handle().clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::projects::project_list,
            commands::projects::project_register,
            commands::projects::project_update,
            commands::projects::project_archive,
            commands::projects::project_unarchive,
            commands::sessions::session_list,
            commands::sessions::session_spawn,
            commands::sessions::session_activate,
            commands::sessions::session_send_input,
            commands::sessions::session_resize,
            commands::sessions::session_end,
            commands::companions::companion_send_input,
            commands::companions::companion_resize,
            commands::companions::companion_respawn,
            commands::scratchpad::note_list,
            commands::scratchpad::note_create,
            commands::scratchpad::note_update,
            commands::scratchpad::note_delete,
            commands::scratchpad::reminder_list,
            commands::scratchpad::reminder_create,
            commands::scratchpad::reminder_set_state,
            commands::scratchpad::reminder_delete,
            commands::scratchpad::cross_project_overview,
            commands::notifications::notification_preference_get,
            commands::notifications::notification_preference_set,
            commands::notifications::session_acknowledge_alert,
            commands::workspace::workspace_get,
            commands::workspace::workspace_save,
            commands::workspace::layout_list,
            commands::workspace::layout_save,
            commands::workspace::layout_restore,
            commands::workspace::layout_delete,
        ])
        .setup(move |app| {
            // Enter the bootstrap runtime so tokio::spawn calls below (and any
            // spawn calls inside spawn_event_bridge) find a runtime context.
            let _guard = setup_rt_handle.enter();

            // T053: event bridge — forward state.event_bus → Tauri events.
            commands::events::spawn_event_bridge(app.handle().clone(), &state_clone);

            // T126: emit workspace:restored with the hydrated state so the
            // frontend can bootstrap without an explicit workspace_get call.
            let app_handle = app.handle().clone();
            let db = state_clone.db.clone();
            setup_rt_handle.spawn(async move {
                match workspace::WorkspaceService::get(&db).await {
                    Ok(ws) => {
                        use tauri::Emitter;
                        // M2 fix: wrap in { state: ... } to match WorkspaceRestoredEvent shape.
                        let _ = app_handle
                            .emit("workspace:restored", serde_json::json!({ "state": ws }));
                        info!("emitted workspace:restored on startup");
                    }
                    Err(e) => {
                        tracing::error!("failed to load workspace state on startup: {e}");
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app, event| {
            // C2: Flush workspace debouncer on graceful exit.
            if let tauri::RunEvent::ExitRequested { .. } = &event
                && let Some(ref debouncer) = shutdown_state.workspace_debouncer
            {
                let debouncer = debouncer.clone();
                let result = shutdown_rt_handle.block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), debouncer.flush()).await
                });
                match result {
                    Ok(()) => info!("workspace debouncer flushed on exit"),
                    Err(_) => warn!("workspace debouncer flush timed out after 5s"),
                }
            }
        });
}

/// Convenience wrapper for tests: open an in-memory DB and build a
/// `WorkbenchState` around it.
#[cfg(test)]
pub async fn test_state() -> WorkbenchResult<WorkbenchState> {
    let db = Database::open_in_memory().await?;
    Ok(WorkbenchState::new(db))
}
