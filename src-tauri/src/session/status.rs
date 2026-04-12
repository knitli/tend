//! Cooperative-first status monitor with Tier 2 heuristic prompt detection.
//!
//! T046: Phase 3 — idle detection + cooperative IPC.
//! T073: Phase 4 — Tier 2 heuristic prompt pattern matching.
//!
//! The monitor watches output activity and IPC `update_status` signals.
//! - **Tier 1 (Cooperative IPC)**: `update_status` from daemon socket always wins.
//!   Once a session has ever received a cooperative update, Tier 2 is permanently
//!   muted for that session.
//! - **Tier 2 (Heuristic)**: Output-activity + prompt pattern matching. Only
//!   active for sessions that have never received a cooperative update.

use crate::model::SessionStatus;
use crate::session::heuristic::{HeuristicDetector, HeuristicResult};
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::Sleep;
use tracing::trace;

/// How long after the last output chunk before the monitor declares the session
/// idle.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// How often the heuristic detector checks for prompt patterns.
const HEURISTIC_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// A status update received from the cooperative IPC channel
/// (`update_status` daemon verb).
#[derive(Clone, Debug)]
pub struct StatusUpdate {
    /// The new status to apply.
    pub status: SessionStatus,
    /// Optional human-readable reason (for alerts).
    pub reason: Option<String>,
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
/// * `output_rx` — receives raw PTY output bytes for the heuristic detector.
///   If `None`, heuristic detection is disabled (e.g., for mirror sessions).
pub async fn run(
    status_tx: watch::Sender<SessionStatus>,
    mut activity_rx: mpsc::UnboundedReceiver<()>,
    mut ipc_rx: mpsc::UnboundedReceiver<StatusUpdate>,
    mut output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
) {
    let mut detector = HeuristicDetector::new();
    let mut heuristic_interval = tokio::time::interval(HEURISTIC_CHECK_INTERVAL);
    heuristic_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Idle timer: only reset on genuine output activity, not on IPC or heuristic ticks.
    let mut idle_deadline: Pin<Box<Sleep>> = Box::pin(tokio::time::sleep(IDLE_TIMEOUT));

    loop {
        tokio::select! {
            // Output activity heartbeat — session is working.
            result = activity_rx.recv() => {
                match result {
                    Some(()) => {
                        // Reset idle timer on genuine output activity.
                        idle_deadline
                            .as_mut()
                            .reset(tokio::time::Instant::now() + IDLE_TIMEOUT);

                        // Only transition to Working if currently Idle. Don't
                        // override NeedsInput from IPC (cooperative takes priority).
                        let current = *status_tx.borrow();
                        if current == SessionStatus::Idle {
                            let _ = status_tx.send(SessionStatus::Working);
                            trace!("status monitor: activity received, status -> Working");
                        } else if current == SessionStatus::NeedsInput && !detector.cooperative_seen {
                            // Heuristic-triggered NeedsInput — new output clears it.
                            let _ = status_tx.send(SessionStatus::Working);
                            detector.reset_trigger();
                            trace!("status monitor: activity cleared heuristic NeedsInput -> Working");
                        }
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
                    Some(StatusUpdate { status, .. }) => {
                        // Mark cooperative IPC as seen — permanently mutes Tier 2.
                        detector.cooperative_seen = true;
                        let _ = status_tx.send(status);
                        trace!(?status, "status monitor: IPC status update applied, Tier 2 muted");
                    }
                    None => {
                        // IPC channel closed — continue monitoring activity.
                        trace!("status monitor: IPC channel closed, continuing with activity only");
                    }
                }
            }

            // Raw PTY output for the heuristic buffer.
            chunk = async {
                match &mut output_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(data) = chunk {
                    detector.feed(&data);
                }
            }

            // Heuristic check interval — run the detector periodically.
            _ = heuristic_interval.tick() => {
                if !detector.cooperative_seen
                    && detector.check() == HeuristicResult::NeedsInput
                    && *status_tx.borrow() != SessionStatus::NeedsInput
                {
                    let _ = status_tx.send(SessionStatus::NeedsInput);
                    trace!("status monitor: heuristic detected needs_input");
                }
            }

            // Idle timeout — no output for IDLE_TIMEOUT seconds.
            // Only fires once after the deadline; reset happens in the activity branch.
            _ = &mut idle_deadline => {
                // Only transition to Idle if currently Working. Don't override
                // NeedsInput or other IPC-pushed states.
                if *status_tx.borrow() == SessionStatus::Working {
                    let _ = status_tx.send(SessionStatus::Idle);
                    trace!("status monitor: idle timeout elapsed, status -> Idle");
                }
                // Re-arm for the next idle cycle.
                idle_deadline
                    .as_mut()
                    .reset(tokio::time::Instant::now() + IDLE_TIMEOUT);
            }
        }
    }
}
