//! T036: `hello` daemon IPC contract tests.

use tend_protocol::{Request, Response, PROTOCOL_VERSION};
use tend_workbench::daemon::handlers::dispatch;

/// Happy path: Hello with correct protocol_version returns Welcome with
/// server_version and protocol_version.
#[tokio::test]
async fn hello_happy_path() {
    let state = crate::common::mock_state().await;

    let req = Request::Hello {
        client: "test-client".into(),
        client_version: "0.0.1".into(),
        protocol_version: PROTOCOL_VERSION,
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Welcome {
            server_version,
            protocol_version,
        } => {
            assert!(
                !server_version.is_empty(),
                "server_version must be non-empty"
            );
            assert_eq!(
                protocol_version, PROTOCOL_VERSION,
                "protocol_version must match"
            );
        }
        Response::Err { code, message, .. } => {
            panic!("expected Welcome, got Err({code:?}): {message}");
        }
        other => panic!("expected Welcome, got {other:?}"),
    }
}

/// Protocol version mismatch: sending an incompatible protocol_version
/// returns PROTOCOL_ERROR.
#[tokio::test]
async fn hello_protocol_version_mismatch() {
    let state = crate::common::mock_state().await;

    let req = Request::Hello {
        client: "test-client".into(),
        client_version: "0.0.1".into(),
        protocol_version: 99, // Invalid version.
    };

    let resp = dispatch(req, &state).await;

    match resp {
        Response::Err { code, .. } => {
            assert_eq!(
                code,
                tend_protocol::ErrorCode::ProtocolError,
                "mismatched version should return PROTOCOL_ERROR"
            );
        }
        Response::Welcome { .. } => {
            panic!("expected PROTOCOL_ERROR for version=99, got Welcome");
        }
        other => panic!("expected Err, got {other:?}"),
    }
}
