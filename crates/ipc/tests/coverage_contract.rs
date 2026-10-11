//! Public contract coverage for IPC framing, handshake, and `NamedPipe` transport.
//!
//! These tests use real Windows `NamedPipe` instances because the highest-risk
//! branches live in the overlapped I/O and peer-identity paths. They never read
//! a user desktop, network service, credential store, or GUI application.

#![cfg(windows)]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use assistant_ipc::{
    AuthenticatedClientHello, ClientHello, Heartbeat, IPC_PROTOCOL_VERSION, IpcError,
    NamedPipeTransport, ServerHello, SessionId, Transport, WireMessage, client_handshake,
    generate_session_id, peer_process_image_path, server_handshake, validate_authentication_token,
};
use assistant_protocol::ErrorCategory;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const TIMEOUT: Duration = Duration::from_secs(5);
const SESSION: &str = "018f6d4e-52a1-7b03-8f22-1234567890ab";
static NEXT_PIPE_ID: AtomicU64 = AtomicU64::new(1);

fn unique_pipe_name() -> String {
    let sequence = NEXT_PIPE_ID.fetch_add(1, Ordering::Relaxed);
    format!("assistant-ipc-coverage-{}-{sequence}", std::process::id())
}

fn session_id() -> SessionId {
    SessionId::parse(SESSION).expect("coverage session id must be canonical")
}

fn heartbeat() -> WireMessage {
    WireMessage::Heartbeat(Heartbeat {
        session_id: SESSION.to_string(),
        timestamp_unix_ms: 1_000,
        alive: true,
    })
}

fn authenticated_hello(version: u32, token: &str) -> WireMessage {
    WireMessage::ClientHello(AuthenticatedClientHello {
        hello: ClientHello {
            version,
            capabilities: Vec::new(),
        },
        authentication_token: token.to_string(),
    })
}

#[test]
fn test_named_pipe_round_trip_and_peer_identity() {
    let pipe_name = unique_pipe_name();
    let server_pipe_name = pipe_name.clone();
    let (ack_sender, ack_receiver) = mpsc::channel();
    let server_thread = thread::spawn(move || {
        let mut server = NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
        server.accept(TIMEOUT).expect("accept client");
        assert_eq!(
            server.peer_process_id().expect("peer pid"),
            std::process::id()
        );
        assert_eq!(server.recv(TIMEOUT).expect("server receive"), heartbeat());
        server.send(&heartbeat(), TIMEOUT).expect("server send");
        ack_receiver.recv_timeout(TIMEOUT).expect("client ack");
        server.close().expect("server close");
    });

    let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
    client.connect(TIMEOUT).expect("connect client");
    assert!(matches!(
        client.peer_process_id(),
        Err(IpcError::PeerIdentityUnavailable { .. })
    ));
    client.send(&heartbeat(), TIMEOUT).expect("client send");
    assert_eq!(client.recv(TIMEOUT).expect("client receive"), heartbeat());
    ack_sender.send(()).expect("send client ack");
    client.close().expect("client close");
    server_thread.join().expect("server thread");
}

#[test]
fn test_authenticated_handshake_succeeds_over_named_pipe() {
    let pipe_name = unique_pipe_name();
    let expected_session = session_id();
    let server_pipe_name = pipe_name.clone();
    let (ack_sender, ack_receiver) = mpsc::channel();
    let server_thread = thread::spawn(move || {
        let mut server = NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
        server.accept(TIMEOUT).expect("accept client");
        let handshake =
            server_handshake(&mut server, TOKEN, Vec::new(), &expected_session, TIMEOUT)
                .expect("server handshake");
        assert_eq!(handshake.server_hello.session_id, SESSION);
        assert_eq!(handshake.client_hello.version, IPC_PROTOCOL_VERSION);
        ack_receiver.recv_timeout(TIMEOUT).expect("client ack");
        server.close().expect("server close");
    });

    let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
    client.connect(TIMEOUT).expect("connect client");
    let server_hello =
        client_handshake(&mut client, TOKEN, Vec::new(), TIMEOUT).expect("client handshake");
    assert_eq!(server_hello.session_id, SESSION);
    assert_eq!(server_hello.version, IPC_PROTOCOL_VERSION);
    ack_sender.send(()).expect("send client ack");
    client.close().expect("client close");
    server_thread.join().expect("server thread");
}

#[test]
fn test_client_handshake_rejects_wrong_kind_and_version() {
    for message in [
        heartbeat(),
        WireMessage::ServerHello(ServerHello {
            version: IPC_PROTOCOL_VERSION + 1,
            capabilities: Vec::new(),
            session_id: SESSION.to_string(),
        }),
    ] {
        let pipe_name = unique_pipe_name();
        let server_pipe_name = pipe_name.clone();
        let (release_sender, release_receiver) = mpsc::channel();
        let server_thread = thread::spawn(move || {
            let mut server =
                NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
            server.accept(TIMEOUT).expect("accept client");
            server
                .send(&message, TIMEOUT)
                .expect("send malformed response");
            release_receiver
                .recv_timeout(TIMEOUT)
                .expect("release malformed server");
            server.close().expect("server close");
        });

        let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
        client.connect(TIMEOUT).expect("connect client");
        let error = client_handshake(&mut client, TOKEN, Vec::new(), TIMEOUT)
            .expect_err("handshake must reject malformed response");
        assert!(matches!(
            error,
            IpcError::UnexpectedMessage { .. } | IpcError::ProtocolVersionMismatch { .. }
        ));
        release_sender.send(()).expect("release malformed server");
        client.close().expect("client close");
        server_thread.join().expect("server thread");
    }
}

#[test]
fn test_server_handshake_rejects_wrong_kind_and_token() {
    let cases = [
        (heartbeat(), TOKEN, "unexpected"),
        (
            authenticated_hello(IPC_PROTOCOL_VERSION, TOKEN),
            "abcdef0123456789abcdef0123456789",
            "auth",
        ),
    ];
    for (message, expected_token, expected_error) in cases {
        let pipe_name = unique_pipe_name();
        let expected_session = session_id();
        let token = expected_token.to_string();
        let server_pipe_name = pipe_name.clone();
        let server_thread = thread::spawn(move || {
            let mut server =
                NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
            server.accept(TIMEOUT).expect("accept client");
            let error =
                server_handshake(&mut server, &token, Vec::new(), &expected_session, TIMEOUT)
                    .expect_err("server handshake must reject invalid client");
            match expected_error {
                "unexpected" => assert!(matches!(error, IpcError::UnexpectedMessage { .. })),
                "auth" => assert!(matches!(error, IpcError::AuthenticationFailed)),
                _ => unreachable!("test case label"),
            }
        });

        let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
        client.connect(TIMEOUT).expect("connect client");
        client.send(&message, TIMEOUT).expect("send client message");
        server_thread.join().expect("server thread");
        client.close().expect("client close");
    }
}

#[test]
fn test_named_pipe_disconnect_is_explicit() {
    let pipe_name = unique_pipe_name();
    let (accepted_sender, accepted_receiver) = mpsc::channel();
    let server_pipe_name = pipe_name.clone();
    let server_thread = thread::spawn(move || {
        let mut server = NamedPipeTransport::server(&server_pipe_name).expect("create server pipe");
        server.accept(TIMEOUT).expect("accept client");
        accepted_sender.send(()).expect("signal accepted client");
        let error = server.recv(TIMEOUT).expect_err("closed client must fail");
        assert!(matches!(error, IpcError::Disconnected { .. }));
    });

    let mut client = NamedPipeTransport::client(&pipe_name).expect("create client");
    client.connect(TIMEOUT).expect("connect client");
    accepted_receiver
        .recv_timeout(TIMEOUT)
        .expect("server accepted client");
    client.close().expect("client close");
    server_thread.join().expect("server thread");
}

#[test]
fn test_named_pipe_connect_timeout_and_role_misuse_are_explicit() {
    let mut missing_client =
        NamedPipeTransport::client(&unique_pipe_name()).expect("create missing client");
    let timeout = missing_client
        .connect(Duration::from_millis(50))
        .expect_err("missing server must time out");
    assert!(matches!(timeout, IpcError::Timeout { .. }));

    let mut server = NamedPipeTransport::server(&unique_pipe_name()).expect("create server pipe");
    assert!(matches!(
        server.connect(TIMEOUT),
        Err(IpcError::Transport { .. })
    ));
    assert!(matches!(
        server.peer_process_id(),
        Err(IpcError::PeerIdentityUnavailable { .. })
    ));

    let mut client = NamedPipeTransport::client(&unique_pipe_name()).expect("create client");
    assert!(matches!(
        client.accept(TIMEOUT),
        Err(IpcError::Transport { .. })
    ));
    assert!(matches!(
        client.recv(TIMEOUT),
        Err(IpcError::Transport { .. })
    ));
}

#[test]
fn test_session_randomness_and_peer_image_path_are_real() {
    let first = generate_session_id().expect("first session id");
    let second = generate_session_id().expect("second session id");
    assert_ne!(first, second);
    assert_eq!(first.as_str().len(), 36);

    let image_path =
        peer_process_image_path(std::process::id()).expect("current process image path");
    assert!(!image_path.trim().is_empty());
}

#[test]
fn test_authentication_token_bounds_are_stable() {
    assert!(validate_authentication_token(TOKEN).is_ok());
    assert!(matches!(
        validate_authentication_token(&"a".repeat(31)),
        Err(IpcError::InvalidAuthenticationToken)
    ));
    assert!(matches!(
        validate_authentication_token(&"a".repeat(257)),
        Err(IpcError::InvalidAuthenticationToken)
    ));
}

#[test]
fn test_ipc_error_categories_and_display_are_stable() {
    let cases = [
        frame_error_cases(),
        permission_error_cases(),
        runtime_error_cases(),
    ]
    .concat();

    for (error, expected_category) in cases {
        assert_eq!(error.code(), expected_category);
        assert!(!error.to_string().trim().is_empty());
    }
}

fn frame_error_cases() -> Vec<(IpcError, ErrorCategory)> {
    vec![
        (
            IpcError::MagicMismatch { received: 0 },
            ErrorCategory::ToolInvalidArgs,
        ),
        (
            IpcError::EnvelopeTooLarge {
                size: 1,
                maximum: 0,
            },
            ErrorCategory::ToolInvalidArgs,
        ),
        (
            IpcError::FrameLengthMismatch {
                expected: 1,
                actual: 2,
            },
            ErrorCategory::ToolInvalidArgs,
        ),
        (
            IpcError::ChecksumMismatch {
                expected: 1,
                actual: 2,
            },
            ErrorCategory::VerifyFailed,
        ),
        (
            IpcError::ProtocolVersionMismatch {
                expected: 1,
                actual: 2,
            },
            ErrorCategory::ToolInvalidArgs,
        ),
        (
            IpcError::UnexpectedMessage {
                expected: "ServerHello",
                actual: "Heartbeat",
            },
            ErrorCategory::ToolInvalidArgs,
        ),
        (
            IpcError::Serialization {
                message: "bad json".to_string(),
            },
            ErrorCategory::ToolInvalidArgs,
        ),
    ]
}

fn permission_error_cases() -> Vec<(IpcError, ErrorCategory)> {
    vec![
        (
            IpcError::InvalidAuthenticationToken,
            ErrorCategory::PlatformPermission,
        ),
        (
            IpcError::AuthenticationFailed,
            ErrorCategory::PlatformPermission,
        ),
        (
            IpcError::PeerIdentityUnavailable {
                message: "unavailable".to_string(),
            },
            ErrorCategory::PlatformPermission,
        ),
        (
            IpcError::PeerIdentityRejected {
                image_path: "peer.exe".to_string(),
            },
            ErrorCategory::PlatformPermission,
        ),
    ]
}

fn runtime_error_cases() -> Vec<(IpcError, ErrorCategory)> {
    vec![
        (
            IpcError::timeout("coverage", Duration::from_millis(7)),
            ErrorCategory::TargetUnresponsive,
        ),
        (
            IpcError::Disconnected {
                message: "closed".to_string(),
            },
            ErrorCategory::TargetUnresponsive,
        ),
        (
            IpcError::UnsupportedPlatform,
            ErrorCategory::CapabilityMissing,
        ),
        (
            IpcError::Transport {
                message: "failed".to_string(),
            },
            ErrorCategory::Transient,
        ),
    ]
}
