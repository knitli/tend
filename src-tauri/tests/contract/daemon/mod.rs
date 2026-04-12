//! Daemon IPC contract tests (T036–T039).
//!
//! Each submodule tests one daemon IPC verb by calling `dispatch(request, &state)`
//! directly. These tests are RED against the current stub/implementation — they
//! define the EXPECTED behavior per the spec.

pub mod hello;
pub mod lifecycle;
pub mod register_session;
pub mod update_status;
