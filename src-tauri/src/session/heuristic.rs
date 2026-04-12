//! T073: Heuristic prompt detector for Tier 2 `needs_input` detection.
//!
//! Maintains a rolling 256-byte buffer (ANSI-stripped) per session. Matches
//! against the pruned high-precision pattern set from `research.md §7`:
//!
//! High-precision triggers:
//!   - `[y/N]`, `[Y/n]`, `[yes/no]`, `(yes/no)` (case-insensitive)
//!   - `password:` or `passphrase:` at end of last non-empty line
//!   - Weak: last non-empty line ends in `?` or `:` with >= 1s silence
//!
//! Explicitly excluded (false-positive sources):
//!   - Bare `> ` at end of line
//!   - Standalone `continue?`
//!
//! Cooperative-IPC monotonicity: once a session has ever received an
//! `update_status` from the daemon socket, the heuristic is permanently
//! muted for that session's lifetime.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Minimum silence before a pattern match triggers `needs_input`.
const MIN_SILENCE: Duration = Duration::from_secs(1);

/// Ring buffer capacity for ANSI-stripped output.
const BUFFER_SIZE: usize = 256;

/// The heuristic prompt detector. One instance per live session.
#[derive(Debug)]
pub struct HeuristicDetector {
    /// Rolling ring buffer of ANSI-stripped output bytes.
    buffer: VecDeque<u8>,
    /// Timestamp of the last output chunk received.
    pub last_output_at: Option<Instant>,
    /// Whether this session has ever received a cooperative IPC status update.
    /// Once true, the heuristic is permanently muted.
    pub cooperative_seen: bool,
    /// Whether the detector has already triggered `needs_input` and is waiting
    /// for a status change to re-arm.
    triggered: bool,
}

/// Result of checking the heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeuristicResult {
    /// No prompt detected — do nothing.
    NoMatch,
    /// A prompt pattern was detected after sufficient silence.
    NeedsInput,
}

impl Default for HeuristicDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicDetector {
    /// Create a new detector for a session.
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(BUFFER_SIZE),
            last_output_at: None,
            cooperative_seen: false,
            triggered: false,
        }
    }

    /// Feed output bytes from the PTY. Strips ANSI escape sequences and
    /// appends to the rolling buffer.
    pub fn feed(&mut self, data: &[u8]) {
        let stripped = strip_ansi(data);
        for &b in &stripped {
            if self.buffer.len() >= BUFFER_SIZE {
                self.buffer.pop_front();
            }
            self.buffer.push_back(b);
        }
        self.last_output_at = Some(Instant::now());
        // New output means the session is active — re-arm the trigger.
        self.triggered = false;
    }

    /// Check whether the current buffer state indicates a prompt.
    ///
    /// Should be called periodically (e.g., every 500ms) by the status monitor.
    /// Returns `NeedsInput` only if:
    ///   1. `cooperative_seen` is false (Tier 1 monotonicity)
    ///   2. At least `MIN_SILENCE` has elapsed since the last output
    ///   3. The buffer tail matches a high-precision pattern
    ///   4. The detector hasn't already triggered (prevents re-fire)
    pub fn check(&mut self) -> HeuristicResult {
        // Cooperative-IPC mutes the heuristic permanently.
        if self.cooperative_seen {
            return HeuristicResult::NoMatch;
        }

        // Already triggered — wait for new output to re-arm.
        if self.triggered {
            return HeuristicResult::NoMatch;
        }

        // Must have some output and sufficient silence.
        let elapsed = match self.last_output_at {
            Some(t) => t.elapsed(),
            None => return HeuristicResult::NoMatch,
        };

        if elapsed < MIN_SILENCE {
            return HeuristicResult::NoMatch;
        }

        // Check patterns against the buffer.
        if self.matches_prompt_pattern() {
            self.triggered = true;
            HeuristicResult::NeedsInput
        } else {
            HeuristicResult::NoMatch
        }
    }

    /// Reset the triggered state (called when status changes away from NeedsInput).
    pub fn reset_trigger(&mut self) {
        self.triggered = false;
    }

    /// Check the buffer tail against the high-precision pattern set.
    fn matches_prompt_pattern(&self) -> bool {
        if self.buffer.is_empty() {
            return false;
        }

        let contiguous: Vec<u8> = self.buffer.iter().copied().collect();
        let text = String::from_utf8_lossy(&contiguous);
        let lower = text.to_lowercase();

        // High-precision bracket patterns (case-insensitive).
        if lower.contains("[y/n]") || lower.contains("[yes/no]") || lower.contains("(yes/no)") {
            // Exclude patterns that appear in diff context or code.
            // Only match if the pattern is near the end of the buffer
            // (within the last line or two).
            let last_lines = last_non_empty_lines(&text, 2);
            let last_lower = last_lines.to_lowercase();
            if last_lower.contains("[y/n]")
                || last_lower.contains("[yes/no]")
                || last_lower.contains("(yes/no)")
            {
                return true;
            }
        }

        // `password:` or `passphrase:` at end of last non-empty line.
        let last_line = last_non_empty_line(&text);
        let last_trimmed = last_line.trim_end().to_lowercase();
        if last_trimmed.ends_with("password:") || last_trimmed.ends_with("passphrase:") {
            // Exclude false positives: `password:` appearing as part of a code
            // variable/type annotation (preceded by a comma, open paren, or
            // function-like context). The real prompt has it at the very end
            // with nothing else after the colon.
            let line_stripped = last_line.trim();
            // Only trigger if "password:" or "passphrase:" is at the end and
            // the line looks like a prompt (starts with a letter or >, not code).
            if !looks_like_code(line_stripped) {
                return true;
            }
            // Even in code-like context, if the line is very short and ends
            // with password:/passphrase:, it's likely a prompt.
            if last_trimmed.len() <= 30 {
                return true;
            }
        }

        // Weak signal: last non-empty line ends in `?` or `:` (but not the
        // patterns above, and not excluded patterns).
        if last_trimmed.ends_with('?') || last_trimmed.ends_with(':') {
            // Exclude bare `> ` patterns.
            let line_stripped = last_line.trim();
            if line_stripped == ">" || line_stripped.is_empty() {
                return false;
            }
            // Exclude `continue?` alone.
            if last_trimmed == "continue?" {
                return false;
            }
            // Exclude lines that look like code.
            if looks_like_code(line_stripped) {
                return false;
            }
            // Only trigger the weak signal if there's been extended silence
            // (>= 1s is already checked by the caller, so this is just the
            // pattern match).
            return true;
        }

        false
    }
}

/// Get the last non-empty line from text.
fn last_non_empty_line(text: &str) -> &str {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
}

/// Get the last N non-empty lines from text, joined.
fn last_non_empty_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(n)
        .collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

/// Simple heuristic: does this line look like code rather than a prompt?
fn looks_like_code(line: &str) -> bool {
    // Code indicators: starts with def/fn/let/var/const, contains `=`, `(`,
    // or is indented, or has a type annotation pattern like `password: str`
    let trimmed = line.trim();
    if trimmed.starts_with("def ")
        || trimmed.starts_with("fn ")
        || trimmed.starts_with("let ")
        || trimmed.starts_with("var ")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("pub ")
        || trimmed.starts_with('#')
        || trimmed.starts_with("//")
    {
        return true;
    }
    // Type annotation pattern: `something: type` where type has no spaces
    // and the line has balanced parens/brackets.
    if trimmed.contains("password: ") || trimmed.contains("passphrase: ") {
        // Has a space after the colon — likely a type annotation, not a prompt.
        return true;
    }
    false
}

/// Strip ANSI escape sequences from a byte slice.
/// Handles CSI sequences (\x1b[...m, etc.) and OSC sequences (\x1b]...\x07).
fn strip_ansi(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        if data[i] == 0x1b {
            // ESC
            i += 1;
            if i < data.len() {
                match data[i] {
                    b'[' => {
                        // CSI sequence: skip until a letter in @-~ range.
                        i += 1;
                        while i < data.len() && !(0x40..=0x7e).contains(&data[i]) {
                            i += 1;
                        }
                        if i < data.len() {
                            i += 1; // skip the final byte
                        }
                    }
                    b']' => {
                        // OSC sequence: skip until BEL (0x07) or ST (ESC \).
                        i += 1;
                        while i < data.len() && data[i] != 0x07 {
                            if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                        if i < data.len() && data[i] == 0x07 {
                            i += 1;
                        }
                    }
                    _ => {
                        // Other escape — skip one more byte.
                        i += 1;
                    }
                }
            }
        } else {
            result.push(data[i]);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_csi() {
        let input = b"\x1b[32mhello\x1b[0m world";
        let result = strip_ansi(input);
        assert_eq!(result, b"hello world");
    }

    #[test]
    fn strip_ansi_removes_osc() {
        let input = b"\x1b]0;title\x07text";
        let result = strip_ansi(input);
        assert_eq!(result, b"text");
    }

    #[test]
    fn detector_triggers_on_y_n_pattern() {
        let mut d = HeuristicDetector::new();
        d.feed(b"Overwrite file? [y/N] ");
        // Simulate silence by backdating last_output_at.
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
    }

    #[test]
    fn detector_triggers_on_password_prompt() {
        let mut d = HeuristicDetector::new();
        d.feed(b"Enter password:");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
    }

    #[test]
    fn detector_does_not_trigger_on_code_password() {
        let mut d = HeuristicDetector::new();
        d.feed(b"def validate(password: str) -> bool:\n");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NoMatch);
    }

    #[test]
    fn cooperative_mutes_heuristic() {
        let mut d = HeuristicDetector::new();
        d.cooperative_seen = true;
        d.feed(b"Enter password:");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NoMatch);
    }

    #[test]
    fn detector_does_not_trigger_on_bare_gt() {
        let mut d = HeuristicDetector::new();
        d.feed(b"> ");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NoMatch);
    }

    #[test]
    fn detector_does_not_trigger_on_standalone_continue() {
        let mut d = HeuristicDetector::new();
        d.feed(b"continue?");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NoMatch);
    }

    #[test]
    fn detector_does_not_retrigger_without_new_output() {
        let mut d = HeuristicDetector::new();
        d.feed(b"Enter password:");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
        // Second check without new output should NOT trigger again.
        assert_eq!(d.check(), HeuristicResult::NoMatch);
    }

    #[test]
    fn detector_retriggers_after_new_output() {
        let mut d = HeuristicDetector::new();
        d.feed(b"Enter password:");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
        // New output re-arms the trigger.
        d.feed(b"\nEnter password:");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
    }

    #[test]
    fn detector_triggers_on_yes_no_paren() {
        let mut d = HeuristicDetector::new();
        d.feed(b"Accept terms (yes/no) ");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
    }

    #[test]
    fn ansi_stripped_before_matching() {
        let mut d = HeuristicDetector::new();
        d.feed(b"\x1b[1mEnter password:\x1b[0m");
        d.last_output_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(d.check(), HeuristicResult::NeedsInput);
    }
}
