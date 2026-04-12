//! T073b: Adversarial PTY corpus — false positive budget gate (SC-011).
//!
//! SC-011 requires <= 1 spurious `needs_input` alert per 100 sessions.
//!
//! This test loads five adversarial fixture files from `tests/fixtures/ptyoutput/`
//! and feeds each through a fresh `HeuristicDetector`. Each fixture simulates
//! 20 sessions (5 fixtures x 20 = 100 sessions total). The total number of
//! `needs_input` triggers across all fixtures must be <= 1.
//!
//! The fixtures are designed to contain prompt-like patterns that should NOT
//! trigger detection:
//! - `diff_with_y_N.txt`: unified diff with `[y/N]` in diff context lines
//! - `code_with_password_literal.txt`: source code with `password:` as identifiers
//! - `agent_narration_with_gt.txt`: agent narration with `>` line prefixes + ANSI
//! - `readme_render.txt`: README-like content with question marks and `password:`
//! - `mixed_ansi_prompts.txt`: log output with embedded prompt substrings
//!
//! RED state: these tests access `last_output_at` which is currently private
//! on `HeuristicDetector`. Phase 4 implementation must make this field `pub`
//! (or add a `pub check_after_silence(Duration)` method) to support
//! integration testing without real sleep delays.

use agentui_workbench::session::heuristic::{HeuristicDetector, HeuristicResult};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Locate the fixtures directory relative to the workspace root.
fn fixtures_dir() -> PathBuf {
    // Integration tests run from the workspace root or src-tauri/.
    // Try both possible locations.
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/ptyoutput"),
        PathBuf::from("tests/fixtures/ptyoutput"),
        PathBuf::from("../tests/fixtures/ptyoutput"),
    ];

    for candidate in &candidates {
        if candidate.is_dir() {
            return candidate.clone();
        }
    }

    panic!(
        "Could not find tests/fixtures/ptyoutput/ directory. \
         Searched: {candidates:?}"
    );
}

/// Feed a fixture through a HeuristicDetector in small chunks (simulating
/// realistic PTY output arrival) and return the number of NeedsInput triggers.
fn count_triggers_for_fixture(content: &[u8]) -> usize {
    let mut triggers = 0;
    let chunk_size = 128;
    let mut detector = HeuristicDetector::new();

    for chunk in content.chunks(chunk_size) {
        detector.feed(chunk);
        // Mid-stream: no silence, so check should not trigger.
        // (feed() resets the output timestamp, so check() sees zero silence.)
    }

    // After all content is delivered, simulate 2 seconds of silence to see
    // if the trailing content triggers a false positive.
    detector.last_output_at = Some(Instant::now() - Duration::from_secs(2));
    if detector.check() == HeuristicResult::NeedsInput {
        triggers += 1;
    }

    triggers
}

/// SC-011 gate: total false-positive triggers across the adversarial corpus
/// must be <= 1 per 100 simulated sessions.
///
/// We simulate 20 sessions per fixture (5 fixtures x 20 = 100 sessions).
#[test]
fn sc011_false_positive_budget() {
    let dir = fixtures_dir();

    let fixture_files = [
        "diff_with_y_N.txt",
        "code_with_password_literal.txt",
        "agent_narration_with_gt.txt",
        "readme_render.txt",
        "mixed_ansi_prompts.txt",
    ];

    let mut total_triggers: usize = 0;
    let sessions_per_fixture: usize = 20;

    for filename in &fixture_files {
        let path = dir.join(filename);
        let content = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("Failed to read fixture {}: {e}", path.display());
        });

        let mut fixture_triggers = 0;
        for _ in 0..sessions_per_fixture {
            fixture_triggers += count_triggers_for_fixture(&content);
        }

        if fixture_triggers > 0 {
            eprintln!(
                "SC-011: {filename} produced {fixture_triggers} triggers across {sessions_per_fixture} sessions"
            );
        }

        total_triggers += fixture_triggers;
    }

    let total_sessions = fixture_files.len() * sessions_per_fixture;
    eprintln!(
        "SC-011 summary: {total_triggers} total triggers across {total_sessions} simulated sessions"
    );

    assert!(
        total_triggers <= 1,
        "SC-011 FAILED: {total_triggers} false-positive triggers across {total_sessions} sessions \
         (budget: <= 1). The heuristic detector needs tuning to reduce false positives."
    );
}

/// Verify that each individual fixture produces zero triggers on its own
/// (stricter per-fixture gate).
#[test]
fn individual_fixtures_produce_zero_triggers() {
    let dir = fixtures_dir();

    let fixture_files = [
        "diff_with_y_N.txt",
        "code_with_password_literal.txt",
        "agent_narration_with_gt.txt",
        "readme_render.txt",
        "mixed_ansi_prompts.txt",
    ];

    for filename in &fixture_files {
        let path = dir.join(filename);
        let content = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("Failed to read fixture {}: {e}", path.display());
        });

        let triggers = count_triggers_for_fixture(&content);
        assert_eq!(
            triggers, 0,
            "Fixture {filename} must produce 0 false-positive triggers, got {triggers}. \
             The heuristic detector must not fire on adversarial PTY output that merely \
             contains prompt-like substrings in non-prompt contexts."
        );
    }
}
