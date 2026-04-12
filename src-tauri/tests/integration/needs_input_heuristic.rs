//! T069: Integration test — heuristic needs_input detection.
//!
//! The `HeuristicDetector` watches PTY output for prompt patterns that suggest
//! the session is blocked waiting for user input. Two-tier system:
//!
//! - **Tier 1 (cooperative)**: Once IPC status seen, heuristic permanently muted.
//! - **Tier 2 (heuristic)**: Pattern matching (`[y/N]`, `password:`, etc.).

use agentui_workbench::session::heuristic::{HeuristicDetector, HeuristicResult};
use std::time::{Duration, Instant};

/// Password prompt followed by silence triggers NeedsInput.
#[test]
fn pty_prompt_pattern_flags_needs_input() {
    let mut detector = HeuristicDetector::new();
    detector.feed(b"Connecting to server...\r\nEnter password:");
    detector.last_output_at = Some(Instant::now() - Duration::from_millis(1500));
    assert_eq!(detector.check(), HeuristicResult::NeedsInput);
}

/// `[y/N]` prompt triggers NeedsInput.
#[test]
fn y_n_prompt_flags_needs_input() {
    let mut detector = HeuristicDetector::new();
    detector.feed(b"Overwrite existing file? [y/N] ");
    detector.last_output_at = Some(Instant::now() - Duration::from_millis(1500));
    assert_eq!(detector.check(), HeuristicResult::NeedsInput);
}

/// `passphrase:` prompt triggers NeedsInput.
#[test]
fn passphrase_prompt_flags_needs_input() {
    let mut detector = HeuristicDetector::new();
    detector.feed(b"Enter passphrase for key '/home/user/.ssh/id_rsa':");
    detector.last_output_at = Some(Instant::now() - Duration::from_millis(1500));
    assert_eq!(detector.check(), HeuristicResult::NeedsInput);
}

/// Cooperative IPC permanently mutes heuristic (Tier 1 monotonicity).
#[test]
fn cooperative_ipc_mutes_heuristic() {
    let mut detector = HeuristicDetector::new();
    detector.cooperative_seen = true;
    detector.feed(b"Connecting to server...\r\nEnter password:");
    detector.last_output_at = Some(Instant::now() - Duration::from_millis(1500));
    assert_eq!(detector.check(), HeuristicResult::NoMatch);
}

/// Normal output ending with period does NOT trigger.
#[test]
fn normal_output_with_trailing_newline_no_trigger() {
    let mut detector = HeuristicDetector::new();
    detector.feed(b"Building project...\r\nAll checks passed.\r\n");
    detector.last_output_at = Some(Instant::now() - Duration::from_secs(2));
    assert_eq!(detector.check(), HeuristicResult::NoMatch);
}

/// Continuous output overwriting a prompt-like chunk prevents detection.
#[test]
fn continuous_output_prevents_idle_detection() {
    let mut detector = HeuristicDetector::new();
    detector.feed(b"Compiling crate 1/10...\r\n");
    detector.feed(b"Enter password: ");
    detector.feed(b"(from cache)\r\nCompiling crate 4/10...\r\n");
    detector.last_output_at = Some(Instant::now() - Duration::from_secs(2));
    assert_eq!(detector.check(), HeuristicResult::NoMatch);
}

/// ANSI escape sequences must be stripped before pattern matching.
#[test]
fn ansi_wrapped_prompt_triggers() {
    let mut detector = HeuristicDetector::new();
    detector.feed(b"\x1b[1m\x1b[33mEnter password:\x1b[0m");
    detector.last_output_at = Some(Instant::now() - Duration::from_millis(1500));
    assert_eq!(detector.check(), HeuristicResult::NeedsInput);
}
