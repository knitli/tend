//! Unix-domain-socket IPC server.
//!
//! T022: length-prefixed-JSON framing over a Unix stream socket at
//! `$XDG_RUNTIME_DIR/tend.sock` (fallback `/tmp/tend-$UID.sock`). Socket
//! permissions are `0600`. Wire types come from `tend-protocol` — no
//! local `protocol.rs`.

use crate::daemon::handlers::dispatch;
use crate::error::{WorkbenchError, WorkbenchResult};
use crate::state::WorkbenchState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tend_protocol::{
    ErrorCode as ProtocolErrorCode, MAX_FRAME_SIZE, Request, Response, error as protocol_error,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Environment variable the workbench sets for spawned CLI clients.
pub const SOCKET_ENV: &str = "AGENTUI_SOCKET";

/// Resolve the default socket path: `$XDG_RUNTIME_DIR/tend.sock` with
/// `/tmp/tend-$UID.sock` as the fallback when `$XDG_RUNTIME_DIR` is unset
/// (macOS, some containers).
pub fn default_socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("tend.sock");
    }
    let uid = {
        #[cfg(unix)]
        // SAFETY: getuid(2) has no preconditions, takes no arguments, and cannot fail.
        unsafe {
            libc_getuid()
        }
        #[cfg(not(unix))]
        {
            0_u32
        }
    };
    PathBuf::from(format!("/tmp/tend-{uid}.sock"))
}

#[cfg(unix)]
unsafe extern "C" {
    fn getuid() -> u32;
}

#[cfg(unix)]
unsafe fn libc_getuid() -> u32 {
    // SAFETY: getuid() is a C FFI call with no preconditions — it always
    // succeeds and has no undefined behavior.
    unsafe { getuid() }
}

/// Handle returned by [`spawn_daemon`]. Holds the listener task so the caller
/// can await or abort it. Removes the socket file on drop.
pub struct DaemonHandle {
    /// Path the listener was bound to.
    pub socket_path: PathBuf,
    /// Background accept-loop task.
    pub task: JoinHandle<()>,
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.socket_path) {
            // ENOENT is fine — socket may already be gone.
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "failed to clean up daemon socket at {}: {e}",
                    self.socket_path.display()
                );
            }
        }
    }
}

/// Bind the daemon socket and spawn an accept loop.
///
/// On success, the socket file exists at `socket_path` with mode `0600` and
/// the accept loop is running. The returned `DaemonHandle::task` resolves
/// when the listener stops (e.g. on process shutdown).
pub async fn spawn_daemon(
    state: WorkbenchState,
    socket_path: Option<PathBuf>,
) -> WorkbenchResult<DaemonHandle> {
    let path = socket_path.unwrap_or_else(default_socket_path);

    // If a socket file exists, find out whether something is actively
    // listening on it before removing it.
    //
    // This protects against the scenario where a second workbench instance
    // clobbers the socket of an already-running workbench, leaving the first
    // instance with a hijacked daemon and confusing UX (sessions registered
    // by the CLI go to the wrong instance, the original UI never updates).
    if path.exists() {
        match tokio::net::UnixStream::connect(&path).await {
            Ok(_) => {
                return Err(WorkbenchError::new(
                    crate::error::ErrorCode::Internal,
                    format!(
                        "another tend workbench appears to be running (daemon socket at {} is accepting connections). \
                         Close the existing instance before starting a new one.",
                        path.display()
                    ),
                ));
            }
            Err(_) => {
                // Connection failed — socket is stale. Remove it so bind() succeeds.
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("could not remove stale socket at {}: {e}", path.display());
                }
            }
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(WorkbenchError::from)?;
    }

    let listener = UnixListener::bind(&path).map_err(WorkbenchError::from)?;
    chmod_0600(&path)?;

    // Export AGENTUI_SOCKET in our own env so child processes the workbench
    // spawns (e.g. T049's session_spawn) can find the socket without any
    // separate discovery dance.
    // SAFETY: This runs once during daemon startup, before any multi-threaded
    // work begins. No concurrent reads of SOCKET_ENV can race.
    unsafe { std::env::set_var(SOCKET_ENV, &path) };

    info!("daemon listening at {}", path.display());

    let state = Arc::new(state);
    let task = tokio::spawn(accept_loop(listener, state));

    Ok(DaemonHandle {
        socket_path: path,
        task,
    })
}

async fn accept_loop(listener: UnixListener, state: Arc<WorkbenchState>) {
    let mut consecutive_errors: u32 = 0;
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                consecutive_errors = 0;
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = serve_connection(stream, state).await {
                        warn!("daemon connection error: {e}");
                    }
                });
            }
            Err(e) => {
                consecutive_errors = consecutive_errors.saturating_add(1);
                if consecutive_errors >= 10 {
                    error!(
                        "daemon accept failed {consecutive_errors} times in a row, giving up: {e}"
                    );
                    return;
                }
                let backoff = std::time::Duration::from_millis(100 * u64::from(consecutive_errors));
                warn!(
                    "daemon accept failed ({consecutive_errors}/10), retrying in {backoff:?}: {e}"
                );
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn serve_connection(
    mut stream: UnixStream,
    state: Arc<WorkbenchState>,
) -> WorkbenchResult<()> {
    // Split once, keep two halves on the single stream using tokio's split.
    let (mut reader, mut writer) = stream.split();

    loop {
        // Read the next frame. EOF (peer closed) exits the loop cleanly.
        let frame = match read_frame(&mut reader).await {
            Ok(Some(frame)) => frame,
            Ok(None) => return Ok(()),
            Err(err) => {
                let response = protocol_error_response(err);
                let _ = write_frame(&mut writer, &response).await;
                return Ok(());
            }
        };

        // Parse the frame into a Request. Malformed → protocol_error.
        let request: Request = match serde_json::from_slice(&frame) {
            Ok(req) => req,
            Err(e) => {
                let response = protocol_error_response(format!("malformed request: {e}"));
                write_frame(&mut writer, &response).await?;
                continue;
            }
        };

        debug!("daemon received {:?}", request);
        let response = dispatch(request, &state).await;
        write_frame(&mut writer, &response).await?;
    }
}

/// Read a little-endian u32-prefixed JSON frame. Returns `Ok(None)` on clean
/// EOF; `Err` on oversize frame, short read, or I/O failure.
pub async fn read_frame<R>(reader: &mut R) -> Result<Option<Vec<u8>>, String>
where
    R: AsyncReadExt + Unpin,
{
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("frame length read failed: {e}")),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(format!(
            "frame of {len} bytes exceeds MAX_FRAME_SIZE ({MAX_FRAME_SIZE})"
        ));
    }
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|e| format!("frame body read failed: {e}"))?;
    Ok(Some(buf))
}

/// Write a little-endian u32-prefixed JSON frame for a `Response`.
pub async fn write_frame<W>(writer: &mut W, response: &Response) -> WorkbenchResult<()>
where
    W: AsyncWriteExt + Unpin,
{
    let bytes = serde_json::to_vec(response)?;
    if bytes.len() > MAX_FRAME_SIZE {
        let err = protocol_error(
            ProtocolErrorCode::MessageTooLarge,
            format!("response of {} bytes exceeds MAX_FRAME_SIZE", bytes.len()),
        );
        let bytes = serde_json::to_vec(&err)?;
        let len = (bytes.len() as u32).to_le_bytes();
        writer.write_all(&len).await.map_err(WorkbenchError::from)?;
        writer
            .write_all(&bytes)
            .await
            .map_err(WorkbenchError::from)?;
        writer.flush().await.map_err(WorkbenchError::from)?;
        return Ok(());
    }
    let len = (bytes.len() as u32).to_le_bytes();
    writer.write_all(&len).await.map_err(WorkbenchError::from)?;
    writer
        .write_all(&bytes)
        .await
        .map_err(WorkbenchError::from)?;
    writer.flush().await.map_err(WorkbenchError::from)?;
    Ok(())
}

fn protocol_error_response(message: impl Into<String>) -> Response {
    protocol_error(ProtocolErrorCode::ProtocolError, message)
}

#[cfg(unix)]
fn chmod_0600(path: &Path) -> WorkbenchResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms).map_err(WorkbenchError::from)
}

#[cfg(not(unix))]
fn chmod_0600(_path: &Path) -> WorkbenchResult<()> {
    // No-op on non-unix; socket permissions are handled via ACLs elsewhere.
    Ok(())
}

// Bridge WorkbenchError from the internal catalog over the wire. Used by
// daemon handlers in US1 that return `WorkbenchError` and let the server
// convert into the wire-visible `Response::Err`.
impl From<WorkbenchError> for Response {
    fn from(err: WorkbenchError) -> Self {
        Response::Err {
            code: err.code.to_protocol(),
            message: err.message,
            details: err.details,
        }
    }
}
