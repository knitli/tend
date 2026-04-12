//! Wire format for the tend daemon IPC protocol.
//!
//! This crate is the **single source of truth** for the daemon IPC wire shape.
//! Both `tend-workbench` (the Tauri backend) and `tend-cli` depend on it
//! via `{ path = "../protocol" }`. Neither crate is permitted to redefine
//! `Request` / `Response` / `ErrorCode` locally, and neither crate imports
//! internals from the other — the dep flows `{src-tauri, cli} → protocol`,
//! never between the two consumers.
//!
//! Protocol version is a single integer. See `contracts/daemon-ipc.md` §6:
//! * Adding fields to an existing message is NOT a version bump.
//! * New message kinds ARE a version bump.
//!
//! T021: `emit_alert` is deliberately NOT in the `Request` enum — v1 treats
//! alerts as a side effect of `update_status { status: "needs_input" }`. Adding
//! `emit_alert` back will require a `protocol_version` bump.
//!
//! T021: `welcome` MUST NOT include a `session_id_format` field — ids are JSON
//! numbers and that field was dead weight (see `contracts/daemon-ipc.md` §3).
//! Forward-compatibility for capability negotiation will use a new
//! `capabilities: Vec<String>` field gated behind a `protocol_version` bump.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// Current wire protocol version. Increment on a breaking change only.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maximum IPC frame size. Anything larger returns [`ErrorCode::MessageTooLarge`].
pub const MAX_FRAME_SIZE: usize = 64 * 1024;

// ---------- Requests (client → server) ----------

/// A request from a CLI wrapper (or other cooperating client) to the workbench.
///
/// Every variant serializes to a JSON object with `"kind": "<snake_case>"` and
/// the rest of the fields inlined at the top level, matching the contracts
/// document exactly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// Initial handshake. MUST be the first frame on every connection.
    Hello {
        /// Client identifier string (e.g. `"tend-run"`).
        client: String,
        /// Client's own version string.
        client_version: String,
        /// Protocol version the client speaks.
        protocol_version: u32,
    },

    /// Register a new external session with the workbench.
    RegisterSession {
        /// Absolute or relative path to the project root. The workbench will
        /// canonicalize this; if unknown, a new project row is created.
        project_path: String,
        /// User-visible session label. Defaults server-side if absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// Actual session working directory (worktree-aware).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
        /// Command array the wrapper spawned (for display only).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<Vec<String>>,
        /// OS pid of the agent child process owned by the wrapper.
        pid: i32,
        /// Opaque agent-provided metadata (task title, branch, model, …).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },

    /// Cooperative status push for a registered session.
    UpdateStatus {
        /// Target session id (returned by `register_session`).
        session_id: i64,
        /// New status. Clients MAY only emit `working` / `idle` / `needs_input`;
        /// `ended`/`error` are derived by the workbench from child exit.
        status: SessionStatusWire,
        /// Optional human-readable reason (displayed with the alert).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// Optional activity summary override (expires after continued output).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },

    /// Keep-alive. Resets the workbench's heuristic-fallback countdown.
    Heartbeat {
        /// Target session id.
        session_id: i64,
    },

    /// Voluntary end notification from the client.
    EndSession {
        /// Target session id.
        session_id: i64,
        /// Child exit code.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },
}

/// The subset of [`SessionStatus`](fn-level) values that cooperating clients are
/// allowed to send over the wire. `Ended` / `Error` are derived server-side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatusWire {
    /// The session is actively producing output or doing work.
    Working,
    /// The session has been silent beyond the idle threshold.
    Idle,
    /// The session is blocked on user input.
    NeedsInput,
}

// ---------- Responses (server → client) ----------

/// A response frame from the workbench back to a client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    /// Successful reply to `Hello`.
    ///
    /// DELIBERATELY no `session_id_format` field — see T021 / round-2 #5.
    Welcome {
        /// Workbench server version string.
        server_version: String,
        /// Protocol version the server is speaking on this connection.
        protocol_version: u32,
    },

    /// Successful reply to `RegisterSession`.
    SessionRegistered {
        /// Newly allocated session id.
        session_id: i64,
        /// Project the session is attached to (existing or freshly created).
        project_id: i64,
    },

    /// Generic success reply with no additional payload.
    Ack,

    /// Error reply.
    Err {
        /// Machine-readable error code.
        code: ErrorCode,
        /// Human-readable message.
        message: String,
        /// Optional structured details (e.g. `{ "path": "..." }`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<serde_json::Value>,
    },
}

// ---------- Error codes ----------

/// The wire-visible subset of workbench error codes.
///
/// This is a superset of the codes the daemon IPC surface can return; the
/// Tauri-command surface uses a broader [`tend_workbench::error::ErrorCode`]
/// enum that includes the full catalog. Round-tripping through the `err` wire
/// shape is the enforcement gate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Unknown `kind`, missing required field, wrong `protocol_version`.
    ProtocolError,
    /// Frame exceeded [`MAX_FRAME_SIZE`].
    MessageTooLarge,
    /// Filesystem path does not exist.
    PathNotFound,
    /// Referenced entity (e.g. session_id) does not exist.
    NotFound,
    /// Socket permissions rejected (reserved; v1 only allows same-user).
    Unauthorized,
    /// Fallback; `message` carries detail.
    Internal,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ErrorCode::ProtocolError => "PROTOCOL_ERROR",
            ErrorCode::MessageTooLarge => "MESSAGE_TOO_LARGE",
            ErrorCode::PathNotFound => "PATH_NOT_FOUND",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Unauthorized => "UNAUTHORIZED",
            ErrorCode::Internal => "INTERNAL",
        };
        write!(f, "{s}")
    }
}

/// Convenience: construct an [`Response::Err`] with a formatted message.
pub fn error(code: ErrorCode, message: impl Into<String>) -> Response {
    Response::Err {
        code,
        message: message.into(),
        details: None,
    }
}

/// Convenience: construct an [`Response::Err`] with a `details` payload.
pub fn error_with_details(
    code: ErrorCode,
    message: impl Into<String>,
    details: serde_json::Value,
) -> Response {
    Response::Err {
        code,
        message: message.into(),
        details: Some(details),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_roundtrips() {
        let req = Request::Hello {
            client: "tend-run".into(),
            client_version: "0.1.0".into(),
            protocol_version: PROTOCOL_VERSION,
        };
        let s = serde_json::to_string(&req).unwrap();
        let parsed: Request = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn welcome_has_exactly_two_fields() {
        // Round-2 #5: session_id_format removed. Welcome must be exactly the
        // two fields server_version and protocol_version. T036 is the
        // contract test enforcing this. Here we pin it at the wire level.
        let resp = Response::Welcome {
            server_version: "0.1.0".into(),
            protocol_version: PROTOCOL_VERSION,
        };
        let v: serde_json::Value = serde_json::to_value(&resp).unwrap();
        let obj = v.as_object().unwrap();
        // kind + server_version + protocol_version = 3 keys total.
        assert_eq!(obj.len(), 3, "welcome wire shape must be exactly 3 keys");
        assert!(obj.contains_key("kind"));
        assert!(obj.contains_key("server_version"));
        assert!(obj.contains_key("protocol_version"));
        assert!(!obj.contains_key("session_id_format"));
    }

    #[test]
    fn error_code_screaming_snake_case() {
        let resp = error(ErrorCode::PathNotFound, "nope");
        let v: serde_json::Value = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["code"], "PATH_NOT_FOUND");
    }

    #[test]
    fn register_session_required_fields() {
        let req = Request::RegisterSession {
            project_path: "/tmp/p".into(),
            label: None,
            working_directory: None,
            command: None,
            pid: 12345,
            metadata: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"project_path\":\"/tmp/p\""));
        assert!(s.contains("\"pid\":12345"));
        // Optional fields omitted when None.
        assert!(!s.contains("\"label\""));
    }
}
