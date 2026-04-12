//! Minimal cooperative-first status monitor.
//!
//! T046: `run()` watches output activity and IPC `update_status` signals to
//! maintain the session's current status. In US1 (Phase 3) the monitor is
//! cooperative-first: if the agent pushes status via IPC, that wins. The only
//! heuristic in this phase is idle detection — if no output arrives for
//! `IDLE_TIMEOUT`, the monitor sets status to `Idle`.
//!
//! Full heuristic detection (needs_input pattern matching, Tier 2 muting after
//! cooperative IPC, adversarial PTY corpus) is deferred to US2.

use crate::model::SessionStatus;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::trace;

/// How long after the last output chunk before the monitor declares the session
/// idle.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// A status update received from the cooperative IPC channel
/// (`update_status` daemon verb).
#[derive(Clone, Debug)]
pub struct StatusUpdate {
    /// The new status to apply.
    pub status: SessionStatus,
}

/// Run the status monitor loop. Returns when the `activity_rx` channel closes
/// (i.e., the reader task exited because the PTY EOF'd or the session ended).
///
/// # Arguments
///
/// * `status_tx` — watch sender to publish status changes.
/// * `activity_rx` — receives a unit `()` each time the reader task forwards
///   an output chunk. Used as a heartbeat for idle detection.
/// * `ipc_rx` — receives cooperative `StatusUpdate` pushes from the daemon
///   `update_status` handler.
pub async fn run(
    status_tx: watch::Sender<SessionStatus>,
    mut activity_rx: mpsc::UnboundedReceiver<()>,
    mut ipc_rx: mpsc::UnboundedReceiver<StatusUpdate>,
) {
    loop {
        tokio::select! {
            // Output activity heartbeat — session is working.
            result = activity_rx.recv() => {
                match result {
                    Some(()) => {
                        let _ = status_tx.send(SessionStatus::Working);
                        trace!("status monitor: activity received, status -> Working");
                    }
                    None => {
                        // Reader task exited (PTY closed). The supervisor will
                        // handle the Ended transition.
                        trace!("status monitor: activity channel closed, exiting");
                        return;
                    }
                }
            }

            // Cooperative IPC status push — always wins.
            update = ipc_rx.recv() => {
                match update {
                    Some(StatusUpdate { status }) => {
                        let _ = status_tx.send(status);
                        trace!(?status, "status monitor: IPC status update applied");
                    }
                    None => {
                        // IPC channel closed — continue monitoring activity.
                        trace!("status monitor: IPC channel closed, continuing with activity only");
                    }
                }
            }

            // Idle timeout — no output for IDLE_TIMEOUT seconds.
            _ = tokio::time::sleep(IDLE_TIMEOUT) => {
                // Only transition to Idle if currently Working. Don't override
                // NeedsInput or other IPC-pushed states.
                if *status_tx.borrow() == SessionStatus::Working {
                    let _ = status_tx.send(SessionStatus::Idle);
                    trace!("status monitor: idle timeout elapsed, status -> Idle");
                }
            }
        }
    }
}
