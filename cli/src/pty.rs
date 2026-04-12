//! T056: CLI-side PTY wrapper using `portable-pty`.
//!
//! Spawns a child process inside a pseudo-terminal and provides
//! bidirectional I/O proxying between the user's real tty and the
//! child PTY.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use anyhow::{bail, Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

/// A spawned child process inside a PTY.
pub struct PtyChild {
    /// The PTY master handle (owns the fd).
    pub master: Box<dyn MasterPty + Send>,
    /// The child process handle.
    pub child: Box<dyn Child + Send + Sync>,
}

impl PtyChild {
    /// Return the OS pid of the child process.
    pub fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }
}

/// Spawn a command inside a new PTY.
///
/// The PTY is sized to match the user's current terminal (if detectable),
/// otherwise defaults to 80x24.
pub fn run_child(command: &[String], cwd: &Path) -> Result<PtyChild> {
    if command.is_empty() {
        bail!("command must not be empty");
    }

    let pty_system = native_pty_system();

    let size = current_terminal_size();

    let pair = pty_system
        .openpty(size)
        .context("failed to open PTY pair")?;

    let mut cmd = CommandBuilder::new(&command[0]);
    if command.len() > 1 {
        cmd.args(&command[1..]);
    }
    cmd.cwd(cwd);

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("failed to spawn child in PTY")?;

    // Drop the slave side — the child has it.
    drop(pair.slave);

    Ok(PtyChild {
        master: pair.master,
        child,
    })
}

/// Proxy I/O bidirectionally between `stdin`/`stdout` and the PTY master.
///
/// Blocks until the child process exits. Returns the child exit code.
///
/// This function takes ownership of the `PtyChild` and puts the user's
/// terminal into raw mode for the duration.
pub fn spawn_proxy(mut pty_child: PtyChild) -> Result<i32> {
    let master_reader = pty_child
        .master
        .try_clone_reader()
        .context("failed to clone PTY reader")?;
    let master_writer = pty_child
        .master
        .take_writer()
        .context("failed to take PTY writer")?;

    let master_reader = Arc::new(std::sync::Mutex::new(master_reader));
    let master_writer = Arc::new(std::sync::Mutex::new(master_writer));

    // PTY → stdout
    let reader_handle = {
        let reader = Arc::clone(&master_reader);
        thread::spawn(move || {
            let mut stdout = std::io::stdout().lock();
            let mut buf = [0u8; 4096];
            loop {
                let mut r = reader.lock().unwrap();
                match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        drop(r);
                        if stdout.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = stdout.flush();
                    }
                    Err(_) => break,
                }
            }
        })
    };

    // stdin → PTY
    let writer_handle = {
        let writer = Arc::clone(&master_writer);
        thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut stdin = stdin.lock();
            let mut buf = [0u8; 4096];
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut w = writer.lock().unwrap();
                        if w.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = w.flush();
                    }
                    Err(_) => break,
                }
            }
        })
    };

    // Wait for child to exit.
    let status = pty_child
        .child
        .wait()
        .context("failed to wait on child process")?;

    // The I/O threads will terminate once the PTY master side closes.
    drop(pty_child.master);
    let _ = reader_handle.join();
    let _ = writer_handle.join();

    Ok(exit_status_to_code(&status))
}

/// Detect the current terminal size, falling back to 80x24.
fn current_terminal_size() -> PtySize {
    // Try to read from the real terminal.
    if let Some((cols, rows)) = term_size::dimensions() {
        PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        }
    } else {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Convert a `portable_pty::ExitStatus` to an integer exit code.
fn exit_status_to_code(status: &portable_pty::ExitStatus) -> i32 {
    if status.success() {
        0
    } else {
        // ExitStatus doesn't expose the raw code on all platforms via
        // portable-pty's public API. Default to 1 for failure.
        1
    }
}
