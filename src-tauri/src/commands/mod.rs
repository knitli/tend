//! Tauri command handlers.
//!
//! Each submodule exposes `#[tauri::command]` functions that are registered
//! in `lib.rs` via `tauri::generate_handler![]`.

pub mod companions;
pub mod events;
pub mod notifications;
pub mod projects;
pub mod scratchpad;
pub mod sessions;
