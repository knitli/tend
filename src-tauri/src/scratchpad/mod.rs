//! Scratchpad services — notes, reminders, and cross-project overview.

pub mod notes;
pub mod overview;
pub mod reminders;

pub use notes::NoteService;
pub use overview::OverviewService;
pub use reminders::ReminderService;
