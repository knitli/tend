//! Shared backend state wired into Tauri via `.manage(state)`.
//!
//! T017: `WorkbenchState` owns the sqlx pool, the live-session handle map,
//! and the event / alert broadcast buses that the event-bridge task (T053)
//! forwards to the frontend over `AppHandle::emit`.

use crate::db::Database;
use crate::model::{Alert, SessionId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Stub placeholder for the live-session actor handle. The real type is
/// defined in `src/session/live.rs` (T045 in US1). Keeping this as an empty
/// struct here means `WorkbenchState` compiles now and can be replaced
/// transparently once the session actor lands.
#[derive(Clone, Debug)]
pub struct LiveSessionHandle {
    // Filled in by T045.
}

/// One envelope type for everything the event bridge forwards to the frontend.
#[derive(Clone, Debug)]
pub enum SessionEventEnvelope {
    /// A session was freshly spawned or reattached after crash recovery.
    Spawned {
        /// Target session id.
        session_id: SessionId,
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
        alert_id: crate::model::AlertId,
        /// Who cleared it.
        by: crate::model::AlertClearedBy,
    },
    /// Filesystem watcher saw a project root disappear.
    ProjectPathMissing {
        /// Owning project.
        project_id: crate::model::ProjectId,
    },
    /// Filesystem watcher saw the project root come back.
    ProjectPathRestored {
        /// Owning project.
        project_id: crate::model::ProjectId,
    },
}

/// The backend's shared state. Handed to every `#[tauri::command]` via
/// `tauri::State<WorkbenchState>`.
#[derive(Clone)]
pub struct WorkbenchState {
    /// Shared database handle.
    pub db: Database,
    /// Map of live-session handles keyed by session id.
    ///
    /// Wrapped in `Arc<RwLock<…>>` so that reader-heavy paths (`session_list`)
    /// don't contend with writers (`session_spawn`, daemon `register_session`).
    pub live_sessions: Arc<RwLock<HashMap<SessionId, LiveSessionHandle>>>,
    /// Broadcast bus for generic session events forwarded to the frontend.
    pub event_bus: broadcast::Sender<SessionEventEnvelope>,
}

impl WorkbenchState {
    /// Construct a fresh state wrapped around an already-open database handle.
    pub fn new(db: Database) -> Self {
        // 1024 is ample for a single-user desktop; the receiver side has a
        // tokio task that forwards to `AppHandle::emit` immediately.
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
            .field(
                "live_sessions",
                &"<Arc<RwLock<HashMap<SessionId, LiveSessionHandle>>>>",
            )
            .field("event_bus", &"<broadcast::Sender<SessionEventEnvelope>>")
            .finish()
    }
}
