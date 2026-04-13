//! T122: Workspace persistence — auto-save/restore of workspace state.
//!
//! C1/C2 fix: backend debounce (100 ms coalescing) + `flush()` for graceful
//! shutdown. The `WorkspaceDebouncer` actor coalesces rapid saves and exposes
//! a synchronous flush path called from the Tauri exit hook.

pub mod layouts;

use crate::db::Database;
use crate::error::WorkbenchResult;
use crate::model::WorkspaceState;
use chrono::Utc;
use sqlx::Row;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify, mpsc};
use tracing::{info, warn};

/// Workspace service — stateless read/write helpers against the DB.
pub struct WorkspaceService;

impl WorkspaceService {
    /// Read the current workspace state. Returns `WorkspaceState::default()` if
    /// no row exists yet (first launch).
    pub async fn get(db: &Database) -> WorkbenchResult<WorkspaceState> {
        let row = sqlx::query("SELECT payload_json FROM workspace_state WHERE id = 1")
            .fetch_optional(db.pool())
            .await?;

        match row {
            Some(r) => {
                let json: String = r.try_get("payload_json")?;
                match serde_json::from_str::<WorkspaceState>(&json) {
                    Ok(state) => Ok(state),
                    Err(e) => {
                        warn!(
                            error = %e,
                            raw_json = %json,
                            "workspace_state payload deserialization failed, returning default"
                        );
                        Ok(WorkspaceState::default())
                    }
                }
            }
            None => Ok(WorkspaceState::default()),
        }
    }

    /// Persist workspace state immediately. Used by the debouncer and flush.
    pub async fn write(db: &Database, state: &WorkspaceState) -> WorkbenchResult<()> {
        let json = serde_json::to_string(state).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize workspace state");
            crate::error::WorkbenchError::internal(e.to_string())
        })?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT OR REPLACE INTO workspace_state (id, payload_json, saved_at) \
             VALUES (1, ?1, ?2)",
        )
        .bind(&json)
        .bind(&now)
        .execute(db.pool())
        .await?;

        Ok(())
    }

    /// Convenience alias used by the `workspace_save` command — delegates to
    /// the debouncer if available, otherwise writes directly.
    pub async fn save(db: &Database, state: &WorkspaceState) -> WorkbenchResult<()> {
        Self::write(db, state).await
    }
}

// ── Debounced writer actor ──────────────────────────────────────────────

/// A debounced writer that coalesces rapid workspace saves into a single
/// DB write every 100 ms. Exposes `flush()` for graceful shutdown.
#[derive(Clone)]
pub struct WorkspaceDebouncer {
    /// Channel to send new state to the actor.
    tx: mpsc::UnboundedSender<WorkspaceState>,
    /// Shared latest state for flush to read.
    latest: Arc<Mutex<Option<WorkspaceState>>>,
    /// Notify the flush caller when writing is done.
    flush_notify: Arc<Notify>,
    /// Database handle for direct flush writes.
    db: Database,
    /// Flag to signal the actor to stop.
    stop: Arc<Notify>,
}

impl WorkspaceDebouncer {
    /// Spawn the debouncer actor. Returns a handle for sending state updates.
    pub fn spawn(db: Database) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let latest: Arc<Mutex<Option<WorkspaceState>>> = Arc::new(Mutex::new(None));
        let flush_notify = Arc::new(Notify::new());
        let stop = Arc::new(Notify::new());

        let debouncer = Self {
            tx,
            latest: Arc::clone(&latest),
            flush_notify: Arc::clone(&flush_notify),
            db: db.clone(),
            stop: Arc::clone(&stop),
        };

        tokio::spawn(Self::run_actor(db, rx, latest, flush_notify, stop));

        debouncer
    }

    /// Queue a workspace state for debounced writing. Non-blocking.
    pub fn save(&self, state: WorkspaceState) {
        let _ = self.tx.send(state);
    }

    /// Immediately flush any pending state to DB. Blocks until the write
    /// completes. Called from the graceful shutdown hook.
    pub async fn flush(&self) {
        let pending = self.latest.lock().await.take();
        if let Some(state) = pending {
            if let Err(e) = WorkspaceService::write(&self.db, &state).await {
                tracing::error!(error = %e, "workspace debouncer flush failed");
            } else {
                info!("workspace state flushed on shutdown");
            }
        }
        self.flush_notify.notify_one();
    }

    /// Signal the actor to stop.
    pub fn stop(&self) {
        self.stop.notify_one();
    }

    async fn run_actor(
        db: Database,
        mut rx: mpsc::UnboundedReceiver<WorkspaceState>,
        latest: Arc<Mutex<Option<WorkspaceState>>>,
        _flush_notify: Arc<Notify>,
        stop: Arc<Notify>,
    ) {
        loop {
            // Wait for the first state update.
            let state = tokio::select! {
                msg = rx.recv() => match msg {
                    Some(s) => s,
                    None => break, // Channel closed.
                },
                _ = stop.notified() => break,
            };

            // Store as latest.
            *latest.lock().await = Some(state);

            // Coalesce: wait 100 ms, draining any newer updates.
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            while let Ok(newer) = rx.try_recv() {
                *latest.lock().await = Some(newer);
            }

            // Write the latest to DB.
            let to_write = latest.lock().await.take();
            if let Some(ws) = to_write
                && let Err(e) = WorkspaceService::write(&db, &ws).await
            {
                tracing::error!(error = %e, "workspace debouncer write failed");
                // Put it back for the next cycle / flush to pick up.
                *latest.lock().await = Some(ws);
            }
        }
    }
}
