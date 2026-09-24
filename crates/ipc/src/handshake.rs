//! Authenticated IPC handshake and wire messages.
//!
//! `ClientHello` and `ServerHello` retain the fields required by
//! `docs/spec/ipc-protocol.md`. Authentication is carried beside the
//! `ClientHello` rather than silently extending the contract-defined struct.

use std::fmt;
use std::time::Duration;

use assistant_protocol::{AuditEvent, Capability, ToolEnvelope};
use serde::{Deserialize, Serialize};

use crate::{IpcError, IpcResult, Transport};

/// Current wire protocol version.
pub const IPC_PROTOCOL_VERSION: u32 = 1;
const MIN_TOKEN_LENGTH: usize = 32;
const MAX_TOKEN_LENGTH: usize = 256;

/// Fields defined by the IPC contract's `ClientHello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Peer's protocol version.
    pub version: u32,
    /// Capability identifiers advertised by the client.
    pub capabilities: Vec<Capability>,
}

/// The authentication token travels beside `ClientHello`, not inside it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedClientHello {
    /// Contract-defined client greeting.
    pub hello: ClientHello,
    /// Shared secret supplied out of band by the launching process.
    pub authentication_token: String,
}

impl fmt::Debug for AuthenticatedClientHello {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedClientHello")
            .field("hello", &self.hello)
            .field("authentication_token", &"<redacted>")
            .finish()
    }
}

/// Contract-defined server greeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHello {
    /// Negotiated protocol version.
    pub version: u32,
    /// Capability identifiers accepted by the server.
    pub capabilities: Vec<Capability>,
    /// UUID-formatted session identifier.
    pub session_id: String,
}

/// Contract-defined heartbeat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Session that owns this heartbeat.
    pub session_id: String,
    /// Unix timestamp in milliseconds.
    pub timestamp_unix_ms: u64,
    /// Whether the sender is still serving the session.
    pub alive: bool,
}

/// Request payload. Tool semantics are intentionally owned by TASK-020.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RequestMessage {
    /// Correlation identifier for request/response matching.
    pub correlation_id: String,
    /// Serializable target identifier; never a platform handle.
    pub target: String,
    /// Existing protocol envelope carrying the tool invocation.
    pub tool_invoke: ToolEnvelope,
}

/// Response payload. Tool semantics are intentionally owned by TASK-020.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResponseMessage {
    /// Correlation identifier from the request.
    pub correlation_id: String,
    /// Existing protocol envelope carrying the result or error.
    pub result: ToolEnvelope,
}

/// Fire-and-forget audit event sent by the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AuditEventMessage {
    /// Existing protocol audit-event type.
    pub event: AuditEvent,
}

/// Every payload accepted by the authenticated transport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum WireMessage {
    /// Initial client greeting and authentication material.
    ClientHello(AuthenticatedClientHello),
    /// Server response to a valid client greeting.
    ServerHello(ServerHello),
    /// Liveness signal.
    Heartbeat(Heartbeat),
    /// Tool request; interpretation belongs to TASK-020.
    Request(RequestMessage),
    /// Tool response; interpretation belongs to TASK-020.
    Response(ResponseMessage),
    /// One-way audit event.
    AuditEvent(Box<AuditEventMessage>),
}

impl WireMessage {
    /// Encode this message using the protocol's JSON serialization.
    ///
    /// # Errors
    /// Returns [`IpcError::Serialization`] if serde cannot encode the value.
    pub fn to_json(&self) -> IpcResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| IpcError::Serialization {
            message: error.to_string(),
        })
    }

    /// Decode one JSON wire message.
    ///
    /// # Errors
    /// Returns [`IpcError::Serialization`] for malformed or unknown payloads.
    pub fn from_json(bytes: &[u8]) -> IpcResult<Self> {
        serde_json::from_slice(bytes).map_err(|error| IpcError::Serialization {
            message: error.to_string(),
        })
    }

    /// Stable label used in mismatch diagnostics.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ClientHello(_) => "ClientHello",
            Self::ServerHello(_) => "ServerHello",
            Self::Heartbeat(_) => "Heartbeat",
            Self::Request(_) => "Request",
            Self::Response(_) => "Response",
            Self::AuditEvent(_) => "AuditEvent",
        }
    }
}

/// Server result after successful token and version verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeResult {
    /// Client greeting accepted by the server.
    pub client_hello: ClientHello,
    /// Server greeting sent back to the client.
    pub server_hello: ServerHello,
}

/// Canonical UUID-formatted session identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    /// Parse and validate a canonical UUID-shaped identifier.
    ///
    /// # Errors
    /// Returns [`IpcError::Serialization`] when the shape or hex digits are invalid.
    pub fn parse(value: impl Into<String>) -> IpcResult<Self> {
        let value = value.into();
        if is_canonical_uuid(&value) {
            Ok(Self(value))
        } else {
            Err(IpcError::Serialization {
                message: "session_id is not a canonical UUID".to_string(),
            })
        }
    }

    /// Borrow the canonical string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Validate authentication material before it is sent or compared.
///
/// # Errors
/// Returns [`IpcError::InvalidAuthenticationToken`] for empty, short, long,
/// or whitespace-containing tokens.
pub fn validate_authentication_token(token: &str) -> IpcResult<()> {
    if token.len() < MIN_TOKEN_LENGTH
        || token.len() > MAX_TOKEN_LENGTH
        || token.chars().any(char::is_whitespace)
    {
        Err(IpcError::InvalidAuthenticationToken)
    } else {
        Ok(())
    }
}

/// Compare two byte strings without early exit on the first unequal byte.
#[must_use]
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left_byte, right_byte) in left.iter().zip(right) {
        difference |= left_byte ^ right_byte;
    }
    difference == 0
}

/// Generate a random UUID-v4-shaped session identifier through the OS CSPRNG.
///
/// # Errors
/// Returns a platform error on non-Windows or when the OS random source fails.
#[cfg(windows)]
pub fn generate_session_id() -> IpcResult<SessionId> {
    let mut bytes = [0_u8; 16];
    crate::windows_peer::fill_random(&mut bytes)?;
    if let Some(version_byte) = bytes.get_mut(6) {
        *version_byte = (*version_byte & 0x0F) | 0x40;
    }
    if let Some(variant_byte) = bytes.get_mut(8) {
        *variant_byte = (*variant_byte & 0x3F) | 0x80;
    }
    SessionId::parse(format_uuid(&bytes))
}

/// Generate a random UUID-v4-shaped session identifier through the OS CSPRNG.
///
/// # Errors
/// Always returns [`IpcError::UnsupportedPlatform`] outside Windows.
#[cfg(not(windows))]
pub const fn generate_session_id() -> IpcResult<SessionId> {
    Err(IpcError::UnsupportedPlatform)
}

#[cfg(windows)]
fn format_uuid(bytes: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            output.push('-');
        }
        let high_index = usize::from(byte >> 4);
        let low_index = usize::from(byte & 0x0F);
        if let (Some(high), Some(low)) = (HEX.get(high_index), HEX.get(low_index)) {
            output.push(char::from(*high));
            output.push(char::from(*low));
        }
    }
    output
}

fn is_canonical_uuid(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }
    for (index, byte) in value.bytes().enumerate() {
        let expected_hyphen = matches!(index, 8 | 13 | 18 | 23);
        if expected_hyphen {
            if byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// Perform the client half of the authenticated handshake.
///
/// # Errors
/// Returns a validation, transport, timeout, or protocol-mismatch error.
/// The function never accepts a different message kind as `ServerHello`.
pub fn client_handshake<T: Transport>(
    transport: &mut T,
    authentication_token: &str,
    capabilities: Vec<Capability>,
    timeout: Duration,
) -> IpcResult<ServerHello> {
    validate_authentication_token(authentication_token)?;
    let hello = WireMessage::ClientHello(AuthenticatedClientHello {
        hello: ClientHello {
            version: IPC_PROTOCOL_VERSION,
            capabilities,
        },
        authentication_token: authentication_token.to_string(),
    });
    transport.send(&hello, timeout)?;
    let response = transport.recv(timeout)?;
    let WireMessage::ServerHello(server_hello) = response else {
        return Err(IpcError::UnexpectedMessage {
            expected: "ServerHello",
            actual: response.kind(),
        });
    };
    validate_protocol_version(server_hello.version)?;
    SessionId::parse(server_hello.session_id.clone())?;
    Ok(server_hello)
}

/// Perform the server half of the authenticated handshake.
///
/// # Errors
/// Returns [`IpcError::AuthenticationFailed`] for a bad token and
/// [`IpcError::ProtocolVersionMismatch`] for an unsupported version.
pub fn server_handshake<T: Transport>(
    transport: &mut T,
    expected_authentication_token: &str,
    capabilities: Vec<Capability>,
    session_id: &SessionId,
    timeout: Duration,
) -> IpcResult<HandshakeResult> {
    validate_authentication_token(expected_authentication_token)?;
    let message = transport.recv(timeout)?;
    let WireMessage::ClientHello(authenticated) = message else {
        return Err(IpcError::UnexpectedMessage {
            expected: "ClientHello",
            actual: message.kind(),
        });
    };
    validate_protocol_version(authenticated.hello.version)?;
    validate_authentication_token(&authenticated.authentication_token)?;
    if !constant_time_eq(
        authenticated.authentication_token.as_bytes(),
        expected_authentication_token.as_bytes(),
    ) {
        return Err(IpcError::AuthenticationFailed);
    }
    let server_hello = ServerHello {
        version: IPC_PROTOCOL_VERSION,
        capabilities,
        session_id: session_id.to_string(),
    };
    transport.send(&WireMessage::ServerHello(server_hello.clone()), timeout)?;
    Ok(HandshakeResult {
        client_hello: authenticated.hello,
        server_hello,
    })
}

const fn validate_protocol_version(actual: u32) -> IpcResult<()> {
    if actual == IPC_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(IpcError::ProtocolVersionMismatch {
            expected: IPC_PROTOCOL_VERSION,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        AuthenticatedClientHello, ClientHello, IPC_PROTOCOL_VERSION, SessionId, Transport,
        WireMessage, constant_time_eq, is_canonical_uuid, server_handshake,
        validate_authentication_token,
    };
    use crate::{IpcError, IpcResult};

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[derive(Debug, Default)]
    struct MockTransport {
        next: Option<WireMessage>,
        sent: Vec<WireMessage>,
    }

    impl MockTransport {
        fn with_next(next: WireMessage) -> Self {
            Self {
                next: Some(next),
                sent: Vec::new(),
            }
        }
    }

    impl Transport for MockTransport {
        fn connect(&mut self, _timeout: Duration) -> IpcResult<()> {
            Ok(())
        }

        fn accept(&mut self, _timeout: Duration) -> IpcResult<()> {
            Ok(())
        }

        fn send(&mut self, message: &WireMessage, _timeout: Duration) -> IpcResult<()> {
            self.sent.push(message.clone());
            Ok(())
        }

        fn recv(&mut self, _timeout: Duration) -> IpcResult<WireMessage> {
            self.next.take().ok_or_else(|| IpcError::Disconnected {
                message: "mock transport has no queued message".to_string(),
            })
        }

        fn close(&mut self) -> IpcResult<()> {
            Ok(())
        }

        fn peer_process_id(&self) -> IpcResult<u32> {
            Ok(1)
        }
    }

    #[test]
    fn test_session_id_round_trip() {
        let value = "018f6d4e-52a1-7b03-8f22-1234567890ab";
        let parsed = SessionId::parse(value);
        assert_eq!(
            parsed.map(|session| session.to_string()),
            Ok(value.to_string())
        );
    }

    #[test]
    fn test_session_id_rejects_malformed_value() {
        assert!(matches!(
            SessionId::parse("not-a-uuid"),
            Err(IpcError::Serialization { .. })
        ));
    }

    #[test]
    fn test_canonical_uuid_checks_positions() {
        assert!(is_canonical_uuid("018f6d4e-52a1-7b03-8f22-1234567890ab"));
        assert!(!is_canonical_uuid("018f6d4e52a1-7b03-8f22-1234567890ab"));
        assert!(!is_canonical_uuid("018f6d4e-52a1-7b03-8f22-1234567890ag"));
    }

    #[test]
    fn test_token_validation_accepts_strong_shape() {
        assert!(validate_authentication_token(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn test_token_validation_rejects_short_or_spaced_values() {
        assert!(matches!(
            validate_authentication_token("short"),
            Err(IpcError::InvalidAuthenticationToken)
        ));
        assert!(matches!(
            validate_authentication_token(&"a ".repeat(32)),
            Err(IpcError::InvalidAuthenticationToken)
        ));
    }

    #[test]
    fn test_constant_time_equality() {
        assert!(constant_time_eq(b"same", b"same"));
        assert!(!constant_time_eq(b"same", b"diff"));
        assert!(!constant_time_eq(b"short", b"longer"));
    }

    #[test]
    fn test_server_handshake_rejects_version_mismatch() {
        let mut transport =
            MockTransport::with_next(WireMessage::ClientHello(AuthenticatedClientHello {
                hello: ClientHello {
                    version: IPC_PROTOCOL_VERSION + 1,
                    capabilities: Vec::new(),
                },
                authentication_token: TOKEN.to_string(),
            }));
        let session_id = SessionId::parse("018f6d4e-52a1-7b03-8f22-1234567890ab");
        assert!(session_id.is_ok());
        let Ok(session_id) = session_id else {
            return;
        };
        assert!(matches!(
            server_handshake(
                &mut transport,
                TOKEN,
                Vec::new(),
                &session_id,
                Duration::from_secs(1),
            ),
            Err(IpcError::ProtocolVersionMismatch { .. })
        ));
    }

    #[test]
    fn test_server_handshake_rejects_missing_token() {
        let mut transport =
            MockTransport::with_next(WireMessage::ClientHello(AuthenticatedClientHello {
                hello: ClientHello {
                    version: IPC_PROTOCOL_VERSION,
                    capabilities: Vec::new(),
                },
                authentication_token: String::new(),
            }));
        let session_id = SessionId::parse("018f6d4e-52a1-7b03-8f22-1234567890ab");
        assert!(session_id.is_ok());
        let Ok(session_id) = session_id else {
            return;
        };
        assert!(matches!(
            server_handshake(
                &mut transport,
                TOKEN,
                Vec::new(),
                &session_id,
                Duration::from_secs(1),
            ),
            Err(IpcError::InvalidAuthenticationToken)
        ));
    }
}
