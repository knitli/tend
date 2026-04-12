//! T057: `tend run` — CLI wrapper entry point.
//!
//! Wires together argument parsing (T054), IPC (T055), and PTY (T056):
//!
//! 1. Parse args via `clap`.
//! 2. Connect to the daemon socket and perform the `hello` handshake.
//! 3. Register a session with the workbench.
//! 4. Spawn the child command in a PTY.
//! 5. Proxy I/O (PTY <-> user tty) + periodic heartbeat.
//! 6. On child exit, notify `end_session` and exit with the child's code.

mod args;
mod ipc;
mod pty;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{debug, error, info, warn};

use crate::args::{Cli, RunArgs};
use crate::ipc::IpcClient;

/// Heartbeat interval — must be well under the server's staleness timeout.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() {
    // Initialize minimal tracing (stderr only, respects RUST_LOG).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let exit_code = match run().await {
        Ok(code) => code,
        Err(e) => {
            error!("{e:#}");
            eprintln!("tend: {e:#}");
            1
        }
    };

    std::process::exit(exit_code);
}

async fn run() -> Result<i32> {
    let Cli::Run(args) = Cli::parse();

    let project_path = resolve_project(&args)?;
    let cwd = args
        .working_directory
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| project_path.clone());

    info!(project = %project_path.display(), "starting tend run");

    // ---- Connect to daemon ------------------------------------------------
    let mut client = match IpcClient::connect(None).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "tend: could not connect to workbench daemon. \
                 Is the tend workbench running?\n  {e:#}"
            );
            return Ok(2);
        }
    };

    // ---- Hello handshake --------------------------------------------------
    client.hello().await.context("hello handshake failed")?;
    debug!("hello handshake succeeded");

    // ---- Spawn PTY child --------------------------------------------------
    let pty_child = pty::run_child(&args.command, &cwd).context("failed to spawn child process")?;

    let child_pid = pty_child.pid().unwrap_or(0) as i32;
    debug!(pid = child_pid, cmd = ?args.command, "child spawned");

    // ---- Register session -------------------------------------------------
    let (session_id, project_id) = client
        .register_session(
            &project_path.to_string_lossy(),
            args.label.clone(),
            args.working_directory.clone(),
            Some(args.command.clone()),
            child_pid,
        )
        .await
        .context("failed to register session")?;

    info!(session_id, project_id, "session registered");

    // ---- Run proxy + heartbeat in parallel --------------------------------
    //
    // The PTY proxy runs on a blocking thread (it does synchronous I/O).
    // The heartbeat runs as an async task. When the proxy finishes (child
    // exited), we cancel the heartbeat and send end_session.

    let heartbeat_handle = tokio::spawn(heartbeat_loop(session_id));

    let exit_code = tokio::task::spawn_blocking(move || pty::spawn_proxy(pty_child))
        .await
        .context("proxy task panicked")?
        .context("proxy failed")?;

    // Cancel the heartbeat loop.
    heartbeat_handle.abort();

    // ---- End session ------------------------------------------------------
    if let Err(e) = client.end_session(session_id, Some(exit_code)).await {
        warn!("failed to send end_session: {e:#}");
    } else {
        debug!(session_id, exit_code, "session ended");
    }

    Ok(exit_code)
}

/// Resolve the project path from `--project` or fall back to `$PWD`.
fn resolve_project(args: &RunArgs) -> Result<PathBuf> {
    let raw = match &args.project {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().context("failed to determine current directory")?,
    };
    // Canonicalize so the workbench gets an absolute, symlink-resolved path.
    std::fs::canonicalize(&raw)
        .with_context(|| format!("project path does not exist: {}", raw.display()))
}

/// Send periodic heartbeats on a **separate** IPC connection.
///
/// We open a second connection because the primary connection is occupied
/// by the main request flow and we need heartbeats to fire independently.
/// If the connection fails, we log and silently stop — the workbench will
/// notice the missing heartbeats via its staleness timer.
async fn heartbeat_loop(session_id: i64) {
    // Small delay before the first heartbeat to let registration settle.
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client = match IpcClient::connect(None).await {
        Ok(mut c) => {
            // The second connection also needs a hello handshake.
            if let Err(e) = c.hello().await {
                warn!("heartbeat connection hello failed: {e:#}");
                return;
            }
            c
        }
        Err(e) => {
            warn!("heartbeat connection failed: {e:#}");
            return;
        }
    };

    let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
    loop {
        interval.tick().await;
        if let Err(e) = client.heartbeat(session_id).await {
            warn!("heartbeat send failed: {e:#}");
            return;
        }
        debug!(session_id, "heartbeat sent");
    }
}
