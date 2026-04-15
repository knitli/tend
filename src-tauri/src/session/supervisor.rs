//! Per-session tokio task supervisor.
//!
//! T047: `spawn_session_tasks()` starts tasks per live session:
//!   1. **Reader** — drains PTY output, broadcasts, emits events, pings monitor.
//!   2. **Exit watcher** — blocks on pty.wait(), sends exit code to oneshot.
//!   3. **Writer** — reads input/resize/kill from handle channels, applies to PTY.
//!      Also watches for child exit via a tokio oneshot from the exit watcher.
//!   4. **Monitor** — runs `status::run()` for idle detection, IPC updates, and
//!      heuristic prompt detection (T073).

use crate::session::live::LiveSessionActor;
use crate::session::status::{self, StatusUpdate};
use crate::state::{SessionEventEnvelope, WorkbenchState};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, trace, warn};

/// How long to accumulate PTY bytes before emitting a batched
/// `SessionEventEnvelope::Output`. ~60 Hz — imperceptible latency for
/// interactive feel, but cuts IPC / base64 / JSON overhead by 3–10× for TUIs
/// that emit cursor/status updates at 20+ Hz.
const OUTPUT_BATCH_WINDOW: Duration = Duration::from_millis(16);

/// Minimum interval between `ActivitySummary::record_chunk` calls for sessions
/// that are NOT currently focused. The activity summary drives list-row
/// previews which don't need sub-second precision; parsing ANSI on every
/// cursor-blink chunk is wasted work.
const UNFOCUSED_ACTIVITY_INTERVAL: Duration = Duration::from_millis(1000);

/// Handles returned from `spawn_session_tasks`.
pub struct SessionTaskHandles {
    /// Send cooperative status updates to the monitor task.
    pub ipc_status_tx: mpsc::UnboundedSender<StatusUpdate>,
}

/// Spawn the reader, writer, and monitor tasks for a live session.
///
/// `activity` is the shared `ActivitySummary` from the `LiveSessionHandle`
/// for this session. The reader task feeds output chunks into it (T135).
pub fn spawn_session_tasks(
    actor: LiveSessionActor,
    state: &WorkbenchState,
    activity: std::sync::Arc<tokio::sync::Mutex<crate::session::activity::ActivitySummary>>,
    replay: std::sync::Arc<tokio::sync::Mutex<crate::session::replay::ReplayBuffer>>,
) -> SessionTaskHandles {
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
    let visible = state.visible_session_ids.clone();

    // Channels for the status monitor.
    let (activity_tx, activity_rx) = mpsc::unbounded_channel::<()>();
    let (ipc_status_tx, ipc_status_rx) = mpsc::unbounded_channel::<StatusUpdate>();
    // Channel for raw PTY output → heuristic detector (T073).
    let (heuristic_tx, heuristic_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Oneshot for the exit watcher to signal the writer thread.
    let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();

    // ---- Reader task ----
    //
    // Structure (what's on the hot path, per PTY chunk):
    //   1. Replay-buffer push (required for pane catch-up on focus change).
    //   2. activity_tx unit-signal (cheap; resets idle timer in status monitor).
    //   3. heuristic_tx send (needed for needs_input detection).
    //   4. ActivitySummary::record_chunk — gated: run every chunk while
    //      focused, otherwise at most once per UNFOCUSED_ACTIVITY_INTERVAL.
    //   5. Accumulate chunk into a batch buffer.
    //
    // Separately, a batched `SessionEventEnvelope::Output` is emitted to the
    // event bus every OUTPUT_BATCH_WINDOW via a tokio timer + select loop.
    // The event bridge further filters Output emissions by focus so only the
    // active pane actually pays IPC cost.
    let reader_output_tx = output_tx;
    let reader_event_bus = event_bus.clone();
    let reader_activity = activity;
    let reader_replay = replay;
    let reader_visible = visible.clone();

    enum Step {
        Chunk(Vec<u8>),
        Flush,
        Closed,
    }

    tokio::spawn(async move {
        let mut batch: Vec<u8> = Vec::new();
        let mut deadline: Option<Instant> = None;
        // Initialize so the first chunk triggers a record regardless of focus.
        let mut last_recorded = Instant::now()
            .checked_sub(UNFOCUSED_ACTIVITY_INTERVAL)
            .unwrap_or_else(Instant::now);

        loop {
            let step = match deadline {
                Some(d) => tokio::select! {
                    biased;
                    maybe = output_rx.recv() => match maybe {
                        Some(c) => Step::Chunk(c),
                        None => Step::Closed,
                    },
                    _ = tokio::time::sleep_until(d.into()) => Step::Flush,
                },
                None => match output_rx.recv().await {
                    Some(c) => Step::Chunk(c),
                    None => Step::Closed,
                },
            };

            match step {
                Step::Chunk(chunk) => {
                    // Keep the dead local broadcast alive for now (no
                    // subscribers, but removing it is outside this scope).
                    let _ = reader_output_tx.send(chunk.clone());
                    let _ = activity_tx.send(());
                    reader_replay.lock().await.push(&chunk);

                    // Throttle ANSI-parsing activity record for non-visible
                    // sessions. Visible sessions get every chunk so the
                    // active pane's status stays precise.
                    let is_visible = match reader_visible.read() {
                        Ok(g) => g.contains(&session_id.get()),
                        Err(_) => false,
                    };
                    if is_visible || last_recorded.elapsed() >= UNFOCUSED_ACTIVITY_INTERVAL {
                        reader_activity.lock().await.record_chunk(&chunk);
                        last_recorded = Instant::now();
                    }

                    let _ = heuristic_tx.send(chunk.clone());

                    batch.extend_from_slice(&chunk);
                    if deadline.is_none() {
                        deadline = Some(Instant::now() + OUTPUT_BATCH_WINDOW);
                    }
                }
                Step::Flush => {
                    if !batch.is_empty() {
                        let _ = reader_event_bus.send(SessionEventEnvelope::Output {
                            session_id,
                            bytes: std::mem::take(&mut batch),
                        });
                    }
                    deadline = None;
                }
                Step::Closed => break,
            }
        }

        // Drain remaining bytes on channel close so short-lived commands
        // (e.g., `echo hi; exit`) don't lose their final output.
        if !batch.is_empty() {
            let _ = reader_event_bus.send(SessionEventEnvelope::Output {
                session_id,
                bytes: batch,
            });
        }
        trace!(%session_id, "reader task: PTY output ended");
    });

    // ---- Exit watcher (blocking thread — calls pty.wait()) ----
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
                WAction::Kill(Some(signal)) => {
                    info!(%session_id, ?signal, "writer: kill signal");
                    // Use libc::kill for both TERM and KILL to avoid deadlocking
                    // with the exit watcher thread which holds the child mutex
                    // while blocking on wait().
                    if let Some(pid) = pty_writer.pid() {
                        #[cfg(unix)]
                        {
                            let sig = match signal {
                                crate::session::live::KillSignal::Term => libc::SIGTERM,
                                crate::session::live::KillSignal::Kill => libc::SIGKILL,
                            };
                            // SAFETY: `pid` originates from `pty.pid()` which
                            // returns our direct child's PID. Linux caps PIDs at
                            // 2^22 (< i32::MAX) so the u32→i32 cast is sound.
                            // Inherent TOCTOU: if the child exits and the PID is
                            // recycled between pid() and kill(), we may signal the
                            // wrong process. This is benign: the exit watcher also
                            // fires and stops the writer loop.
                            debug_assert!(pid <= i32::MAX as u32, "pid {pid} exceeds i32::MAX");
                            unsafe {
                                libc::kill(pid as i32, sig);
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = pty_writer.kill();
                        }
                    } else {
                        let _ = pty_writer.kill();
                    }
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

    // ---- Monitor task (T046 idle + T073 heuristic) ----
    tokio::spawn(async move {
        status::run(status_tx, activity_rx, ipc_status_rx, Some(heuristic_rx)).await;
    });

    SessionTaskHandles { ipc_status_tx }
}

enum WAction {
    Input(Option<Vec<u8>>),
    Resize(Option<(u16, u16)>),
    Kill(Option<crate::session::live::KillSignal>),
    ChildExited,
}
