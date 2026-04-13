//! T040: End-to-end CLI → daemon IPC → sessions table integration test.
//!
//! 1. Start a temp daemon (`spawn_daemon` with a temp socket path).
//! 2. Connect as a client, send `hello`, then `register_session` with a temp
//!    project path.
//! 3. Query `session_list` via `SessionService::list` and assert the registered
//!    session appears with `ownership = Wrapper`.
//!
//! This proves the full happy path: CLI client → daemon socket → dispatch →
//! service → sqlite → list query roundtrip.

mod common;

use std::time::Duration;
use tend_protocol::{PROTOCOL_VERSION, Request, Response};
use tend_workbench::daemon::spawn_daemon;
use tend_workbench::model::SessionOwnership;
use tend_workbench::session::SessionService;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Write a u32-LE-prefixed JSON frame to the stream.
async fn write_frame(stream: &mut UnixStream, req: &Request) {
    let payload = serde_json::to_vec(req).expect("serialize request");
    let len = (payload.len() as u32).to_le_bytes();
    stream.write_all(&len).await.expect("write frame length");
    stream.write_all(&payload).await.expect("write frame body");
    stream.flush().await.expect("flush");
}

/// Read a u32-LE-prefixed JSON frame from the stream and deserialize.
async fn read_frame(stream: &mut UnixStream) -> Response {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .expect("read frame length");
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.expect("read frame body");
    serde_json::from_slice(&body).expect("deserialize response")
}

#[tokio::test]
async fn cli_register_session_appears_in_list() {
    #[cfg(not(unix))]
    {
        eprintln!("T040 is unix-only; skipping on non-unix");
        return;
    }

    #[cfg(unix)]
    {
        let state = common::mock_state().await;
        let socket_path = common::temp_socket_path();

        // Create a temp project directory so the daemon can canonicalize it.
        let tmp_dir = tempfile::tempdir().expect("create temp project dir");
        let project_path = tmp_dir.path().to_str().expect("path to str").to_string();

        // 1. Start the daemon.
        let handle = spawn_daemon(state.clone(), Some(socket_path.clone()))
            .await
            .expect("spawn_daemon");

        // Give the listener a moment to bind.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 2. Connect as a client.
        let mut stream = UnixStream::connect(&socket_path)
            .await
            .expect("connect to daemon");

        // Send hello.
        let hello = Request::Hello {
            client: "tend-run".into(),
            client_version: "0.1.0-test".into(),
            protocol_version: PROTOCOL_VERSION,
        };
        write_frame(&mut stream, &hello).await;
        let welcome = read_frame(&mut stream).await;
        assert!(
            matches!(welcome, Response::Welcome { .. }),
            "expected Welcome, got {welcome:?}"
        );

        // Send register_session.
        let register = Request::RegisterSession {
            project_path: project_path.clone(),
            label: Some("t040-test".into()),
            working_directory: None,
            command: Some(vec!["echo".into(), "hello".into()]),
            pid: std::process::id() as i32,
            metadata: None,
        };
        write_frame(&mut stream, &register).await;
        let reg_resp = read_frame(&mut stream).await;

        let (session_id, project_id) = match reg_resp {
            Response::SessionRegistered {
                session_id,
                project_id,
            } => (session_id, project_id),
            other => panic!("expected SessionRegistered, got {other:?}"),
        };

        assert!(session_id > 0, "session_id must be positive");
        assert!(project_id > 0, "project_id must be positive");

        // 3. Query via SessionService::list and assert the session appears.
        let project_id_typed = tend_workbench::model::ProjectId::new(project_id);
        let sessions = SessionService::list(&state, Some(project_id_typed), false)
            .await
            .expect("session_list should succeed");

        assert_eq!(sessions.len(), 1, "should have exactly one session");
        let summary = &sessions[0];
        assert_eq!(summary.session.id.get(), session_id);
        assert_eq!(
            summary.session.ownership,
            SessionOwnership::Wrapper,
            "IPC-registered session must have ownership = Wrapper"
        );
        assert_eq!(
            summary.session.label.as_str(),
            "t040-test",
            "label must round-trip through IPC"
        );

        // Clean up: drop the stream, abort the daemon task.
        drop(stream);
        handle.task.abort();
        let _ = std::fs::remove_file(&socket_path);
    }
}
