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
    MAX_ENVELOPE_SIZE, NamedPipeTransport, RequestMessage, ResponseMessage, ServerHello, SessionId,
    Transport, WireMessage, client_handshake, decode_frame, encode_frame, generate_session_id,
    peer_process_image_path, server_handshake, validate_authentication_token,
};
use assistant_protocol::{ErrorCategory, ToolEnvelope};

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

#[derive(Debug, Default)]
struct MockTransport {
    next: Option<WireMessage>,
    sent: Vec<WireMessage>,
}

impl MockTransport {
    const fn with_next(next: WireMessage) -> Self {
        Self {
            next: Some(next),
            sent: Vec::new(),
        }
    }

    fn empty() -> Self {
        Self::default()
    }
}

impl Transport for MockTransport {
    fn connect(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
        self.next = None;
        Ok(())
    }

    fn accept(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
        self.sent.clear();
        Ok(())
    }

    fn send(&mut self, message: &WireMessage, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
        self.sent.push(message.clone());
        Ok(())
    }

    fn recv(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<WireMessage> {
        self.next.take().ok_or_else(|| IpcError::Disconnected {
            message: "mock transport has no queued message".to_string(),
        })
    }

    fn close(&mut self) -> assistant_ipc::IpcResult<()> {
        self.next = None;
        Ok(())
    }

    fn peer_process_id(&self) -> assistant_ipc::IpcResult<u32> {
        Ok(1)
    }
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
fn test_authenticated_client_hello_debug_redacts_token() {
    let hello = AuthenticatedClientHello {
        hello: ClientHello {
            version: IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
        },
        authentication_token: TOKEN.to_string(),
    };
    let rendered = format!("{hello:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains(TOKEN));
}

#[test]
fn test_request_and_response_constructors_preserve_correlation_and_payload() {
    let envelope = ToolEnvelope::ok(
        "paint.canvas.draw".to_string(),
        "task-273".to_string(),
        "step-1".to_string(),
        serde_json::json!({"shape": "rectangle"}),
    );
    let request = RequestMessage::new("correlation-1", "paint-window", envelope.clone());
    assert_eq!(request.correlation_id, "correlation-1");
    assert_eq!(request.target, "paint-window");
    assert_eq!(request.tool_invoke, envelope);

    let response = ResponseMessage::new("correlation-1", envelope);
    assert_eq!(response.correlation_id, "correlation-1");
    assert_eq!(response.result.tool, "paint.canvas.draw");
    assert_eq!(WireMessage::Request(request).kind(), "Request");
    assert_eq!(WireMessage::Response(response).kind(), "Response");
}

#[test]
fn test_wire_message_invalid_json_is_a_serialization_error() {
    let error = WireMessage::from_json(b"not-json").expect_err("malformed JSON must fail");
    assert!(matches!(error, IpcError::Serialization { .. }));
}

#[test]
fn test_client_handshake_rejects_invalid_local_token_before_transport() {
    let mut transport = MockTransport::empty();
    let error = client_handshake(&mut transport, "short", Vec::new(), TIMEOUT)
        .expect_err("short local token must fail before send");
    assert!(matches!(error, IpcError::InvalidAuthenticationToken));
    assert!(transport.sent.is_empty());
}

#[test]
fn test_client_handshake_validates_server_version_and_session_id() {
    let cases = [
        (
            WireMessage::ServerHello(ServerHello {
                version: IPC_PROTOCOL_VERSION + 1,
                capabilities: Vec::new(),
                session_id: SESSION.to_string(),
            }),
            "version",
        ),
        (
            WireMessage::ServerHello(ServerHello {
                version: IPC_PROTOCOL_VERSION,
                capabilities: Vec::new(),
                session_id: "not-a-canonical-uuid".to_string(),
            }),
            "session",
        ),
    ];

    for (message, expected_error) in cases {
        let mut transport = MockTransport::with_next(message);
        let error = client_handshake(&mut transport, TOKEN, Vec::new(), TIMEOUT)
            .expect_err("malformed server response must fail");
        match expected_error {
            "version" => assert!(matches!(error, IpcError::ProtocolVersionMismatch { .. })),
            "session" => assert!(matches!(error, IpcError::Serialization { .. })),
            _ => unreachable!("test case label"),
        }
        assert!(matches!(
            transport.sent.first(),
            Some(WireMessage::ClientHello(_))
        ));
    }
}

#[test]
fn test_server_handshake_rejects_invalid_local_token_before_receive() {
    let mut transport = MockTransport::empty();
    let session = session_id();
    let error = server_handshake(&mut transport, "short", Vec::new(), &session, TIMEOUT)
        .expect_err("short expected token must fail before receive");
    assert!(matches!(error, IpcError::InvalidAuthenticationToken));
    assert!(transport.sent.is_empty());
}

#[test]
fn test_frame_encoder_rejects_oversized_envelope() {
    let oversized = vec![0_u8; MAX_ENVELOPE_SIZE + 1];
    let error = encode_frame(&oversized).expect_err("oversized envelope must fail");
    assert!(matches!(error, IpcError::EnvelopeTooLarge { .. }));
}

#[test]
fn test_decode_frame_rejects_truncated_prefixes() {
    for length in [0_usize, 1, 7] {
        let bytes = vec![0_u8; length];
        let error = decode_frame(&bytes).expect_err("truncated frame must fail");
        assert!(matches!(error, IpcError::FrameLengthMismatch { .. }));
    }

    let error = decode_frame(&[0_u8; 11]).expect_err("zero magic must fail");
    assert!(matches!(error, IpcError::MagicMismatch { .. }));
}

#[test]
fn test_peer_process_image_path_rejects_missing_process() {
    let error = peer_process_image_path(0).expect_err("pid zero must not resolve");
    assert!(matches!(error, IpcError::PeerIdentityUnavailable { .. }));
}

#[test]
fn test_unconnected_named_pipe_send_fails_explicitly() {
    let mut client = NamedPipeTransport::client(&unique_pipe_name()).expect("create client");
    let error = client
        .send(&heartbeat(), TIMEOUT)
        .expect_err("unconnected client must fail");
    assert!(matches!(error, IpcError::Transport { .. }));
    client.close().expect("close unconnected client");
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
