//! T056: CLI-side PTY wrapper using `portable-pty`.
//!
//! Spawns a child process inside a pseudo-terminal and provides
//! bidirectional I/O proxying between the user's real tty and the
//! child PTY.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result, bail};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Read from a raw fd, retrying on EINTR. Returns 0 on EOF.
fn read_fd(fd: i32, buf: &mut [u8]) -> std::io::Result<usize> {
    loop {
        // SAFETY: read(2) with a valid fd, a writable buffer, and a length
        // that fits in the buffer. EINTR is retried.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

/// Write all bytes to a raw fd, retrying on EINTR and short writes.
fn write_all_fd(fd: i32, mut buf: &[u8]) -> std::io::Result<()> {
    while !buf.is_empty() {
        // SAFETY: write(2) with a valid fd, a readable buffer, and a length
        // that fits in the buffer. EINTR is retried; short writes are looped.
        let n = unsafe { libc::write(fd, buf.as_ptr().cast(), buf.len()) };
        if n >= 0 {
            buf = &buf[n as usize..];
            continue;
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
    Ok(())
}

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

/// RAII guard that restores cooked terminal mode on drop (including panics).
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self> {
        crossterm::terminal::enable_raw_mode().context("failed to enable raw mode")?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Proxy I/O bidirectionally between `stdin`/`stdout` and the PTY master.
///
/// Blocks until the child process exits. Returns the child exit code.
///
/// This function takes ownership of the `PtyChild` and puts the user's
/// terminal into raw mode for the duration.
pub fn spawn_proxy(mut pty_child: PtyChild) -> Result<i32> {
    // H3: Enable raw mode so the child PTY receives every keystroke
    // (including Ctrl+C) without the parent shell intercepting them.
    let _raw_guard = RawModeGuard::enable()?;

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
    // Use direct write(2) on STDOUT_FILENO to avoid Rust's stdout buffering
    // and locking, which can interact badly with raw-mode terminals.
    let reader_handle = {
        let reader = Arc::clone(&master_reader);
        thread::spawn(move || {
            use std::io::Read as _;
            let mut buf = [0u8; 4096];
            loop {
                let mut r = reader.lock().unwrap();
                match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        drop(r);
                        if write_all_fd(libc::STDOUT_FILENO, &buf[..n]).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    };

    // stdin → PTY
    // Use direct read(2) on STDIN_FILENO instead of std::io::stdin().lock().
    // The latter goes through a buffered StdinRaw + a process-global mutex
    // which does not play well with raw-mode terminals: keystrokes can sit
    // in the BufReader's internal buffer or block indefinitely behind the
    // global stdin lock when other Rust I/O is active.
    let writer_handle = {
        let writer = Arc::clone(&master_writer);
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match read_fd(libc::STDIN_FILENO, &mut buf) {
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
///
/// M9: Return the actual exit code instead of collapsing everything to 0/1.
fn exit_status_to_code(status: &portable_pty::ExitStatus) -> i32 {
    status.exit_code() as i32
}
