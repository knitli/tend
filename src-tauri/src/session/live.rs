//! Live session actor and handle.
//!
//! T045: `LiveSessionHandle` is the Clone-able handle stored in
//! `WorkbenchState::live_sessions`. `LiveSessionActor` owns the PTY.

use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{SessionId, SessionStatus};
use crate::session::pty::{OutputRx, Pty};
use crate::session::status::StatusUpdate;
use portable_pty::PtySize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch, Mutex};

/// Clone-able handle to a live session. Stored in `WorkbenchState::live_sessions`.
#[derive(Clone, Debug)]
pub struct LiveSessionHandle {
    /// The session this handle controls.
    pub session_id: SessionId,
    /// Writer channel — None for mirror sessions.
    writer_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    /// Resize channel — None for mirror sessions.
    resize_tx: Option<mpsc::UnboundedSender<(u16, u16)>>,
    /// Kill signal — None for mirror sessions.
    kill_tx: Option<mpsc::UnboundedSender<()>>,
    /// IPC status update sender — set after supervisor tasks start.
    /// Used by daemon `update_status` to push cooperative status to the monitor.
    ipc_status_tx: Arc<Mutex<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    /// Whether this handle has a real PTY backing it (false for mirrors).
    pub is_mirror: bool,
}

impl LiveSessionHandle {
    /// Create a full (workbench-owned) handle with write/resize/kill channels.
    pub fn full(
        session_id: SessionId,
        writer_tx: mpsc::UnboundedSender<Vec<u8>>,
        resize_tx: mpsc::UnboundedSender<(u16, u16)>,
        kill_tx: mpsc::UnboundedSender<()>,
    ) -> Self {
        Self {
            session_id,
            writer_tx: Some(writer_tx),
            resize_tx: Some(resize_tx),
            kill_tx: Some(kill_tx),
            ipc_status_tx: Arc::new(Mutex::new(None)),
            is_mirror: false,
        }
    }

    /// Create an attached-mirror handle (no PTY, read-only).
    pub fn attached_mirror(session_id: SessionId) -> Self {
        Self {
            session_id,
            writer_tx: None,
            resize_tx: None,
            kill_tx: None,
            ipc_status_tx: Arc::new(Mutex::new(None)),
            is_mirror: true,
        }
    }

    /// Install the IPC status sender. Called after supervisor tasks start.
    pub async fn set_ipc_status_tx(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        *self.ipc_status_tx.lock().await = Some(tx);
    }

    /// Send a cooperative status update to the monitor task.
    pub async fn send_ipc_status(&self, update: StatusUpdate) -> WorkbenchResult<()> {
        let guard = self.ipc_status_tx.lock().await;
        match &*guard {
            Some(tx) => tx.send(update).map_err(|_| {
                WorkbenchError::new(ErrorCode::SessionEnded, "IPC status channel closed")
            }),
            None => Ok(()), // Not yet wired or mirror session — silently ignore.
        }
    }

    /// Write bytes to the PTY stdin.
    pub fn write(&self, bytes: &[u8]) -> WorkbenchResult<()> {
        match &self.writer_tx {
            Some(tx) => tx.send(bytes.to_vec()).map_err(|_| {
                WorkbenchError::new(ErrorCode::SessionEnded, "session writer channel closed")
            }),
            None => Err(WorkbenchError::session_read_only(self.session_id.get())),
        }
    }

    /// Resize the PTY window.
    pub fn resize(&self, cols: u16, rows: u16) -> WorkbenchResult<()> {
        match &self.resize_tx {
            Some(tx) => tx.send((cols, rows)).map_err(|_| {
                WorkbenchError::new(ErrorCode::SessionEnded, "session resize channel closed")
            }),
            None => Err(WorkbenchError::session_read_only(self.session_id.get())),
        }
    }

    /// Signal the session to end.
    pub fn end(&self) -> WorkbenchResult<()> {
        match &self.kill_tx {
            Some(tx) => tx.send(()).map_err(|_| {
                WorkbenchError::new(ErrorCode::SessionEnded, "session kill channel closed")
            }),
            None => Err(WorkbenchError::session_read_only(self.session_id.get())),
        }
    }
}

/// The live-session actor. Owns the PTY and the channel endpoints.
/// Not Clone — the supervisor destructures this.
pub struct LiveSessionActor {
    /// Session id.
    pub session_id: SessionId,
    /// The PTY handle.
    pub pty: Pty,
    /// Receiver for raw PTY output chunks.
    pub output_rx: OutputRx,
    /// Receives input bytes from the handle's write channel.
    pub writer_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Receives resize requests.
    pub resize_rx: mpsc::UnboundedReceiver<(u16, u16)>,
    /// Receives kill signals.
    pub kill_rx: mpsc::UnboundedReceiver<()>,
    /// Watch sender for status updates.
    pub status_tx: watch::Sender<SessionStatus>,
    /// Broadcast sender for output bytes (shared with subscribers).
    pub output_tx: broadcast::Sender<Vec<u8>>,
}

/// Spawn a PTY-backed live session and return the actor + handle.
pub fn spawn_live_session(
    session_id: SessionId,
    command: &[String],
    cwd: &Path,
    env: &BTreeMap<String, String>,
    cols: u16,
    rows: u16,
) -> WorkbenchResult<(LiveSessionActor, LiveSessionHandle)> {
    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };
    let (pty, output_rx) = Pty::spawn(command, cwd, env, size)?;

    let (writer_tx, writer_rx) = mpsc::unbounded_channel();
    let (resize_tx, resize_rx) = mpsc::unbounded_channel();
    let (kill_tx, kill_rx) = mpsc::unbounded_channel();
    let (status_tx, _status_rx) = watch::channel(SessionStatus::Working);
    let (output_tx, _) = broadcast::channel(512);

    let handle = LiveSessionHandle::full(session_id, writer_tx, resize_tx, kill_tx);

    let actor = LiveSessionActor {
        session_id,
        pty,
        output_rx,
        writer_rx,
        resize_rx,
        kill_rx,
        status_tx,
        output_tx,
    };

    Ok((actor, handle))
}
