//! Integration tests — Phases 4–5.
//!
//! Phase 4 (US2 Alerts): alert lifecycle, quiet-hours suppression, heuristic
//! prompt detection, and the SC-011 false-positive budget gate.
//!
//! Phase 5 (US3 Activation): companion worktree handling, idempotency.
//!
//! Written RED-first (TDD gate) — they will turn GREEN as the corresponding
//! tasks land the implementations.

mod common;

mod integration {
    // Phase 4 (US2)
    pub mod alert_autoclear;
    pub mod heuristic_false_positives;
    pub mod needs_input_heuristic;
    pub mod quiet_hours;

    // Phase 5 (US3)
    pub mod companion_idempotency;
    pub mod companion_worktree;
}
