//! Unix-domain-socket IPC server module.
//!
//! T023: `spawn_daemon` is called from `lib::run` alongside the Tauri app.
//! It binds the socket, starts the accept loop, and returns the task handle
//! so the caller can await shutdown.

pub mod handlers;
pub mod server;

pub use server::{DaemonHandle, default_socket_path, spawn_daemon};
