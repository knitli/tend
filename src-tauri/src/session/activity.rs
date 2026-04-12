//! Per-session activity summary — ring buffer + last-line extraction.
//!
//! T134: `ActivitySummary` holds a bounded ring buffer of recent PTY output
//! (~8 KiB / 200 lines) and extracts a short human-readable summary from the
//! most-recent non-blank, non-prompt line. Agents that send `update_status`
//! with a `summary` string override the heuristic derivation.
//!
//! Design: research.md §13 — ring-buffer last-line, agent-provided override.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of lines retained in the ring buffer.
const MAX_LINES: usize = 200;

/// Maximum byte capacity of the ring buffer (~16 KiB, matching research.md §13).
const MAX_BYTES: usize = 16 * 1024;

/// Maximum length of the returned summary string (characters).
const MAX_SUMMARY_CHARS: usize = 80;

/// After this many seconds of continued output activity, an override expires.
const OVERRIDE_ACTIVITY_TIMEOUT: Duration = Duration::from_secs(10);

/// Absolute expiry for an override regardless of activity.
const OVERRIDE_ABSOLUTE_TIMEOUT: Duration = Duration::from_secs(60);

/// Per-session activity summary derived from PTY output.
///
/// Thread-safety: intended to be wrapped in `Arc<Mutex<..>>` or
/// `tokio::sync::Mutex` and shared between the reader task (writes) and
/// the session-list query path (reads).
#[derive(Debug)]
pub struct ActivitySummary {
    /// Ring buffer of recent output lines (most recent at back).
    lines: VecDeque<String>,
    /// Current total byte count across all lines in the ring.
    total_bytes: usize,
    /// Partial line accumulator (no trailing newline yet).
    partial: String,

    /// Agent-provided override summary.
    override_summary: Option<String>,
    /// When the override was set.
    override_set_at: Option<Instant>,
    /// Timestamp of the last `record_chunk` call after the override was set.
    /// Used for the 10-second continued-activity timeout.
    last_activity_after_override: Option<Instant>,
}

impl ActivitySummary {
    /// Create a new empty summary.
    pub fn new() -> Self {
        Self {
            lines: VecDeque::with_capacity(MAX_LINES),
            total_bytes: 0,
            partial: String::new(),
            override_summary: None,
            override_set_at: None,
            last_activity_after_override: None,
        }
    }

    /// Feed raw PTY output bytes into the ring buffer.
    pub fn record_chunk(&mut self, bytes: &[u8]) {
        // Track activity for override expiry.
        if self.override_summary.is_some() {
            self.last_activity_after_override = Some(Instant::now());
        }

        // Decode as lossy UTF-8 and strip ANSI escape sequences.
        let text = String::from_utf8_lossy(bytes);
        let stripped = strip_ansi(&text);

        // Process character-by-character to handle \r (carriage return)
        // like a real terminal: reset to start of current line.
        for ch in stripped.chars() {
            if ch == '\n' {
                // Complete line — push to ring buffer. M2/M4 fix: skip
                // empty lines produced by CRLF to preserve ring capacity.
                let line = std::mem::take(&mut self.partial);
                if !line.is_empty() {
                    self.push_line(line);
                }
            } else if ch == '\r' {
                // Carriage return: reset partial (next chars overwrite).
                self.partial.clear();
            } else {
                self.partial.push(ch);
                // C1: Cap partial buffer to prevent unbounded growth from
                // PTY output that never contains a newline.
                if self.partial.len() > MAX_BYTES {
                    let keep_from = self.partial.len() - MAX_BYTES;
                    self.partial.drain(0..keep_from);
                }
            }
        }

        // Enforce byte budget.
        self.trim_to_budget();
    }

    /// Set an agent-provided override summary. Expires after 10 s of continued
    /// output activity or 60 s absolute (research.md §13).
    pub fn override_with(&mut self, summary: &str) {
        let trimmed = summary.trim();
        if trimmed.is_empty() {
            self.override_summary = None;
            self.override_set_at = None;
            self.last_activity_after_override = None;
        } else {
            self.override_summary = Some(truncate_chars(trimmed, MAX_SUMMARY_CHARS));
            self.override_set_at = Some(Instant::now());
            self.last_activity_after_override = None;
        }
    }

    /// Get the current activity summary, preferring an unexpired override.
    #[must_use]
    pub fn current(&self) -> Option<String> {
        // Check override first.
        if let Some(ref summary) = self.override_summary {
            if !self.is_override_expired() {
                return Some(summary.clone());
            }
        }

        // Fall back to last non-blank, non-prompt line from the ring buffer.
        self.last_meaningful_line()
    }

    /// Check whether the override has expired due to timeouts.
    fn is_override_expired(&self) -> bool {
        let set_at = match self.override_set_at {
            Some(t) => t,
            None => return true,
        };

        let now = Instant::now();

        // Absolute timeout.
        if now.duration_since(set_at) >= OVERRIDE_ABSOLUTE_TIMEOUT {
            return true;
        }

        // Activity timeout: if output has been produced continuously for ≥10 s
        // since the override was set, expire it. H1 fix: removed the 500ms
        // query-time guard which made this path practically unreachable.
        if let Some(last_activity) = self.last_activity_after_override {
            if last_activity.duration_since(set_at) >= OVERRIDE_ACTIVITY_TIMEOUT {
                return true;
            }
        }

        false
    }

    /// Extract the last non-blank, non-prompt line from the ring buffer.
    fn last_meaningful_line(&self) -> Option<String> {
        for line in self.lines.iter().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip lines that look like shell prompts (end with $ or #
            // followed by optional whitespace, or common prompt markers).
            if is_prompt_line(trimmed) {
                continue;
            }
            return Some(truncate_chars(trimmed, MAX_SUMMARY_CHARS));
        }
        None
    }

    /// Push a complete line into the ring buffer.
    fn push_line(&mut self, line: String) {
        let line_bytes = line.len();
        self.lines.push_back(line);
        self.total_bytes += line_bytes;

        // Evict oldest lines if we exceed the line count.
        while self.lines.len() > MAX_LINES {
            if let Some(old) = self.lines.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(old.len());
            }
        }
    }

    /// Trim the buffer to stay within the byte budget.
    fn trim_to_budget(&mut self) {
        while self.total_bytes > MAX_BYTES && !self.lines.is_empty() {
            if let Some(old) = self.lines.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(old.len());
            }
        }
    }

    /// Clear all state. Called on session end for explicit cleanup (QA L1).
    pub fn clear(&mut self) {
        self.lines.clear();
        self.total_bytes = 0;
        self.partial.clear();
        self.override_summary = None;
        self.override_set_at = None;
        self.last_activity_after_override = None;
    }

    /// Number of complete lines in the ring buffer (for testing / diagnostics).
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

impl Default for ActivitySummary {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip ANSI escape sequences from a string.
/// L4 fix: returns `Cow::Borrowed` when no escape sequences are present.
fn strip_ansi(s: &str) -> Cow<'_, str> {
    // Fast path: no escape sequences at all.
    if !s.contains('\x1b') {
        return Cow::Borrowed(s);
    }
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip CSI sequences: ESC [ ... final_byte
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                              // Consume parameter bytes (0x30-0x3F) and intermediate bytes (0x20-0x2F)
                              // until a final byte (0x40-0x7E).
                loop {
                    match chars.peek() {
                        Some(&c) if ('\x40'..='\x7e').contains(&c) => {
                            chars.next(); // consume final byte
                            break;
                        }
                        Some(_) => {
                            chars.next();
                        }
                        None => break,
                    }
                }
            } else if matches!(
                chars.peek(),
                Some(&']') | Some(&'P') | Some(&'^') | Some(&'_')
            ) {
                // OSC (ESC ]), DCS (ESC P), PM (ESC ^), APC (ESC _):
                // all ST-terminated sequences. Consume until ST (ESC \) or BEL.
                // H3 fix: added DCS/PM/APC alongside OSC.
                chars.next(); // consume introducer character
                loop {
                    match chars.next() {
                        Some('\x07') => break, // BEL
                        Some('\x1b') if chars.peek() == Some(&'\\') => {
                            chars.next();
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
            // Other ESC sequences (single-char): skip next char.
            else if chars.peek().is_some() {
                chars.next();
            }
        } else {
            result.push(c);
        }
    }
    Cow::Owned(result)
}

/// Check if a line looks like a shell prompt.
///
/// M1 fix: Require a path-like or user@host prefix before `$` / `#` to
/// reduce false positives on agent output. Bare `>` only matches if the
/// entire line is `>` (zsh secondary prompt), not prose ending in `>`.
fn is_prompt_line(line: &str) -> bool {
    // Bare `>` secondary prompt (exact match after trim).
    if line == ">" {
        return true;
    }
    // Lines ending with $ or # are prompts only if they contain a path
    // separator or @ (e.g. user@host:~/dir$, /home/user#).
    if (line.ends_with('$') || line.ends_with('#'))
        && (line.contains('/') || line.contains('@') || line.contains(':'))
    {
        return true;
    }
    false
}

/// Truncate a string to at most `max_chars` Unicode scalar values,
/// appending "…" if truncated.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    // Find the byte boundary of the (max_chars)th character.
    // If the string is shorter, return it as-is.
    match s.char_indices().nth(max_chars) {
        None => s.to_string(),
        Some(_) => {
            // Keep (max_chars - 1) chars + "…" = max_chars total.
            let keep = max_chars.saturating_sub(1);
            let end = s
                .char_indices()
                .nth(keep)
                .map_or(s.len(), |(i, _)| i);
            format!("{}…", &s[..end])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_summary_returns_none() {
        let summary = ActivitySummary::new();
        assert_eq!(summary.current(), None);
    }

    #[test]
    fn records_single_line() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"Hello world\n");
        assert_eq!(summary.current(), Some("Hello world".to_string()));
    }

    #[test]
    fn records_multiple_lines_returns_last() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"line 1\nline 2\nline 3\n");
        assert_eq!(summary.current(), Some("line 3".to_string()));
    }

    #[test]
    fn skips_blank_lines() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"meaningful line\n\n  \n");
        assert_eq!(summary.current(), Some("meaningful line".to_string()));
    }

    #[test]
    fn skips_prompt_lines() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"real output\nuser@host:~$ \n");
        assert_eq!(summary.current(), Some("real output".to_string()));
    }

    #[test]
    fn strips_ansi_sequences() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"\x1b[32mgreen text\x1b[0m\n");
        assert_eq!(summary.current(), Some("green text".to_string()));
    }

    #[test]
    fn ring_buffer_bounded_by_lines() {
        let mut summary = ActivitySummary::new();
        for i in 0..300 {
            summary.record_chunk(format!("line {i}\n").as_bytes());
        }
        assert!(summary.line_count() <= MAX_LINES);
        // Last line should be the most recent.
        assert_eq!(summary.current(), Some("line 299".to_string()));
    }

    #[test]
    fn ring_buffer_bounded_by_bytes() {
        let mut summary = ActivitySummary::new();
        // Each line is ~100 bytes, 200 lines = ~20 KiB > 8 KiB budget.
        let long_line = "x".repeat(100);
        for _ in 0..200 {
            summary.record_chunk(format!("{long_line}\n").as_bytes());
        }
        assert!(summary.total_bytes <= MAX_BYTES + 200); // Some slack for partial.
    }

    #[test]
    fn override_takes_precedence() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"background noise\n");
        summary.override_with("Refactoring lexer");
        assert_eq!(summary.current(), Some("Refactoring lexer".to_string()));
    }

    #[test]
    fn empty_override_clears() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"real output\n");
        summary.override_with("task override");
        summary.override_with("");
        assert_eq!(summary.current(), Some("real output".to_string()));
    }

    #[test]
    fn truncates_long_lines() {
        let mut summary = ActivitySummary::new();
        let long_line = "a".repeat(200);
        summary.record_chunk(format!("{long_line}\n").as_bytes());
        let result = summary.current().unwrap();
        assert!(result.chars().count() <= MAX_SUMMARY_CHARS);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn partial_line_not_returned_until_newline() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"partial");
        // No newline yet — the partial line is not a complete line.
        // But if there's nothing else, current() should still return None
        // because we only surface complete lines.
        assert_eq!(summary.current(), None);

        // Complete the line.
        summary.record_chunk(b" complete\n");
        assert_eq!(summary.current(), Some("partial complete".to_string()));
    }

    #[test]
    fn handles_carriage_returns() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"progress: 50%\rprogress: 100%\n");
        assert_eq!(summary.current(), Some("progress: 100%".to_string()));
    }

    // C1: partial buffer is bounded even without newlines.
    #[test]
    fn partial_buffer_capped() {
        let mut summary = ActivitySummary::new();
        // Feed 20 KiB of data with no newlines.
        let chunk = "x".repeat(20 * 1024);
        summary.record_chunk(chunk.as_bytes());
        assert!(
            summary.partial.len() <= MAX_BYTES,
            "partial should be capped at MAX_BYTES, got {}",
            summary.partial.len()
        );
    }

    // C1: empty input does not panic.
    #[test]
    fn empty_chunk_is_noop() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"");
        assert_eq!(summary.line_count(), 0);
        assert_eq!(summary.current(), None);
    }

    // H2: bare > is detected as a prompt.
    #[test]
    fn skips_bare_angle_bracket_prompt() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"real output\n>\n");
        assert_eq!(summary.current(), Some("real output".to_string()));
    }

    // H3: DCS sequences are stripped.
    #[test]
    fn strips_dcs_sequences() {
        let mut summary = ActivitySummary::new();
        // DCS (ESC P) ... ST (ESC \)
        summary.record_chunk(b"\x1bPsome dcs payload\x1b\\visible text\n");
        assert_eq!(summary.current(), Some("visible text".to_string()));
    }

    // H3: APC sequences are stripped.
    #[test]
    fn strips_apc_sequences() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"\x1b_apc payload\x1b\\visible text\n");
        assert_eq!(summary.current(), Some("visible text".to_string()));
    }

    // H2: prompt detection — line ending with $ still detected.
    #[test]
    fn skips_dollar_prompt() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"real output\nuser@host:~$\n");
        assert_eq!(summary.current(), Some("real output".to_string()));
    }

    // Whitespace-only override clears.
    #[test]
    fn whitespace_only_override_clears() {
        let mut summary = ActivitySummary::new();
        summary.record_chunk(b"real output\n");
        summary.override_with("task");
        summary.override_with("   ");
        assert_eq!(summary.current(), Some("real output".to_string()));
    }
}
