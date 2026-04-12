//! Workbench backend library — `run()` bootstraps the Tauri app and all the
//! backend services.
//!
//! T018 + T026: initialize `tracing_subscriber` with env-filter (`AGENTUI_LOG`),
//! pretty output in dev builds and JSON in release builds, open the DB,
//! construct `WorkbenchState`, run crash recovery (T025), bind the daemon
//! socket (T022/T023), and hand control to Tauri with the notification plugin
//! installed. US1 will grow the `tauri::generate_handler![]` list — for now
//! the handler surface is empty.

#![warn(missing_docs)]

pub mod daemon;
pub mod db;
pub mod error;
pub mod model;
pub mod session;
pub mod state;

use crate::db::Database;
use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::session::recovery::reconcile_and_reattach;
use crate::state::WorkbenchState;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize structured logging. Uses env-filter controlled by `AGENTUI_LOG`
/// (fallback: `info`). Pretty output in debug builds, JSON in release.
pub fn init_tracing() {
    let filter =
        EnvFilter::try_from_env("AGENTUI_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

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
        let state = WorkbenchState::new(db);

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
        info!("daemon socket bound at {}", daemon_handle.socket_path.display());

        Ok((state, daemon_handle))
    });

    let (state, _daemon_handle) = match bootstrap {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("agentui-workbench bootstrap failed: {e}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Convenience wrapper for tests: open an in-memory DB and build a
/// `WorkbenchState` around it.
#[cfg(test)]
pub async fn test_state() -> WorkbenchResult<WorkbenchState> {
    let db = Database::open_in_memory().await?;
    Ok(WorkbenchState::new(db))
}
