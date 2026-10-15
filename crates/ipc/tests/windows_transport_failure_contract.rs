//! Public failure and timeout contract for the Windows `NamedPipe` transport.
//!
//! These tests exercise only crate-root APIs. They use local pipes and process
//! metadata, never a network service, credential store, GUI, or real app.

#![cfg(windows)]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use assistant_ipc::{
    Heartbeat, IpcError, NamedPipeTransport, RequestMessage, Transport, WireMessage,
    peer_process_image_path,
};
use assistant_protocol::ToolEnvelope;

const TIMEOUT: Duration = Duration::from_secs(5);
const SHORT_TIMEOUT: Duration = Duration::from_millis(50);
static NEXT_PIPE_ID: AtomicU64 = AtomicU64::new(1);

fn unique_pipe_name() -> String {
    let sequence = NEXT_PIPE_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "assistant-ipc-failure-contract-{}-{sequence}",
        std::process::id()
    )
}

fn heartbeat() -> WireMessage {
    WireMessage::Heartbeat(Heartbeat {
        session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_string(),
        timestamp_unix_ms: 2_000,
        alive: true,
    })
}

fn large_request() -> WireMessage {
    let payload = "x".repeat(4 * 1024 * 1024);
    WireMessage::Request(RequestMessage::new(
        "large-request",
        "ipc-test-target",
        ToolEnvelope::ok(
            "ipc.test.large".to_string(),
            "task-302".to_string(),
            "step-1".to_string(),
            serde_json::json!({"payload": payload}),
        ),
    ))
}

fn assert_invalid_pipe_name(pipe_name: &str) {
    assert!(matches!(
        NamedPipeTransport::client(pipe_name),
        Err(IpcError::Serialization { .. })
    ));
    assert!(matches!(
        NamedPipeTransport::server(pipe_name),
        Err(IpcError::Serialization { .. })
    ));
}

#[test]
fn test_named_pipe_construction_rejects_invalid_names() {
    assert_invalid_pipe_name("");
    assert_invalid_pipe_name("two words");
    assert_invalid_pipe_name("nested/name");
    assert_invalid_pipe_name(r"nested\name");
    assert_invalid_pipe_name(&"a".repeat(201));
}

#[test]
fn test_closed_transports_fail_explicitly_without_an_os_handle() {
    let pipe_name = unique_pipe_name();
    let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
    client.close().expect("close unconnected client");
    client.close().expect("second close is a no-op");
    assert!(matches!(
        client.send(&heartbeat(), SHORT_TIMEOUT),
        Err(IpcError::Transport { .. })
    ));
    assert!(matches!(
        client.recv(SHORT_TIMEOUT),
        Err(IpcError::Transport { .. })
    ));
    assert!(matches!(
        client.peer_process_id(),
        Err(IpcError::PeerIdentityUnavailable { .. })
    ));

    let mut server = NamedPipeTransport::server(&unique_pipe_name()).expect("create server");
    server.close().expect("close unaccepted server");
    assert!(matches!(
        server.accept(SHORT_TIMEOUT),
        Err(IpcError::Transport { .. })
    ));
}

#[test]
fn test_accept_timeout_is_explicit_and_the_server_can_close() {
    let mut server = NamedPipeTransport::server(&unique_pipe_name()).expect("create server");
    let error = server
        .accept(SHORT_TIMEOUT)
        .expect_err("accept without a client must time out");
    assert!(matches!(
        error,
        IpcError::Timeout {
            operation: "named-pipe-accept",
            ..
        }
    ));
    server.close().expect("close after accept timeout");
}

#[test]
fn test_recv_timeout_cancels_the_read_and_connection_can_be_reused() {
    let pipe_name = unique_pipe_name();
    let server_pipe_name = pipe_name.clone();
    let (release_sender, release_receiver) = mpsc::channel();
    let (ack_sender, ack_receiver) = mpsc::channel();
    let server_thread = thread::spawn(move || {
        let mut server = NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
        server.accept(TIMEOUT).expect("accept client");
        release_receiver
            .recv_timeout(TIMEOUT)
            .expect("wait for client timeout");
        server.send(&heartbeat(), TIMEOUT).expect("server send");
        ack_receiver.recv_timeout(TIMEOUT).expect("client ack");
        server.close().expect("server close");
    });

    let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
    client.connect(TIMEOUT).expect("connect client");
    let error = client
        .recv(SHORT_TIMEOUT)
        .expect_err("quiet pipe must time out");
    assert!(matches!(
        error,
        IpcError::Timeout {
            operation: "named-pipe-prefix",
            ..
        }
    ));
    release_sender.send(()).expect("release server");
    assert_eq!(
        client.recv(TIMEOUT).expect("receive after timeout"),
        heartbeat()
    );
    ack_sender.send(()).expect("ack server");
    client.close().expect("client close");
    server_thread.join().expect("server thread");
}

#[test]
fn test_send_timeout_is_explicit_when_the_peer_does_not_read() {
    let pipe_name = unique_pipe_name();
    let server_pipe_name = pipe_name.clone();
    let (release_sender, release_receiver) = mpsc::channel();
    let server_thread = thread::spawn(move || {
        let mut server = NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
        server.accept(TIMEOUT).expect("accept client");
        release_receiver
            .recv_timeout(TIMEOUT)
            .expect("wait for client send timeout");
        server.close().expect("server close");
    });

    let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
    client.connect(TIMEOUT).expect("connect client");
    let error = client
        .send(&large_request(), SHORT_TIMEOUT)
        .expect_err("unread large frame must time out");
    assert!(matches!(
        error,
        IpcError::Timeout {
            operation: "named-pipe-send",
            ..
        }
    ));
    release_sender.send(()).expect("release server");
    client.close().expect("client close");
    server_thread.join().expect("server thread");
}

#[test]
fn test_peer_process_image_path_rejects_a_nonexistent_pid() {
    let error = peer_process_image_path(u32::MAX).expect_err("nonexistent pid must fail");
    assert!(matches!(error, IpcError::PeerIdentityUnavailable { .. }));
}
