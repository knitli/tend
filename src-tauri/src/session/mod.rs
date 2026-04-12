//! Session lifecycle — PTY wrapper, live-session actor, status monitor,
//! supervisor tasks, service layer, and crash-recovery reconciliation.

pub mod live;
pub mod pty;
pub mod reaper;
pub mod recovery;
pub mod service;
pub mod status;
pub mod supervisor;

pub use service::SessionService;
