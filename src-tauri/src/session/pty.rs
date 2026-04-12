//! PTY abstraction built on `portable-pty`.
//!
//! T020: wraps `native_pty_system()` to give the rest of the backend a small,
//! test-friendly surface: spawn a child under a PTY, read/write bytes, resize,
//! query pid, await exit. The raw `Box<dyn MasterPty>` from portable-pty is
//! hidden behind this type.
//!
//! Threading model: portable-pty's reader/writer are blocking `std::io`
//! traits. We bridge them into async by spawning a dedicated std thread for
//! reading and streaming bytes into an unbounded tokio `mpsc` channel. The
//! writer side is small enough that we call it from a blocking task when
//! needed. This is deliberately simple — we are never running hundreds of
//! sessions at once.

use crate::error::{ErrorCode, WorkbenchError, WorkbenchResult};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// A running PTY-hosted child process.
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pid: Option<u32>,
}

/// Chunks of output coming off the PTY's master side.
pub type OutputRx = mpsc::UnboundedReceiver<Vec<u8>>;

impl Pty {
    /// Spawn `command` inside a fresh PTY, in `cwd`, with the given extra env.
    ///
    /// Returns the [`Pty`] handle and an async receiver that yields output
    /// chunks as the child produces them.
    pub fn spawn(
        command: &[String],
        cwd: &Path,
        env: &BTreeMap<String, String>,
        size: PtySize,
    ) -> WorkbenchResult<(Self, OutputRx)> {
        if command.is_empty() {
            return Err(WorkbenchError::new(
                ErrorCode::SpawnFailed,
                "empty command array",
            ));
        }

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(size).map_err(|e| {
            WorkbenchError::with_details(
                ErrorCode::SpawnFailed,
                format!("openpty failed: {e}"),
                serde_json::json!({ "stage": "openpty" }),
            )
        })?;

        let mut builder = CommandBuilder::new(&command[0]);
        if command.len() > 1 {
            builder.args(&command[1..]);
        }
        builder.cwd(cwd);
        for (k, v) in env.iter() {
            builder.env(k, v);
        }

        let child = pair.slave.spawn_command(builder).map_err(|e| {
            WorkbenchError::with_details(
                ErrorCode::SpawnFailed,
                format!("spawn_command failed: {e}"),
                serde_json::json!({ "stage": "spawn_command", "command": command }),
            )
        })?;

        let pid = child.process_id();
        // portable-pty returns the slave's owning handle; drop it once the
        // child has inherited it so the master-only end reflects EOF when the
        // child exits.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| {
            WorkbenchError::new(
                ErrorCode::SpawnFailed,
                format!("failed to clone pty reader: {e}"),
            )
        })?;
        let writer = pair.master.take_writer().map_err(|e| {
            WorkbenchError::new(
                ErrorCode::SpawnFailed,
                format!("failed to take pty writer: {e}"),
            )
        })?;

        // Bridge the blocking reader into an async channel.
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break; // receiver dropped
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });

        Ok((
            Self {
                master: pair.master,
                child: Arc::new(Mutex::new(child)),
                writer: Arc::new(Mutex::new(writer)),
                pid,
            },
            rx,
        ))
    }

    /// Pid of the child process (if known).
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Resize the PTY's window.
    pub fn resize(&self, cols: u16, rows: u16) -> WorkbenchResult<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| {
                WorkbenchError::with_details(
                    ErrorCode::WriteFailed,
                    format!("pty resize failed: {e}"),
                    serde_json::json!({ "cols": cols, "rows": rows }),
                )
            })?;
        Ok(())
    }

    /// Write bytes to the PTY master (→ child stdin).
    pub fn write_bytes(&self, bytes: &[u8]) -> WorkbenchResult<()> {
        let mut w = self.writer.lock().map_err(|_| {
            WorkbenchError::new(ErrorCode::WriteFailed, "pty writer mutex poisoned")
        })?;
        w.write_all(bytes).map_err(WorkbenchError::from)?;
        w.flush().map_err(WorkbenchError::from)?;
        Ok(())
    }

    /// Send a SIGTERM (or equivalent) to the child process.
    pub fn kill(&self) -> WorkbenchResult<()> {
        let mut c = self
            .child
            .lock()
            .map_err(|_| WorkbenchError::new(ErrorCode::Internal, "pty child mutex poisoned"))?;
        c.kill().map_err(WorkbenchError::from)?;
        Ok(())
    }

    /// Block until the child exits and return its raw wait status.
    pub fn wait(&self) -> WorkbenchResult<i32> {
        let mut c = self
            .child
            .lock()
            .map_err(|_| WorkbenchError::new(ErrorCode::Internal, "pty child mutex poisoned"))?;
        let status = c.wait().map_err(WorkbenchError::from)?;
        Ok(status.exit_code() as i32)
    }

    /// Create a wait-only handle that can be sent to a separate thread.
    /// The returned handle shares the child Arc so `wait()` on either blocks
    /// until the child exits.
    pub fn clone_for_wait(&self) -> PtyWaitHandle {
        PtyWaitHandle {
            child: Arc::clone(&self.child),
        }
    }
}

/// A handle that can only wait for the child to exit.
/// Safe to send to a separate thread for exit detection.
pub struct PtyWaitHandle {
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

impl PtyWaitHandle {
    /// Block until the child exits and return its exit code.
    pub fn wait(&self) -> WorkbenchResult<i32> {
        let mut c = self
            .child
            .lock()
            .map_err(|_| WorkbenchError::new(ErrorCode::Internal, "pty child mutex poisoned"))?;
        let status = c.wait().map_err(WorkbenchError::from)?;
        Ok(status.exit_code() as i32)
    }
}

impl std::fmt::Debug for Pty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pty").field("pid", &self.pid).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Small helper: a shell that echoes a known string and exits.
    fn echo_cmd(msg: &str) -> Vec<String> {
        if cfg!(windows) {
            vec!["cmd".into(), "/C".into(), format!("echo {msg}")]
        } else {
            vec!["/bin/sh".into(), "-c".into(), format!("echo {msg}")]
        }
    }

    #[test]
    fn spawn_echo_and_read() {
        let cwd = std::env::temp_dir();
        let env: BTreeMap<String, String> = BTreeMap::new();
        let (pty, mut rx) = Pty::spawn(
            &echo_cmd("hello-pty"),
            &cwd,
            &env,
            PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        )
        .expect("spawn");

        assert!(pty.pid().is_some());

        // Drain until the reader thread sees EOF.
        let mut buf = Vec::new();
        // Use blocking_recv in a short-lived std thread so we don't need a tokio runtime here.
        let drain = std::thread::spawn(move || {
            while let Some(chunk) = rx.blocking_recv() {
                buf.extend_from_slice(&chunk);
            }
            buf
        });

        let code = pty.wait().expect("wait");
        drop(pty); // closes the master, reader thread sees EOF
        let output = drain.join().expect("drain thread join");

        // On Unix `echo` exits 0; on Windows `cmd /C` also exits 0.
        assert_eq!(code, 0, "child exit code");
        let as_str = String::from_utf8_lossy(&output);
        assert!(
            as_str.contains("hello-pty"),
            "expected echo output, got: {as_str:?}"
        );
    }

    #[test]
    fn spawn_empty_command_rejected() {
        let err = Pty::spawn(
            &[],
            &std::env::temp_dir(),
            &BTreeMap::new(),
            PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        )
        .expect_err("should reject empty command");
        assert_eq!(err.code, ErrorCode::SpawnFailed);
    }
}
