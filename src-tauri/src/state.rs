//! Shared backend state wired into Tauri via `.manage(state)`.

use crate::db::Database;
use crate::model::{
    Alert, AlertClearedBy, AlertId, CompanionId, CompanionTerminal, ProjectId, Session, SessionId,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

// Re-export so existing callers can use `crate::state::LiveSessionHandle`.
pub use crate::session::live::LiveSessionHandle;

/// One envelope type for everything the event bridge forwards to the frontend.
#[derive(Clone, Debug)]
pub enum SessionEventEnvelope {
    /// A session was freshly spawned or reattached after crash recovery.
    Spawned {
        /// The full session record (contract: `{ session: Session }`).
        session: Session,
    },
    /// A session's child process exited.
    Ended {
        /// Target session id.
        session_id: SessionId,
        /// Child exit code if known.
        code: Option<i32>,
    },
    /// A chunk of PTY output for a session.
    Output {
        /// Target session id.
        session_id: SessionId,
        /// Raw byte payload.
        bytes: Vec<u8>,
    },
    /// An alert was raised.
    AlertRaised {
        /// The alert record.
        alert: Alert,
    },
    /// An alert was cleared.
    AlertCleared {
        /// Alert id that was cleared.
        alert_id: AlertId,
        /// Who cleared it.
        by: AlertClearedBy,
    },
    /// Filesystem watcher saw a project root disappear.
    ProjectPathMissing {
        /// Owning project.
        project_id: ProjectId,
    },
    /// Filesystem watcher saw the project root come back.
    ProjectPathRestored {
        /// Owning project.
        project_id: ProjectId,
    },
    /// A companion terminal was spawned or respawned.
    CompanionSpawned {
        /// The owning session.
        session_id: SessionId,
        /// The companion record.
        companion: CompanionTerminal,
    },
    /// A chunk of companion terminal output.
    CompanionOutput {
        /// The owning session.
        session_id: SessionId,
        /// Raw byte payload.
        bytes: Vec<u8>,
    },
}

/// Clone-able handle to a live companion terminal.
#[derive(Clone, Debug)]
pub struct LiveCompanionHandle {
    /// The companion id in the DB.
    pub companion_id: CompanionId,
    /// The owning session.
    pub session_id: SessionId,
    /// Writer channel for companion PTY stdin.
    pub writer_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    /// Resize channel for companion PTY.
    pub resize_tx: tokio::sync::mpsc::UnboundedSender<(u16, u16)>,
    /// Kill signal.
    pub kill_tx: tokio::sync::mpsc::UnboundedSender<()>,
}

impl LiveCompanionHandle {
    /// Write bytes to the companion PTY stdin.
    pub fn write(&self, bytes: &[u8]) -> crate::error::WorkbenchResult<()> {
        self.writer_tx.send(bytes.to_vec()).map_err(|_| {
            crate::error::WorkbenchError::new(
                crate::error::ErrorCode::SessionEnded,
                "companion writer channel closed",
            )
        })
    }

    /// Resize the companion PTY window.
    pub fn resize(&self, cols: u16, rows: u16) -> crate::error::WorkbenchResult<()> {
        self.resize_tx.send((cols, rows)).map_err(|_| {
            crate::error::WorkbenchError::new(
                crate::error::ErrorCode::SessionEnded,
                "companion resize channel closed",
            )
        })
    }

    /// Signal the companion to end.
    pub fn kill(&self) -> crate::error::WorkbenchResult<()> {
        self.kill_tx.send(()).map_err(|_| {
            crate::error::WorkbenchError::new(
                crate::error::ErrorCode::SessionEnded,
                "companion kill channel closed",
            )
        })
    }
}

/// The backend's shared state.
#[derive(Clone)]
pub struct WorkbenchState {
    /// Shared database handle.
    pub db: Database,
    /// Map of live-session handles keyed by session id.
    pub live_sessions: Arc<RwLock<HashMap<SessionId, LiveSessionHandle>>>,
    /// Map of live-companion handles keyed by session id (1:1 with session).
    pub live_companions: Arc<RwLock<HashMap<SessionId, LiveCompanionHandle>>>,
    /// Broadcast bus for session events forwarded to the frontend.
    pub event_bus: broadcast::Sender<SessionEventEnvelope>,
}

impl WorkbenchState {
    /// Construct a fresh state wrapped around an already-open database handle.
    pub fn new(db: Database) -> Self {
        let (event_bus, _rx) = broadcast::channel(1024);
        Self {
            db,
            live_sessions: Arc::new(RwLock::new(HashMap::new())),
            live_companions: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
        }
    }
}

impl std::fmt::Debug for WorkbenchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkbenchState")
            .field("db", &"<SqlitePool>")
            .field("live_sessions", &"<Arc<RwLock<…>>>")
            .field("live_companions", &"<Arc<RwLock<…>>>")
            .field("event_bus", &"<broadcast::Sender<…>>")
            .finish()
    }
}
