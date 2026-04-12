//! Contract tests — Phases 3–5.
//!
//! Each submodule exercises one Tauri command or daemon IPC verb against
//! `mock_state()` / `temp_db()`. These tests are written RED-first (TDD gate)
//! and will turn GREEN as the corresponding tasks implement the real service
//! methods.

mod common;

mod contract {
    // Phase 3 (US1)
    pub mod alerts;
    pub mod daemon;
    pub mod notification_preferences;
    pub mod project_archive;
    pub mod project_list;
    pub mod project_register;
    pub mod project_update;
    pub mod session_list;
    pub mod session_spawn;

    // Phase 5 (US3)
    pub mod companion;
    pub mod session_activate;
    pub mod session_io;

    // Phase 6 (US5 Scratchpad)
    pub mod notes;
    pub mod overview;
    pub mod reminders;

    // Phase 7 (US6 Workspace + Layouts)
    pub mod layouts;
    pub mod workspace;

    // Phase 8 (US4 Activity Summary)
    pub mod session_summary;
}
