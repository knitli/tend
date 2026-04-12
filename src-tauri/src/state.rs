//! Shared backend state wired into Tauri via `.manage(state)`.

use crate::db::Database;
use crate::model::{Alert, AlertClearedBy, AlertId, ProjectId, Session, SessionId};
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
}

/// The backend's shared state.
#[derive(Clone)]
pub struct WorkbenchState {
    /// Shared database handle.
    pub db: Database,
    /// Map of live-session handles keyed by session id.
    pub live_sessions: Arc<RwLock<HashMap<SessionId, LiveSessionHandle>>>,
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
            event_bus,
        }
    }
}

impl std::fmt::Debug for WorkbenchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkbenchState")
            .field("db", &"<SqlitePool>")
            .field("live_sessions", &"<Arc<RwLock<…>>>")
            .field("event_bus", &"<broadcast::Sender<…>>")
            .finish()
    }
}
