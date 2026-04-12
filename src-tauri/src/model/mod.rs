//! Domain model for the workbench backend.
//!
//! T012: newtype IDs for every persisted entity. These wrap `i64` (the
//! SQLite rowid type) and derive `sqlx::Type` so sqlx can bind them directly
//! as query parameters. The newtypes prevent accidental cross-entity id
//! mixups at the type level.
//!
//! T013: per-entity modules with serde-friendly domain types for the Tauri
//! command JSON boundary.

pub mod alert;
pub mod companion;
pub mod layout;
pub mod note;
pub mod notification;
pub mod project;
pub mod reminder;
pub mod session;
pub mod workspace;

use serde::{Deserialize, Serialize};

/// Macro: define a newtype around `i64` with all the derives we need.
macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Copy,
            Clone,
            Debug,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            Serialize,
            Deserialize,
            sqlx::Type,
        )]
        #[serde(transparent)]
        #[sqlx(transparent)]
        pub struct $name(pub i64);

        impl $name {
            /// Construct from a raw row id.
            pub const fn new(id: i64) -> Self {
                Self(id)
            }

            /// Extract the underlying i64.
            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<i64> for $name {
            fn from(value: i64) -> Self {
                Self(value)
            }
        }

        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

id_newtype!(
    /// Stable id for a registered project.
    ProjectId
);
id_newtype!(
    /// Stable id for a session (persisted).
    SessionId
);
id_newtype!(
    /// Stable id for a companion terminal row.
    CompanionId
);
id_newtype!(
    /// Stable id for a scratchpad note.
    NoteId
);
id_newtype!(
    /// Stable id for a reminder.
    ReminderId
);
id_newtype!(
    /// Stable id for a named layout.
    LayoutId
);
id_newtype!(
    /// Stable id for a raised alert.
    AlertId
);
id_newtype!(
    /// Stable id for a notification preference row.
    NotificationPreferenceId
);

// Re-export the main record types for convenience.
pub use alert::{Alert, AlertClearedBy, AlertKind};
pub use companion::CompanionTerminal;
pub use layout::Layout;
pub use note::Note;
pub use notification::{NotificationChannel, NotificationPreference, QuietHours};
pub use project::{Project, ProjectSettings};
pub use reminder::{Reminder, ReminderState};
pub use session::{
    Session, SessionMetadata, SessionOwnership, SessionStatus, SessionSummary, StatusSource,
};
pub use workspace::WorkspaceState;

/// UTC-aware timestamp serialized as ISO-8601.
pub type Timestamp = chrono::DateTime<chrono::Utc>;

/// Type alias for an OS pid (kept as i32 to match `std::process::Child::id` /
/// `libc::pid_t` across platforms we care about).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pid(pub i32);

impl std::fmt::Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
