//! Database layer: sqlx pool + migration runner.
//!
//! T015: `Database::open(path)` resolves the XDG data dir, creates the parent
//! directory, opens a sqlx pool, and runs the embedded migrations.

pub mod queries;

use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::info;

/// Handle to the workbench's SQLite database.
#[derive(Clone, Debug)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open the workbench database at the XDG data-local default path.
    ///
    /// Resolves to `~/.local/share/agentui/workbench.db` on Linux. Creates
    /// the parent directory if needed, opens a connection pool, and runs all
    /// forward-only migrations from `src-tauri/migrations/`.
    pub async fn open_default() -> WorkbenchResult<Self> {
        let path = default_db_path()?;
        Self::open(&path).await
    }

    /// Open the workbench database at a specific path. Useful for tests.
    pub async fn open(path: &Path) -> WorkbenchResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                WorkbenchError::new(
                    ErrorCode::Internal,
                    format!("failed to create db parent dir {}: {e}", parent.display()),
                )
            })?;
        }

        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .map_err(|e| {
                WorkbenchError::new(
                    ErrorCode::Internal,
                    format!("invalid sqlite URL for {}: {e}", path.display()),
                )
            })?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .map_err(WorkbenchError::from)?;

        info!("running sqlx migrations at {}", path.display());
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| {
                WorkbenchError::new(ErrorCode::Internal, format!("migration failed: {e}"))
            })?;

        Ok(Self { pool })
    }

    /// Open an in-memory database for tests. Each call returns an isolated DB.
    pub async fn open_in_memory() -> WorkbenchResult<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|e| {
                WorkbenchError::new(ErrorCode::Internal, format!("invalid in-memory opts: {e}"))
            })?
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1) // in-memory DB is per-connection; hold it to one.
            .connect_with(opts)
            .await
            .map_err(WorkbenchError::from)?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| {
                WorkbenchError::new(
                    ErrorCode::Internal,
                    format!("in-memory migration failed: {e}"),
                )
            })?;

        Ok(Self { pool })
    }

    /// Access the underlying sqlx pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Resolve the default database path (`$XDG_DATA_HOME/agentui/workbench.db`).
pub fn default_db_path() -> WorkbenchResult<PathBuf> {
    let data_dir = dirs::data_local_dir().ok_or_else(|| {
        WorkbenchError::new(
            ErrorCode::Internal,
            "could not resolve XDG data local directory",
        )
    })?;
    Ok(data_dir.join("agentui").join("workbench.db"))
}
