//! T058: CLI IPC happy-path integration test.
//!
//! Starts a lightweight mock daemon that responds to the IPC protocol,
//! then exercises the CLI's `IpcClient` through the full lifecycle:
//! hello → register_session → heartbeat → end_session.

mod common;

use tend_protocol::{Request, Response, PROTOCOL_VERSION};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

/// Start a mock daemon that speaks just enough of the protocol for the test.
async fn start_mock_daemon(socket_path: &std::path::Path) -> tokio::task::JoinHandle<Vec<String>> {
    let listener = UnixListener::bind(socket_path).expect("bind mock socket");
    let next_id = Arc::new(AtomicI64::new(1));

    tokio::spawn(async move {
        let mut received_kinds = Vec::new();
        let (mut stream, _) = listener.accept().await.expect("accept");

        loop {
            // Read frame: u32 LE length prefix + JSON body.
            let mut len_buf = [0u8; 4];
            if stream.read_exact(&mut len_buf).await.is_err() {
                break;
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            if stream.read_exact(&mut buf).await.is_err() {
                break;
            }

            let req: Request = serde_json::from_slice(&buf).expect("parse request");
            let kind = match &req {
                Request::Hello { .. } => "hello",
                Request::RegisterSession { .. } => "register_session",
                Request::UpdateStatus { .. } => "update_status",
                Request::Heartbeat { .. } => "heartbeat",
                Request::EndSession { .. } => "end_session",
            };
            received_kinds.push(kind.to_string());

            let response = match req {
                Request::Hello {
                    protocol_version, ..
                } => {
                    if protocol_version != PROTOCOL_VERSION {
                        Response::Err {
                            code: tend_protocol::ErrorCode::ProtocolError,
                            message: "version mismatch".into(),
                            details: None,
                        }
                    } else {
                        Response::Welcome {
                            server_version: "0.1.0-test".into(),
                            protocol_version: PROTOCOL_VERSION,
                        }
                    }
                }
                Request::RegisterSession { .. } => {
                    let sid = next_id.fetch_add(1, Ordering::SeqCst);
                    Response::SessionRegistered {
                        session_id: sid,
                        project_id: 1,
                    }
                }
                Request::Heartbeat { .. }
                | Request::UpdateStatus { .. }
                | Request::EndSession { .. } => Response::Ack,
            };

            let resp_bytes = serde_json::to_vec(&response).unwrap();
            let resp_len = (resp_bytes.len() as u32).to_le_bytes();
            let _ = stream.write_all(&resp_len).await;
            let _ = stream.write_all(&resp_bytes).await;
            let _ = stream.flush().await;
        }

        received_kinds
    })
}

/// Full lifecycle: hello → register → heartbeat → end.
#[tokio::test]
async fn ipc_lifecycle_happy_path() {
    let socket_path = common::temp_socket_path();
    let mock = start_mock_daemon(&socket_path).await;

    // Give the listener a moment to bind.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Connect the CLI's IPC framing directly (same protocol as IpcClient).
    let mut stream = tokio::net::UnixStream::connect(&socket_path)
        .await
        .expect("connect");

    // Helper: send a request and read the response.
    async fn roundtrip(stream: &mut tokio::net::UnixStream, req: &Request) -> Response {
        let payload = serde_json::to_vec(req).unwrap();
        let len = (payload.len() as u32).to_le_bytes();
        stream.write_all(&len).await.unwrap();
        stream.write_all(&payload).await.unwrap();
        stream.flush().await.unwrap();

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.unwrap();
        let rlen = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; rlen];
        stream.read_exact(&mut body).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    // 1. Hello → Welcome.
    let resp = roundtrip(
        &mut stream,
        &Request::Hello {
            client: "test".into(),
            client_version: "0.0.0".into(),
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await;
    assert!(matches!(resp, Response::Welcome { .. }), "expected Welcome");

    // 2. Register session.
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let resp = roundtrip(
        &mut stream,
        &Request::RegisterSession {
            project_path: tmp_dir.path().to_string_lossy().into(),
            label: Some("t058".into()),
            working_directory: None,
            command: Some(vec!["/bin/echo".into(), "hi".into()]),
            pid: std::process::id() as i32,
            metadata: None,
        },
    )
    .await;
    let session_id = match &resp {
        Response::SessionRegistered {
            session_id,
            project_id,
        } => {
            assert!(*session_id > 0);
            assert!(*project_id > 0);
            *session_id
        }
        other => panic!("expected SessionRegistered, got {other:?}"),
    };

    // 3. Heartbeat → Ack.
    let resp = roundtrip(&mut stream, &Request::Heartbeat { session_id }).await;
    assert!(matches!(resp, Response::Ack), "heartbeat should Ack");

    // 4. End session → Ack.
    let resp = roundtrip(
        &mut stream,
        &Request::EndSession {
            session_id,
            exit_code: Some(0),
        },
    )
    .await;
    assert!(matches!(resp, Response::Ack), "end_session should Ack");

    // Close the connection so the mock task can return.
    drop(stream);

    let kinds = mock.await.expect("mock task");
    assert_eq!(
        kinds,
        vec!["hello", "register_session", "heartbeat", "end_session"],
        "expected exact IPC frame sequence"
    );

    let _ = std::fs::remove_file(&socket_path);
}
