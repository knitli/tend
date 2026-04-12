//! Session lifecycle. In Phase 2 we only have the PTY wrapper (T020) and
//! the crash-recovery reconcile_and_reattach pass (T025). The actor, status
//! monitor, service, and commands land in US1.

pub mod pty;
pub mod recovery;
