//! Tauri command handlers.
//!
//! Each submodule exposes `#[tauri::command]` functions that are registered
//! in `lib.rs` via `tauri::generate_handler![]`.

pub mod events;
pub mod projects;
pub mod sessions;
