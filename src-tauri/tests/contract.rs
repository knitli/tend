//! US1 contract tests — Phase 3.
//!
//! Each submodule exercises one Tauri command or daemon IPC verb against
//! `mock_state()` / `temp_db()`. These tests are written RED-first (TDD gate)
//! and will turn GREEN as T049–T052 implement the real service methods.

mod common;

mod contract {
    pub mod alerts;
    pub mod daemon;
    pub mod notification_preferences;
    pub mod project_archive;
    pub mod project_list;
    pub mod project_register;
    pub mod project_update;
    pub mod session_list;
    pub mod session_spawn;
}
