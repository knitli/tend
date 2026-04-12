//! Notification subsystem — alert lifecycle, preferences, and OS dispatch.
//!
//! T072: `AlertService` — raise, clear, acknowledge, list_open.
//! T075: `PreferenceService` — get/set notification prefs with quiet-hours.
//! T076: `dispatch_alert` — route to OS notification or in-app per prefs.

pub mod alerts;
pub mod dispatch;
pub mod preferences;

pub use alerts::AlertService;
pub use dispatch::dispatch_alert;
pub use preferences::PreferenceService;
