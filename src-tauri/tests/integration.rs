//! Phase 4 (US2 Alerts) integration tests.
//!
//! These tests validate alert lifecycle, quiet-hours suppression, heuristic
//! prompt detection, and the SC-011 false-positive budget gate.
//!
//! Written RED-first (TDD gate) — they will turn GREEN as Phase 4 tasks
//! land the `HeuristicDetector`, `PreferenceService`, and full alert
//! dispatch logic.

mod common;

mod integration {
    pub mod alert_autoclear;
    pub mod heuristic_false_positives;
    pub mod needs_input_heuristic;
    pub mod quiet_hours;
}
