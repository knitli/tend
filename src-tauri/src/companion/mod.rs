//! Companion terminal service — lazily spawned paired shells.
//!
//! T090: `CompanionService::ensure(session_id)` looks up or creates a companion
//! terminal for a session. T091: output is wired to the event bus.

use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use crate::model::{CompanionId, CompanionTerminal, Pid, SessionId, Timestamp};
use crate::session::pty::Pty;
use crate::state::{LiveCompanionHandle, SessionEventEnvelope, WorkbenchState};
use chrono::Utc;
use portable_pty::PtySize;
use sqlx::Row;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::{info, trace, warn};

/// Companion terminal service — stateless, operates on the shared DB + state.
pub struct CompanionService;

impl CompanionService {
    /// Ensure a companion terminal exists for the given session. If one already
    /// exists and its PTY is alive, return it. If it exists but the PTY is dead,
    /// respawn it. If none exists, create a new one.
    ///
    /// Serialized per session_id via a per-session mutex to prevent TOCTOU
    /// races (C1 review fix).
    pub async fn ensure(
        state: &WorkbenchState,
        session_id: SessionId,
    ) -> WorkbenchResult<CompanionTerminal> {
        // C1 fix: acquire per-session lock to serialize concurrent ensure calls.
        let lock = state.companion_lock(session_id).await;
        let _guard = lock.lock().await;

        // Look up the session's working directory.
        let session_row =
            sqlx::query("SELECT working_directory, status FROM sessions WHERE id = ?1")
                .bind(session_id.get())
                .fetch_optional(state.db.pool())
                .await?;

        let session_row = session_row
            .ok_or_else(|| WorkbenchError::not_found(format!("session {session_id}")))?;

        let status: String = session_row.try_get("status")?;
        if status == "ended" || status == "error" {
            return Err(WorkbenchError::new(
                ErrorCode::SessionEnded,
                format!("session {session_id} has ended"),
            ));
        }

        let working_dir: String = session_row.try_get("working_directory")?;
        let cwd = PathBuf::from(&working_dir);

        // Check for an existing companion row.
        let existing = sqlx::query(
            "SELECT id, pid, shell_path, initial_cwd, started_at, ended_at \
             FROM companion_terminals WHERE session_id = ?1",
        )
        .bind(session_id.get())
        .fetch_optional(state.db.pool())
        .await?;

        if let Some(row) = existing {
            let companion_id = CompanionId::new(row.try_get::<i64, _>("id")?);
            let pid: Option<i64> = row.try_get("pid")?;

            // Check if the existing companion's PTY is still alive.
            if let Some(p) = pid
                && is_process_alive(p as u32)
            {
                // Existing companion is alive — return it.
                let shell_path: String = row.try_get("shell_path")?;
                let initial_cwd: String = row.try_get("initial_cwd")?;
                let started_at: String = row.try_get("started_at")?;
                let ended_at: Option<String> = row.try_get("ended_at")?;

                return Ok(CompanionTerminal {
                    id: companion_id,
                    session_id,
                    pid: Some(Pid(p as i32)),
                    shell_path: PathBuf::from(shell_path),
                    initial_cwd: PathBuf::from(initial_cwd),
                    started_at: parse_timestamp(&started_at)?,
                    ended_at: ended_at.as_deref().map(parse_timestamp).transpose()?,
                });
            }

            // Companion exists but PTY is dead — mark old as ended and respawn.
            let now = Utc::now().to_rfc3339();
            sqlx::query("UPDATE companion_terminals SET ended_at = ?1, pid = NULL WHERE id = ?2")
                .bind(&now)
                .bind(companion_id.get())
                .execute(state.db.pool())
                .await?;

            // Remove stale handle.
            state.live_companions.write().await.remove(&session_id);

            // Respawn in-place by updating the existing row.
            return Self::spawn_companion(state, session_id, &cwd, Some(companion_id)).await;
        }

        // No companion exists — create a new one.
        Self::spawn_companion(state, session_id, &cwd, None).await
    }

    /// Forcibly respawn a companion terminal, killing the existing one if alive.
    ///
    /// Also serialized per session_id. If spawning the replacement fails, the
    /// old companion is already dead — the error is propagated so the frontend
    /// can surface it rather than seeing a stale `NotFound`.
    pub async fn respawn(
        state: &WorkbenchState,
        session_id: SessionId,
    ) -> WorkbenchResult<CompanionTerminal> {
        let lock = state.companion_lock(session_id).await;
        let _guard = lock.lock().await;

        // Validate the session is still alive before tearing down the old
        // companion — avoids killing a working companion for a dead session.
        let session_row =
            sqlx::query("SELECT working_directory, status FROM sessions WHERE id = ?1")
                .bind(session_id.get())
                .fetch_optional(state.db.pool())
                .await?;

        let session_row = session_row
            .ok_or_else(|| WorkbenchError::not_found(format!("session {session_id}")))?;

        let status: String = session_row.try_get("status")?;
        if status == "ended" || status == "error" {
            return Err(WorkbenchError::new(
                ErrorCode::SessionEnded,
                format!("session {session_id} has ended"),
            ));
        }

        let working_dir: String = session_row.try_get("working_directory")?;
        let cwd = PathBuf::from(&working_dir);

        // Kill existing companion if alive.
        let handle = { state.live_companions.write().await.remove(&session_id) };
        if let Some(h) = handle {
            let _ = h.kill();
        }

        // Mark existing row as ended.
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE companion_terminals SET ended_at = ?1, pid = NULL \
             WHERE session_id = ?2 AND ended_at IS NULL",
        )
        .bind(&now)
        .bind(session_id.get())
        .execute(state.db.pool())
        .await?;

        // Check if there's an existing row to reuse (the one we just ended).
        let existing_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM companion_terminals WHERE session_id = ?1")
                .bind(session_id.get())
                .fetch_optional(state.db.pool())
                .await?;

        Self::spawn_companion(state, session_id, &cwd, existing_id.map(CompanionId::new)).await
    }

    /// Kill and clean up the companion for a session (called by the reaper when
    /// a session ends — C2 review fix).
    pub async fn cleanup_for_session(state: &WorkbenchState, session_id: SessionId) {
        // Kill the live handle if present.
        let handle = { state.live_companions.write().await.remove(&session_id) };
        if let Some(h) = handle {
            let _ = h.kill();
        }

        // Mark DB row ended.
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "UPDATE companion_terminals SET ended_at = ?1, pid = NULL \
             WHERE session_id = ?2 AND ended_at IS NULL",
        )
        .bind(&now)
        .bind(session_id.get())
        .execute(state.db.pool())
        .await;

        // Clean up the per-session lock entry.
        state.companion_locks.write().await.remove(&session_id);
    }

    /// Internal: spawn a companion shell, update/insert the DB row, install the
    /// live handle, and start supervisor tasks.
    async fn spawn_companion(
        state: &WorkbenchState,
        session_id: SessionId,
        cwd: &Path,
        existing_id: Option<CompanionId>,
    ) -> WorkbenchResult<CompanionTerminal> {
        let shell = resolve_shell()?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let cwd_str = cwd.to_string_lossy().to_string();

        // Validate cwd exists.
        if !cwd.is_dir() {
            return Err(WorkbenchError::new(
                ErrorCode::CompanionSpawnFailed,
                format!(
                    "companion working directory does not exist: {}",
                    cwd.display()
                ),
            ));
        }

        // Spawn the PTY.
        let command = vec![shell.clone()];
        let env = BTreeMap::new();
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let (pty, output_rx) = Pty::spawn(&command, cwd, &env, size).map_err(|e| {
            WorkbenchError::with_details(
                ErrorCode::CompanionSpawnFailed,
                format!("failed to spawn companion shell: {e}"),
                serde_json::json!({ "shell": &shell, "cwd": &cwd_str }),
            )
        })?;

        let pid = pty.pid().map(|p| Pid(p as i32));

        // Insert or update the DB row.
        // M4: if this fails after PTY spawn, kill the orphan process.
        let companion_id = match Self::upsert_companion_row(
            state,
            session_id,
            existing_id,
            pid,
            &shell,
            &cwd_str,
            &now_str,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                let _ = pty.kill();
                return Err(e);
            }
        };

        let companion = CompanionTerminal {
            id: companion_id,
            session_id,
            pid,
            shell_path: PathBuf::from(&shell),
            initial_cwd: cwd.to_path_buf(),
            started_at: now,
            ended_at: None,
        };

        // M5: bounded writer channel (256 messages) for backpressure.
        let (writer_tx, writer_rx) = tokio::sync::mpsc::channel(256);
        let (resize_tx, resize_rx) = tokio::sync::mpsc::unbounded_channel();
        let (kill_tx, kill_rx) = tokio::sync::mpsc::unbounded_channel();

        let handle = LiveCompanionHandle {
            companion_id,
            session_id,
            writer_tx,
            resize_tx,
            kill_tx,
        };

        // Install handle.
        state
            .live_companions
            .write()
            .await
            .insert(session_id, handle);

        // Start supervisor tasks for the companion.
        spawn_companion_tasks(
            pty, output_rx, writer_rx, resize_rx, kill_rx, session_id, state,
        );

        // Emit companion:spawned event.
        let _ = state
            .event_bus
            .send(SessionEventEnvelope::CompanionSpawned {
                session_id,
                companion: companion.clone(),
            });

        info!(%session_id, %companion_id, ?pid, "companion terminal spawned");
        Ok(companion)
    }

    /// Insert or update the companion_terminals DB row.
    async fn upsert_companion_row(
        state: &WorkbenchState,
        session_id: SessionId,
        existing_id: Option<CompanionId>,
        pid: Option<Pid>,
        shell: &str,
        cwd_str: &str,
        now_str: &str,
    ) -> WorkbenchResult<CompanionId> {
        if let Some(id) = existing_id {
            sqlx::query(
                "UPDATE companion_terminals \
                 SET pid = ?1, shell_path = ?2, initial_cwd = ?3, started_at = ?4, ended_at = NULL \
                 WHERE id = ?5",
            )
            .bind(pid.map(|p| p.0 as i64))
            .bind(shell)
            .bind(cwd_str)
            .bind(now_str)
            .bind(id.get())
            .execute(state.db.pool())
            .await?;
            Ok(id)
        } else {
            let result = sqlx::query(
                "INSERT INTO companion_terminals (session_id, pid, shell_path, initial_cwd, started_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(session_id.get())
            .bind(pid.map(|p| p.0 as i64))
            .bind(shell)
            .bind(cwd_str)
            .bind(now_str)
            .execute(state.db.pool())
            .await?;
            Ok(CompanionId::new(result.last_insert_rowid()))
        }
    }
}

/// Resolve the user's preferred shell. Validates the path exists (L1 fix).
fn resolve_shell() -> WorkbenchResult<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let path = Path::new(&shell);
    if !path.exists() {
        return Err(WorkbenchError::new(
            ErrorCode::CompanionSpawnFailed,
            format!("shell does not exist: {shell}"),
        ));
    }
    Ok(shell)
}

/// Check if a process is still alive using a lightweight /proc check on Linux,
/// falling back to sysinfo on other platforms (H4 fix).
fn is_process_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        use sysinfo::{Pid, System};
        let mut sys = System::new();
        let spid = Pid::from_u32(pid);
        sys.refresh_process(spid);
        sys.process(spid).is_some()
    }
}

/// Parse an RFC-3339 timestamp string.
fn parse_timestamp(s: &str) -> WorkbenchResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| WorkbenchError::internal(format!("invalid timestamp '{s}': {e}")))
}

/// Spawn reader, writer, and cleanup tasks for a companion terminal.
fn spawn_companion_tasks(
    pty: Pty,
    mut output_rx: crate::session::pty::OutputRx,
    mut writer_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    mut resize_rx: tokio::sync::mpsc::UnboundedReceiver<(u16, u16)>,
    mut kill_rx: tokio::sync::mpsc::UnboundedReceiver<()>,
    session_id: SessionId,
    state: &WorkbenchState,
) {
    let event_bus = state.event_bus.clone();
    let live_companions = state.live_companions.clone();
    let db = state.db.clone();

    // H3 fix: capture runtime handle from the calling async context BEFORE
    // spawning std threads, so exit watcher cleanup is unconditional.
    let rt_handle = tokio::runtime::Handle::current();

    // Reader task — forwards PTY output to event bus.
    tokio::spawn(async move {
        while let Some(chunk) = output_rx.recv().await {
            let _ = event_bus.send(SessionEventEnvelope::CompanionOutput {
                session_id,
                bytes: chunk,
            });
        }
        trace!(%session_id, "companion reader: PTY output ended");
    });

    // Exit watcher + writer thread (combined, like session supervisor).
    let pty_for_exit = pty.clone_for_wait();
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel::<Option<i32>>();

    // Exit watcher thread — uses captured rt_handle for unconditional cleanup (H3).
    let exit_rt = rt_handle.clone();
    let exit_companions = live_companions;
    let exit_db = db;
    let exit_sid = session_id;
    std::thread::spawn(move || {
        let exit_code = pty_for_exit.wait().ok();
        trace!(%exit_sid, ?exit_code, "companion exit watcher: shell exited");
        let _ = exit_tx.send(exit_code);

        // Unconditional cleanup using the captured handle.
        exit_rt.spawn(async move {
            let now = Utc::now().to_rfc3339();
            let _ = sqlx::query(
                "UPDATE companion_terminals SET ended_at = ?1, pid = NULL \
                 WHERE session_id = ?2 AND ended_at IS NULL",
            )
            .bind(&now)
            .bind(exit_sid.get())
            .execute(exit_db.pool())
            .await;
            exit_companions.write().await.remove(&exit_sid);
        });
    });

    // Writer thread.
    std::thread::spawn(move || {
        let rt = rt_handle;
        let mut exit_rx = exit_rx;

        loop {
            let action = rt.block_on(async {
                tokio::select! {
                    bytes = writer_rx.recv() => CAction::Input(bytes),
                    size = resize_rx.recv() => CAction::Resize(size),
                    signal = kill_rx.recv() => CAction::Kill(signal),
                    _ = &mut exit_rx => CAction::ChildExited,
                }
            });

            match action {
                CAction::Input(Some(data)) => {
                    if let Err(e) = pty.write_bytes(&data) {
                        warn!(%session_id, %e, "companion writer: PTY write failed");
                        break;
                    }
                }
                CAction::Input(None) => break,
                CAction::Resize(Some((cols, rows))) => {
                    if let Err(e) = pty.resize(cols, rows) {
                        warn!(%session_id, %e, "companion writer: resize failed");
                    }
                }
                CAction::Resize(None) => {}
                CAction::Kill(Some(())) => {
                    info!(%session_id, "companion writer: kill signal");
                    let _ = pty.kill();
                    break;
                }
                CAction::Kill(None) => break,
                CAction::ChildExited => {
                    trace!(%session_id, "companion writer: shell exited, stopping");
                    break;
                }
            }
        }

        trace!(%session_id, "companion writer thread exiting");
    });
}

enum CAction {
    Input(Option<Vec<u8>>),
    Resize(Option<(u16, u16)>),
    Kill(Option<()>),
    ChildExited,
}
