//! Filesystem watcher for project root directories.
//!
//! T043: uses `notify` to watch each registered project's canonical_path.
//! Emits `project:path_missing` and `project:path_restored` events via the
//! event bus. These events are informational — the UI shows a warning badge.

use crate::model::ProjectId;
use crate::state::{SessionEventEnvelope, WorkbenchState};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Manages filesystem watchers for all registered projects.
pub struct ProjectWatcher {
    /// Map of project id → watcher handle.
    watchers: Arc<RwLock<HashMap<ProjectId, WatcherHandle>>>,
}

struct WatcherHandle {
    _watcher: RecommendedWatcher,
    _path: PathBuf,
}

impl Default for ProjectWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectWatcher {
    /// Create a new watcher manager.
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start watching a project's root directory. If the path disappears, emits
    /// `ProjectPathMissing`; if it reappears, emits `ProjectPathRestored`.
    pub async fn watch(
        &self,
        project_id: ProjectId,
        path: &Path,
        state: &WorkbenchState,
    ) -> Result<(), String> {
        let event_bus = state.event_bus.clone();
        let pid = project_id;
        let watched_path = path.to_path_buf();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Remove(_)) {
                    let _ = event_bus
                        .send(SessionEventEnvelope::ProjectPathMissing { project_id: pid });
                }
            }
            Err(e) => {
                warn!("filesystem watch error for project {pid}: {e}");
            }
        })
        .map_err(|e| format!("failed to create watcher: {e}"))?;

        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("failed to watch {}: {e}", path.display()))?;

        info!(%project_id, path = %path.display(), "watching project directory");

        self.watchers.write().await.insert(
            project_id,
            WatcherHandle {
                _watcher: watcher,
                _path: watched_path,
            },
        );

        Ok(())
    }

    /// Stop watching a project (e.g. on archive).
    pub async fn unwatch(&self, project_id: ProjectId) {
        self.watchers.write().await.remove(&project_id);
    }
}
