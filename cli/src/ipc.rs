//! T055: IPC client for communicating with the tend workbench daemon.
//!
//! Uses `tend_protocol::{Request, Response, ...}` as the single source
//! of truth for wire types. Frame format: u32 LE length prefix + JSON body,
//! capped at [`MAX_FRAME_SIZE`].

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use tend_protocol::{Request, Response, SessionStatusWire, MAX_FRAME_SIZE, PROTOCOL_VERSION};

/// IPC client wrapping a Unix domain socket connection.
pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    // -- Connection --------------------------------------------------------

    /// Connect to the workbench daemon socket.
    ///
    /// Resolution order for the socket path:
    /// 1. `path` argument (if `Some`)
    /// 2. `$AGENTUI_SOCKET` environment variable
    /// 3. `$XDG_RUNTIME_DIR/tend.sock`
    /// 4. `/tmp/tend-$UID.sock`
    pub async fn connect(path: Option<PathBuf>) -> Result<Self> {
        let sock = match path {
            Some(p) => p,
            None => resolve_socket_path()?,
        };
        let stream = UnixStream::connect(&sock)
            .await
            .with_context(|| format!("failed to connect to daemon at {}", sock.display()))?;
        Ok(Self { stream })
    }

    // -- Low-level framing -------------------------------------------------

    /// Send a `Request` and read back a `Response`.
    pub async fn send_request(&mut self, req: &Request) -> Result<Response> {
        self.write_frame(req).await?;
        self.read_frame().await
    }

    /// Serialize and write a length-prefixed JSON frame.
    async fn write_frame(&mut self, req: &Request) -> Result<()> {
        let payload = serde_json::to_vec(req).context("failed to serialize request")?;
        if payload.len() > MAX_FRAME_SIZE {
            bail!(
                "frame too large ({} bytes, max {})",
                payload.len(),
                MAX_FRAME_SIZE
            );
        }
        let len = payload.len() as u32;
        self.stream
            .write_all(&len.to_le_bytes())
            .await
            .context("failed to write frame length")?;
        self.stream
            .write_all(&payload)
            .await
            .context("failed to write frame body")?;
        self.stream.flush().await.context("failed to flush")?;
        Ok(())
    }

    /// Read a length-prefixed JSON frame and deserialize the `Response`.
    async fn read_frame(&mut self) -> Result<Response> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .context("failed to read frame length")?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_FRAME_SIZE {
            bail!(
                "server frame too large ({} bytes, max {})",
                len,
                MAX_FRAME_SIZE
            );
        }
        let mut body = vec![0u8; len];
        self.stream
            .read_exact(&mut body)
            .await
            .context("failed to read frame body")?;
        serde_json::from_slice(&body).context("failed to deserialize response")
    }

    // -- Convenience helpers -----------------------------------------------

    /// Perform the initial `Hello` handshake and expect `Welcome`.
    pub async fn hello(&mut self) -> Result<Response> {
        let req = Request::Hello {
            client: "tend-run".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: PROTOCOL_VERSION,
        };
        let resp = self.send_request(&req).await?;
        match &resp {
            Response::Welcome { .. } => Ok(resp),
            Response::Err { code, message, .. } => {
                bail!("hello rejected: {code} - {message}")
            }
            other => bail!("unexpected response to hello: {other:?}"),
        }
    }

    /// Register a new session and return `(session_id, project_id)`.
    pub async fn register_session(
        &mut self,
        project_path: &str,
        label: Option<String>,
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        pid: i32,
    ) -> Result<(i64, i64)> {
        let req = Request::RegisterSession {
            project_path: project_path.into(),
            label,
            working_directory,
            command,
            pid,
            metadata: None,
        };
        let resp = self.send_request(&req).await?;
        match resp {
            Response::SessionRegistered {
                session_id,
                project_id,
            } => Ok((session_id, project_id)),
            Response::Err { code, message, .. } => {
                bail!("register_session failed: {code} - {message}")
            }
            other => bail!("unexpected response to register_session: {other:?}"),
        }
    }

    /// Send a heartbeat for the given session.
    pub async fn heartbeat(&mut self, session_id: i64) -> Result<()> {
        let req = Request::Heartbeat { session_id };
        let resp = self.send_request(&req).await?;
        expect_ack("heartbeat", &resp)
    }

    /// Push a status update for the given session.
    #[allow(dead_code)] // Will be used in US2 when cooperative status is wired up.
    pub async fn update_status(
        &mut self,
        session_id: i64,
        status: SessionStatusWire,
        reason: Option<String>,
    ) -> Result<()> {
        let req = Request::UpdateStatus {
            session_id,
            status,
            reason,
            summary: None,
        };
        let resp = self.send_request(&req).await?;
        expect_ack("update_status", &resp)
    }

    /// Notify the workbench that the session has ended.
    pub async fn end_session(&mut self, session_id: i64, exit_code: Option<i32>) -> Result<()> {
        let req = Request::EndSession {
            session_id,
            exit_code,
        };
        let resp = self.send_request(&req).await?;
        expect_ack("end_session", &resp)
    }
}

// -- Helpers ---------------------------------------------------------------

/// Check an `Ack` response, returning an error for anything else.
fn expect_ack(verb: &str, resp: &Response) -> Result<()> {
    match resp {
        Response::Ack => Ok(()),
        Response::Err { code, message, .. } => bail!("{verb} failed: {code} - {message}"),
        other => bail!("unexpected response to {verb}: {other:?}"),
    }
}

/// Resolve the default daemon socket path.
///
/// 1. `$AGENTUI_SOCKET`
/// 2. `$XDG_RUNTIME_DIR/tend.sock`
/// 3. `/tmp/tend-$UID.sock`
fn resolve_socket_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("AGENTUI_SOCKET") {
        return Ok(PathBuf::from(p));
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(dir).join("tend.sock"));
    }
    // Fallback: /tmp/tend-$UID.sock
    // SAFETY: getuid(2) has no preconditions, takes no arguments, and cannot fail.
    let uid = unsafe { libc::getuid() };
    Ok(PathBuf::from(format!("/tmp/tend-{uid}.sock")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_socket_from_env() {
        // Temporarily set AGENTUI_SOCKET and verify it takes precedence.
        // SAFETY: Test runs in a single thread (#[test] is not #[tokio::test]),
        // so no concurrent env reads can race.
        let original = std::env::var("AGENTUI_SOCKET").ok();
        unsafe { std::env::set_var("AGENTUI_SOCKET", "/run/custom.sock") };
        let path = resolve_socket_path().unwrap();
        assert_eq!(path, PathBuf::from("/run/custom.sock"));

        // Restore.
        match original {
            Some(v) => unsafe { std::env::set_var("AGENTUI_SOCKET", v) },
            None => unsafe { std::env::remove_var("AGENTUI_SOCKET") },
        }
    }
}
