//! Bounded replay buffer for a live session's raw PTY bytes.
//!
//! Problem: the frontend's AgentPane can only subscribe to `session:event`
//! output *after* it mounts. For a freshly-spawned TUI like Claude, the
//! child process may have already emitted the initial full-screen render
//! by the time the listener is ready — leaving the UI blank because
//! subsequent updates are incremental diffs against a frame we never saw.
//!
//! Fix: keep the last ~64KB of raw PTY bytes in a ring buffer, and expose
//! a `session_read_backlog` command so the pane can replay them on mount.

use std::collections::VecDeque;

/// Max bytes retained. A typical terminal screen (80x24 full-color ANSI) is
/// ~8KB; 64KB covers the initial render plus scrollback-worthy context for
/// most TUIs without bloating memory for many simultaneous sessions.
pub const REPLAY_CAP: usize = 64 * 1024;

/// A fixed-capacity ring of raw bytes. When full, oldest bytes are dropped
/// to make room for new ones.
#[derive(Debug, Default)]
pub struct ReplayBuffer {
    buf: VecDeque<u8>,
}

impl ReplayBuffer {
    /// Construct an empty buffer pre-sized to [`REPLAY_CAP`].
    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(REPLAY_CAP),
        }
    }

    /// Append `chunk`. If the resulting size exceeds [`REPLAY_CAP`], drop
    /// from the front. Chunks larger than cap are truncated from their head
    /// (we keep the tail, which is most likely to reflect current screen state).
    pub fn push(&mut self, chunk: &[u8]) {
        if chunk.len() >= REPLAY_CAP {
            // New chunk alone exceeds cap — replace wholesale with its tail.
            self.buf.clear();
            self.buf.extend(&chunk[chunk.len() - REPLAY_CAP..]);
            return;
        }
        let needed = (self.buf.len() + chunk.len()).saturating_sub(REPLAY_CAP);
        if needed > 0 {
            self.buf.drain(..needed);
        }
        self.buf.extend(chunk);
    }

    /// Return a contiguous snapshot of the current buffer contents.
    pub fn snapshot(&self) -> Vec<u8> {
        let (a, b) = self.buf.as_slices();
        let mut out = Vec::with_capacity(self.buf.len());
        out.extend_from_slice(a);
        out.extend_from_slice(b);
        out
    }

    /// Number of bytes currently buffered.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// `true` if no bytes have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_under_cap_retains_everything() {
        let mut r = ReplayBuffer::new();
        r.push(b"hello ");
        r.push(b"world");
        assert_eq!(r.snapshot(), b"hello world");
    }

    #[test]
    fn push_over_cap_drops_oldest() {
        let mut r = ReplayBuffer::new();
        r.push(&vec![b'a'; REPLAY_CAP - 10]);
        r.push(b"xxxxxxxxxxxxxxxxxxxx"); // 20 bytes — overflows by 10
        let snap = r.snapshot();
        assert_eq!(snap.len(), REPLAY_CAP);
        // First 10 'a's were dropped; the last 10 'a's remain, then all 'x's.
        assert!(snap.starts_with(&vec![b'a'; REPLAY_CAP - 20]));
        assert!(snap.ends_with(b"xxxxxxxxxxxxxxxxxxxx"));
    }

    #[test]
    fn single_chunk_larger_than_cap_keeps_tail() {
        let mut r = ReplayBuffer::new();
        let mut big = vec![b'z'; REPLAY_CAP + 100];
        big[REPLAY_CAP + 99] = b'!';
        r.push(&big);
        let snap = r.snapshot();
        assert_eq!(snap.len(), REPLAY_CAP);
        assert_eq!(*snap.last().unwrap(), b'!');
    }
}
