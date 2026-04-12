//! T133: Integration test — `update_status { summary: "Refactoring lexer" }`
//! overrides heuristic-derived summary until the next `update_status` or
//! until output activity resumes past a timeout.

use agentui_workbench::session::activity::ActivitySummary;

/// Agent-provided summary overrides the heuristic-derived summary.
#[test]
fn override_takes_precedence_over_ring_buffer() {
    let mut activity = ActivitySummary::new();

    // Feed some output.
    activity.record_chunk(b"Compiling src/main.rs\n");
    assert_eq!(
        activity.current(),
        Some("Compiling src/main.rs".to_string())
    );

    // Agent overrides with a task summary.
    activity.override_with("Refactoring lexer");
    assert_eq!(activity.current(), Some("Refactoring lexer".to_string()));
}

/// A new `update_status` with summary replaces the previous override.
#[test]
fn new_override_replaces_old() {
    let mut activity = ActivitySummary::new();

    activity.override_with("Refactoring lexer");
    assert_eq!(activity.current(), Some("Refactoring lexer".to_string()));

    activity.override_with("Running tests");
    assert_eq!(activity.current(), Some("Running tests".to_string()));
}

/// An empty summary string clears the override.
#[test]
fn empty_override_clears_back_to_heuristic() {
    let mut activity = ActivitySummary::new();

    activity.record_chunk(b"heuristic line\n");
    activity.override_with("agent summary");
    assert_eq!(activity.current(), Some("agent summary".to_string()));

    // Empty string clears the override.
    activity.override_with("");
    assert_eq!(activity.current(), Some("heuristic line".to_string()));
}

/// While override is fresh and no output has been produced, it persists.
#[test]
fn override_persists_when_idle() {
    let mut activity = ActivitySummary::new();

    activity.override_with("Waiting for CI");

    // No output activity — override should still be active.
    assert_eq!(activity.current(), Some("Waiting for CI".to_string()));
}

/// Override is truncated to the max summary length.
#[test]
fn override_truncated_if_too_long() {
    let mut activity = ActivitySummary::new();

    let long = "a".repeat(200);
    activity.override_with(&long);
    let result = activity.current().unwrap();
    assert!(result.chars().count() <= 80);
    assert!(result.ends_with('…'));
}

/// Override survives additional output (until timeout).
#[test]
fn override_survives_additional_output_briefly() {
    let mut activity = ActivitySummary::new();

    activity.override_with("Agent task title");

    // Feed some output shortly after — override should still win because the
    // activity timeout (10 s) hasn't elapsed.
    activity.record_chunk(b"some output\n");

    // Override should still be active (no 10 s has elapsed).
    assert_eq!(activity.current(), Some("Agent task title".to_string()));
}
