//! Daemon IPC request dispatch.
//!
//! T024: STUB for Phase 2. Every variant returns a `protocol_error(…)` with a
//! "not yet implemented" message. The real implementations for `hello`,
//! `register_session`, `update_status`, `heartbeat`, and `end_session` land
//! in US1 (T050, T051, T052). Contract tests in T036–T039 are expected to
//! be RED against this stub — that's the TDD gate.

use crate::state::WorkbenchState;
use agentui_protocol::{error as protocol_error, ErrorCode, Request, Response};

/// Dispatch a daemon IPC request to its handler. Every variant returns a
/// protocol error in Phase 2.
pub async fn dispatch(req: Request, _state: &WorkbenchState) -> Response {
    match req {
        Request::Hello { .. } => protocol_error(
            ErrorCode::ProtocolError,
            "hello handler not yet implemented (US1 T050)",
        ),
        Request::RegisterSession { .. } => protocol_error(
            ErrorCode::ProtocolError,
            "register_session handler not yet implemented (US1 T051)",
        ),
        Request::UpdateStatus { .. } => protocol_error(
            ErrorCode::ProtocolError,
            "update_status handler not yet implemented (US1 T052)",
        ),
        Request::Heartbeat { .. } => protocol_error(
            ErrorCode::ProtocolError,
            "heartbeat handler not yet implemented (US1 T052)",
        ),
        Request::EndSession { .. } => protocol_error(
            ErrorCode::ProtocolError,
            "end_session handler not yet implemented (US1 T052)",
        ),
    }
}
