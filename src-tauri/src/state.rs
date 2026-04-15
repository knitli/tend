//! Shared backend state wired into Tauri via `.manage(state)`.

use crate::db::Database;
use crate::model::{
    Alert, AlertClearedBy, AlertId, CompanionId, CompanionTerminal, ProjectId, Session, SessionId,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::{Mutex, RwLock, broadcast};

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
    /// Writer channel for companion PTY stdin (bounded for backpressure — M5).
    pub writer_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    /// Resize channel for companion PTY.
    pub resize_tx: tokio::sync::mpsc::UnboundedSender<(u16, u16)>,
    /// Kill signal.
    pub kill_tx: tokio::sync::mpsc::UnboundedSender<()>,
}

impl LiveCompanionHandle {
    /// Write bytes to the companion PTY stdin.
    pub fn write(&self, bytes: &[u8]) -> crate::error::WorkbenchResult<()> {
        self.writer_tx.try_send(bytes.to_vec()).map_err(|_| {
            crate::error::WorkbenchError::new(
                crate::error::ErrorCode::WriteFailed,
                "companion writer channel closed or full",
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
    /// Per-session companion locks to serialize ensure/respawn and prevent
    /// TOCTOU races (C1 review fix).
    pub companion_locks: Arc<RwLock<HashMap<SessionId, Arc<Mutex<()>>>>>,
    /// Broadcast bus for session events forwarded to the frontend.
    pub event_bus: broadcast::Sender<SessionEventEnvelope>,
    /// Debounced workspace writer (C1/C2 fix). Initialized once in `run()`.
    pub workspace_debouncer: Option<crate::workspace::WorkspaceDebouncer>,
    /// Captured user shell environment (PATH, HOME, etc. from a fresh login +
    /// interactive shell). Merged into spawned-session env so workbench-launched
    /// agents see the same PATH the user has in their terminal. Populated once
    /// during bootstrap; empty until then.
    pub shell_env: Arc<HashMap<String, String>>,
    /// Set of session ids whose raw PTY output should be forwarded to the
    /// webview. Empty set means "no session active" (overview / empty state).
    /// The event bridge reads this on every PTY chunk to decide whether to
    /// forward Output/CompanionOutput events. Most agent UIs emit output at
    /// ~20 Hz; with N sessions running, bridging all of it through base64 +
    /// JSON + Tauri IPC for panes that aren't mounted is wasted CPU. Raw
    /// bytes are still captured in the per-session replay buffer so newly-
    /// visible panes catch up via `session_read_backlog`.
    ///
    /// Phase 4 generalised this from a single `AtomicI64` to a set so the
    /// multi-pane workspace can mark multiple sessions visible simultaneously.
    /// `std::sync::RwLock` is used (not the tokio variant) because the hot
    /// path is a sync `HashSet::contains` — async acquisition would add a
    /// yield point per PTY chunk for no benefit on a set of ≤ dozens of ids.
    /// Writes (focus/visible changes) are rare (user-driven) and brief.
    pub visible_session_ids: Arc<StdRwLock<HashSet<i64>>>,
}

impl WorkbenchState {
    /// Construct a fresh state wrapped around an already-open database handle.
    pub fn new(db: Database) -> Self {
        let (event_bus, _rx) = broadcast::channel(1024);
        Self {
            db,
            live_sessions: Arc::new(RwLock::new(HashMap::new())),
            live_companions: Arc::new(RwLock::new(HashMap::new())),
            companion_locks: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            workspace_debouncer: None,
            shell_env: Arc::new(HashMap::new()),
            visible_session_ids: Arc::new(StdRwLock::new(HashSet::new())),
        }
    }

    /// Initialize the workspace debouncer. Called once from `run()` after
    /// the tokio runtime is available.
    pub fn init_debouncer(&mut self) {
        self.workspace_debouncer =
            Some(crate::workspace::WorkspaceDebouncer::spawn(self.db.clone()));
    }

    /// Install the captured user shell environment. Called once during
    /// bootstrap, before Tauri starts accepting commands.
    pub fn set_shell_env(&mut self, env: HashMap<String, String>) {
        self.shell_env = Arc::new(env);
    }

    /// Replace the set of visible sessions. Passing an empty iterator clears
    /// the set (overview / empty-state). Used by `session_set_visible` and
    /// the `session_set_focus` shim.
    pub fn set_visible_sessions<I: IntoIterator<Item = i64>>(&self, ids: I) {
        let mut g = self
            .visible_session_ids
            .write()
            .unwrap_or_else(|e| e.into_inner());
        g.clear();
        g.extend(ids);
    }

    /// Snapshot the current visible-sessions set. For tests / introspection.
    pub fn visible_sessions_snapshot(&self) -> HashSet<i64> {
        self.visible_session_ids
            .read()
            .expect("visible_session_ids lock poisoned")
            .clone()
    }

    /// Get or create a per-session mutex for companion operations.
    pub async fn companion_lock(&self, session_id: SessionId) -> Arc<Mutex<()>> {
        let locks = self.companion_locks.read().await;
        if let Some(lock) = locks.get(&session_id) {
            return Arc::clone(lock);
        }
        drop(locks);

        let mut locks = self.companion_locks.write().await;
        Arc::clone(
            locks
                .entry(session_id)
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
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
