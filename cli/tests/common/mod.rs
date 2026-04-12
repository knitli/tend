//! Shared test helpers for the CLI crate.
//!
//! T028: a mock workbench-side IPC server that accepts a single connection
//! and replies to `hello` with `welcome`. Used by CLI happy-path tests in
//! US1 (T058) and by the daemon-IPC contract tests that exercise the
//! client-side framing.
//!
//! **Per-binary note**: this module lives at `cli/tests/common/mod.rs` and
//! is shared via `mod common;` in each test file. See the src-tauri version
//! for the same convention note.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Return a unique unix-domain socket path.
pub fn temp_socket_path() -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("agentui-cli-test-{pid}-{seq}.sock"))
}
