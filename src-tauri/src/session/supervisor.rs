//! Per-session tokio task supervisor.
//!
//! T047: `spawn_session_tasks()` starts tasks per live session:
//!   1. **Reader** — drains PTY output, broadcasts, emits events, pings monitor.
//!   2. **Exit watcher** — blocks on pty.wait(), sends exit code to oneshot.
//!   3. **Writer** — reads input/resize/kill from handle channels, applies to PTY.
//!      Also watches for child exit via a tokio oneshot from the exit watcher.
//!   4. **Monitor** — runs `status::run()` for idle detection and IPC updates.

use crate::session::live::LiveSessionActor;
use crate::session::status::{self, StatusUpdate};
use crate::state::{SessionEventEnvelope, WorkbenchState};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, trace, warn};

/// Handles returned from `spawn_session_tasks`.
pub struct SessionTaskHandles {
    /// Send cooperative status updates to the monitor task.
    pub ipc_status_tx: mpsc::UnboundedSender<StatusUpdate>,
}

/// Spawn the reader, writer, and monitor tasks for a live session.
pub fn spawn_session_tasks(actor: LiveSessionActor, state: &WorkbenchState) -> SessionTaskHandles {
    let LiveSessionActor {
        session_id,
        pty,
        mut output_rx,
        writer_rx,
        resize_rx,
        kill_rx,
        status_tx,
        output_tx,
    } = actor;

    let event_bus = state.event_bus.clone();

    // Channels for the status monitor.
    let (activity_tx, activity_rx) = mpsc::unbounded_channel::<()>();
    let (ipc_status_tx, ipc_status_rx) = mpsc::unbounded_channel::<StatusUpdate>();

    // Oneshot for the exit watcher to signal the writer thread.
    let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();

    // ---- Reader task (async — output_rx is already bridged from blocking thread) ----
    let reader_output_tx = output_tx;
    let reader_event_bus = event_bus.clone();
    tokio::spawn(async move {
        while let Some(chunk) = output_rx.recv().await {
            let _ = reader_output_tx.send(chunk.clone());
            let _ = reader_event_bus.send(SessionEventEnvelope::Output {
                session_id,
                bytes: chunk,
            });
            let _ = activity_tx.send(());
        }
        trace!(%session_id, "reader task: PTY output ended");
    });

    // ---- Exit watcher (blocking thread — calls pty.wait()) ----
    // We need to split the PTY: the exit watcher owns pty.wait(), the writer
    // thread owns pty.write_bytes/resize/kill. Since Pty has interior mutability
    // (Arc<Mutex<…>>), we can share the same Pty across threads.
    let pty_for_writer = pty;
    let pty_for_exit = pty_for_writer.clone_for_wait();
    let exit_event_bus = event_bus;
    std::thread::spawn(move || {
        let exit_code = pty_for_exit.wait().ok();
        info!(%session_id, ?exit_code, "exit watcher: child exited");
        let _ = exit_tx.send(exit_code);
        let _ = exit_event_bus.send(SessionEventEnvelope::Ended {
            session_id,
            code: exit_code,
        });
    });

    // ---- Writer task (async — receives from channels + exit signal) ----
    let rt_handle = tokio::runtime::Handle::current();
    let pty_writer = pty_for_writer;
    std::thread::spawn(move || {
        let rt = rt_handle;
        let mut writer_rx = writer_rx;
        let mut resize_rx = resize_rx;
        let mut kill_rx = kill_rx;
        let mut exit_rx = exit_rx;

        loop {
            let action = rt.block_on(async {
                tokio::select! {
                    bytes = writer_rx.recv() => WAction::Input(bytes),
                    size = resize_rx.recv() => WAction::Resize(size),
                    signal = kill_rx.recv() => WAction::Kill(signal),
                    _ = &mut exit_rx => WAction::ChildExited,
                }
            });

            match action {
                WAction::Input(Some(data)) => {
                    if let Err(e) = pty_writer.write_bytes(&data) {
                        warn!(%session_id, %e, "writer: PTY write failed");
                        break;
                    }
                }
                WAction::Input(None) => break,
                WAction::Resize(Some((cols, rows))) => {
                    if let Err(e) = pty_writer.resize(cols, rows) {
                        warn!(%session_id, %e, "writer: resize failed");
                    }
                }
                WAction::Resize(None) => {} // non-fatal
                WAction::Kill(Some(())) => {
                    info!(%session_id, "writer: kill signal");
                    let _ = pty_writer.kill();
                    break;
                }
                WAction::Kill(None) => break,
                WAction::ChildExited => {
                    trace!(%session_id, "writer: child exited, stopping");
                    break;
                }
            }
        }

        trace!(%session_id, "writer thread exiting");
    });

    // ---- Monitor task ----
    tokio::spawn(async move {
        status::run(status_tx, activity_rx, ipc_status_rx).await;
    });

    SessionTaskHandles { ipc_status_tx }
}

enum WAction {
    Input(Option<Vec<u8>>),
    Resize(Option<(u16, u16)>),
    Kill(Option<()>),
    ChildExited,
}
