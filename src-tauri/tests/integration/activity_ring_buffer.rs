//! T132: Integration test — after 300 output chunks, the ring buffer contains
//! only the last N lines and the summary is the most-recent non-empty line.

use agentui_workbench::session::activity::ActivitySummary;

/// After 300 output chunks, the ring buffer stays bounded and the summary
/// reflects the most-recent meaningful line.
#[test]
fn ring_buffer_bounded_after_300_chunks() {
    let mut activity = ActivitySummary::new();

    for i in 0..300 {
        let chunk = format!("output line {i}\n");
        activity.record_chunk(chunk.as_bytes());
    }

    // The ring buffer should be bounded (≤200 lines per MAX_LINES).
    assert!(
        activity.line_count() <= 200,
        "ring buffer should be bounded to 200 lines, got {}",
        activity.line_count()
    );

    // The summary should be the most-recent non-empty line.
    let summary = activity.current().expect("should have a summary");
    assert_eq!(summary, "output line 299");
}

/// After feeding many large chunks, the ring buffer stays within byte budget.
#[test]
fn ring_buffer_bounded_by_bytes_after_many_large_chunks() {
    let mut activity = ActivitySummary::new();

    let large_line = "x".repeat(200);
    for i in 0..300 {
        let chunk = format!("{large_line} chunk {i}\n");
        activity.record_chunk(chunk.as_bytes());
    }

    // Should not exceed 200 lines.
    assert!(activity.line_count() <= 200);

    // The summary should be the most recent line (truncated to ~80 chars).
    let summary = activity.current().expect("should have a summary");
    assert!(
        summary.chars().count() <= 80,
        "summary should be truncated to ≤80 chars, got {}",
        summary.chars().count()
    );
}

/// Interleaved blank and non-blank lines: the summary is the last non-blank.
#[test]
fn summary_skips_blank_lines() {
    let mut activity = ActivitySummary::new();

    for i in 0..50 {
        activity.record_chunk(format!("real output {i}\n").as_bytes());
        activity.record_chunk(b"\n");
        activity.record_chunk(b"   \n");
    }

    let summary = activity.current().expect("should have a summary");
    assert_eq!(summary, "real output 49");
}

/// Multi-line chunks are split correctly.
#[test]
fn multi_line_chunk_split() {
    let mut activity = ActivitySummary::new();

    let chunk = "first line\nsecond line\nthird line\n";
    activity.record_chunk(chunk.as_bytes());

    assert_eq!(activity.line_count(), 3);
    assert_eq!(activity.current(), Some("third line".to_string()));
}

/// ANSI escape sequences are stripped from the summary.
#[test]
fn ansi_stripped_in_summary() {
    let mut activity = ActivitySummary::new();

    activity.record_chunk(b"\x1b[1;32m\xe2\x9c\x93 All tests passed\x1b[0m\n");

    let summary = activity.current().expect("should have a summary");
    // The ANSI codes should be stripped, Unicode preserved.
    assert!(summary.contains("All tests passed"));
    assert!(!summary.contains("\x1b["));
}
