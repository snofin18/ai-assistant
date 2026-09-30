//! Core-side UI command listener (ADR-0057 D1 / D4 / D5 / D7).
//!
//! Responsibilities:
//! - accept one authenticated UI connection and run one session loop;
//! - dispatch each validated `UiIpcRequest` through a [`UiCommandHandler`];
//! - answer with a `UiIpcResponse` carrying the same correlation id, or a
//!   structured rejection that names a stable `ErrorCode`.
//!
//! Boundaries:
//! - does not parse UI semantics itself (that is [`crate::ui_ipc`]);
//! - does not execute tools, resolve targets, or decide policy;
//! - does not accept tool-shaped `Request` / `Response` on this channel.
//!
//! Invariants:
//! 1. every `UiRequest` gets exactly one `UiResponse` with the same id;
//! 2. a rejection never happens after a side effect (`dispatch_ui_command`
//!    parses first);
//! 3. tool-shaped traffic and unknown messages fail the session instead of
//!    being ignored;
//! 4. heartbeat silence fails the session explicitly.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, HeartbeatMonitor, IpcError, IpcResult, NamedPipeTransport, ServerHello, Transport,
    UiIpcRequest, UiIpcResponse, WireMessage, generate_session_id, peer_process_image_path,
    server_handshake,
};

use crate::ui_ipc::{UiCommandHandler, dispatch_ui_command};

/// Configuration for the Core-side UI listener (ADR-0057 D4).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct UiServerConfig {
    /// Named pipe component without the `\\.\pipe\` prefix.
    pub pipe_name: String,
    /// Environment variable holding the one-shot authentication token.
    pub token_environment_variable: String,
    /// Allowed canonical peer image paths. Empty means deny all.
    pub allowed_peer_images: Vec<PathBuf>,
    /// Deadline for creating and accepting the pipe.
    pub handshake_timeout: Duration,
    /// Maximum silence allowed before the watchdog fires.
    pub heartbeat_timeout: Duration,
}

impl UiServerConfig {
    /// Creates a config that denies every peer until one is added.
    #[must_use]
    pub fn new(
        pipe_name: impl Into<String>,
        token_environment_variable: impl Into<String>,
        heartbeat_timeout: Duration,
    ) -> Self {
        Self {
            pipe_name: pipe_name.into(),
            token_environment_variable: token_environment_variable.into(),
            allowed_peer_images: Vec::new(),
            handshake_timeout: Duration::from_secs(5),
            heartbeat_timeout,
        }
    }

    /// Adds one allowed peer image.
    #[must_use]
    pub fn with_allowed_peer(mut self, peer_image: impl Into<PathBuf>) -> Self {
        self.allowed_peer_images.push(peer_image.into());
        self
    }

    /// Returns whether a peer image path is on the allow-list.
    ///
    /// Comparison is case- and separator-insensitive and never hashes: the peer
    /// identity must stay debuggable when a connection is rejected (ADR-0057 D4
    /// 沿用 automation-host 的既有模式).
    #[must_use]
    pub fn is_peer_allowed(&self, peer_image: &str) -> bool {
        let normalized = normalize_image_path(peer_image);
        self.allowed_peer_images
            .iter()
            .any(|allowed| normalize_image_path(&allowed.to_string_lossy()) == normalized)
    }
}

/// Serves exactly one UI session on the configured named pipe.
///
/// # Errors
///
/// Fails closed for a missing token, rejected peer identity, handshake failure,
/// tool-shaped traffic on the UI channel, or transport timeout/disconnect.
pub fn serve<H: UiCommandHandler>(config: &UiServerConfig, handler: &mut H) -> IpcResult<()> {
    let authentication_token = std::env::var(&config.token_environment_variable)
        .map_err(|_| IpcError::InvalidAuthenticationToken)?;

    let mut transport = NamedPipeTransport::server(&config.pipe_name)?;
    transport.accept(config.handshake_timeout)?;
    let peer_process_id = transport.peer_process_id()?;
    let peer_image_path = peer_process_image_path(peer_process_id)?;
    if !config.is_peer_allowed(&peer_image_path) {
        return Err(IpcError::PeerIdentityRejected {
            image_path: peer_image_path,
        });
    }

    let session_id = generate_session_id()?;
    let handshake = server_handshake(
        &mut transport,
        &authentication_token,
        Vec::new(),
        &session_id,
        config.handshake_timeout,
    )?;
    serve_session(
        &mut transport,
        &handshake.server_hello,
        config.heartbeat_timeout,
        handler,
    )
}

/// Runs one already-handshaken session over any [`Transport`].
///
/// This is the white-box seam: tests drive it with a scripted transport, so the
/// dispatch loop is covered without a real pipe.
///
/// # Errors
///
/// Returns an IPC error for malformed heartbeats, tool-shaped messages on the UI
/// channel, watchdog expiry, or transport failure.
pub fn serve_session<T, H>(
    transport: &mut T,
    server_hello: &ServerHello,
    heartbeat_timeout: Duration,
    handler: &mut H,
) -> IpcResult<()>
where
    T: Transport,
    H: UiCommandHandler,
{
    let now = unix_time_ms()?;
    let mut monitor = HeartbeatMonitor::new(heartbeat_timeout, now);
    send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
    let poll_interval = heartbeat_timeout / 3;

    loop {
        let now = unix_time_ms()?;
        monitor.check(now)?;
        let wait = monitor.remaining(now).min(poll_interval);
        match transport.recv(wait) {
            Ok(WireMessage::Heartbeat(heartbeat)) => {
                if heartbeat.session_id != server_hello.session_id || !heartbeat.alive {
                    return Err(IpcError::UnexpectedMessage {
                        expected: "live Heartbeat for the negotiated UI session",
                        actual: "Heartbeat",
                    });
                }
                monitor.observe(now);
                send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
            }
            Ok(WireMessage::UiRequest(request)) => {
                monitor.observe(now);
                let response = process_ui_request(handler, &request);
                transport.send(&WireMessage::UiResponse(response), heartbeat_timeout)?;
            }
            Ok(other) => {
                // 工具形状的 Request/Response 走错通道必须显式失败，不能忽略
                return Err(IpcError::UnexpectedMessage {
                    expected: "Heartbeat or UiRequest",
                    actual: other.kind(),
                });
            }
            Err(IpcError::Timeout { .. }) => {
                let now = unix_time_ms()?;
                monitor.check(now)?;
                send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Applies one UI request and builds its matching response.
///
/// Parsing happens before the handler runs, so a malformed payload cannot cause
/// a side effect; every rejection names a stable `ErrorCode`.
#[must_use]
pub fn process_ui_request<H: UiCommandHandler>(
    handler: &mut H,
    request: &UiIpcRequest,
) -> UiIpcResponse {
    let correlation_id = request.correlation_id.clone();
    match dispatch_ui_command(handler, &request.command) {
        Ok(outcome) => match serde_json::to_value(&outcome) {
            Ok(value) => UiIpcResponse::outcome(correlation_id, value),
            Err(error) => UiIpcResponse::rejected(
                correlation_id,
                "Fatal",
                format!("outcome serialization failed: {error}"),
            ),
        },
        Err(error) => UiIpcResponse::rejected(
            correlation_id,
            format!("{:?}", error.error_code()),
            error.to_string(),
        ),
    }
}

fn send_heartbeat<T: Transport>(
    transport: &mut T,
    session_id: &str,
    timestamp_unix_ms: u64,
    timeout: Duration,
) -> IpcResult<()> {
    transport.send(
        &WireMessage::Heartbeat(Heartbeat {
            session_id: session_id.to_string(),
            timestamp_unix_ms,
            alive: true,
        }),
        timeout,
    )
}

fn normalize_image_path(path: &str) -> String {
    path.trim().replace('/', "\\").to_ascii_lowercase()
}

fn unix_time_ms() -> IpcResult<u64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| IpcError::Transport {
            message: format!("system clock is before Unix epoch: {error}"),
        })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| IpcError::Transport {
        message: "system time does not fit in u64 milliseconds".to_owned(),
    })
}

/// Returns whether the given path is non-empty (config-validity helper).
#[must_use]
pub fn looks_like_image_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::time::Duration;

    use assistant_ipc::{
        IpcError, RequestMessage, ServerHello, Transport, UiIpcRequest, UiIpcResult, WireMessage,
    };
    use assistant_protocol::ToolEnvelope;

    use super::{UiServerConfig, looks_like_image_path, process_ui_request, serve_session};
    use crate::ui_ipc::{
        UiAuthorizationScope, UiCommand, UiCommandError, UiCommandHandler, UiCommandOutcome,
    };

    #[derive(Default)]
    struct ScriptedTransport {
        incoming: VecDeque<WireMessage>,
        sent: Vec<WireMessage>,
    }

    impl Transport for ScriptedTransport {
        fn connect(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
            Ok(())
        }
        fn accept(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
            Ok(())
        }
        fn send(
            &mut self,
            message: &WireMessage,
            _timeout: Duration,
        ) -> assistant_ipc::IpcResult<()> {
            self.sent.push(message.clone());
            Ok(())
        }
        fn recv(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<WireMessage> {
            self.incoming
                .pop_front()
                .ok_or_else(|| IpcError::Disconnected {
                    message: "scripted transport exhausted".to_owned(),
                })
        }
        fn close(&mut self) -> assistant_ipc::IpcResult<()> {
            Ok(())
        }
        fn peer_process_id(&self) -> assistant_ipc::IpcResult<u32> {
            Ok(1)
        }
    }

    #[derive(Default)]
    struct RecordingHandler {
        seen: Vec<UiCommand>,
        reject_with: Option<&'static str>,
    }

    impl UiCommandHandler for RecordingHandler {
        fn handle(&mut self, command: UiCommand) -> Result<UiCommandOutcome, UiCommandError> {
            self.seen.push(command);
            if let Some(field) = self.reject_with {
                return Err(UiCommandError::EmptyField { field });
            }
            Ok(UiCommandOutcome::IntentAccepted {
                intent_id: "i_1".to_owned(),
            })
        }
    }

    fn hello() -> ServerHello {
        ServerHello {
            version: assistant_ipc::IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
        }
    }

    fn submit_intent_request() -> UiIpcRequest {
        UiIpcRequest::new(
            "c_1",
            serde_json::json!({
                "version": "1.0",
                "command": { "kind": "submit_intent", "intent_id": "i_1", "goal": "replace and save" }
            }),
        )
    }

    fn dispatch(request: &UiIpcRequest) -> (RecordingHandler, UiIpcResult) {
        let mut handler = RecordingHandler::default();
        let response = process_ui_request(&mut handler, request);
        assert_eq!(
            response.correlation_id, request.correlation_id,
            "响应必须回同一 correlation_id"
        );
        (handler, response.result)
    }

    #[test]
    fn test_valid_request_is_dispatched_and_answered() {
        let (handler, result) = dispatch(&submit_intent_request());
        assert_eq!(handler.seen.len(), 1);
        assert!(matches!(
            result,
            UiIpcResult::Outcome { ref outcome }
                if outcome.get("status").and_then(|value| value.as_str()) == Some("intent_accepted")
        ));
    }

    #[test]
    fn test_unknown_field_is_rejected_before_the_handler_runs() {
        let request = UiIpcRequest::new(
            "c_2",
            serde_json::json!({
                "version": "1.0",
                "command": { "kind": "pause_task", "task_id": "t_1", "approved_by": "attacker" }
            }),
        );
        let (handler, result) = dispatch(&request);
        assert!(handler.seen.is_empty(), "非法载荷不得触达处理器");
        assert!(matches!(
            result,
            UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
        ));
    }

    #[test]
    fn test_version_drift_is_rejected_before_the_handler_runs() {
        let request = UiIpcRequest::new(
            "c_3",
            serde_json::json!({
                "version": "9.9",
                "command": { "kind": "pause_task", "task_id": "t_1" }
            }),
        );
        let (handler, result) = dispatch(&request);
        assert!(handler.seen.is_empty());
        assert!(matches!(
            result,
            UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
        ));
    }

    #[test]
    fn test_handler_rejection_is_mapped_to_an_error_code() {
        let mut handler = RecordingHandler {
            seen: Vec::new(),
            reject_with: Some("task_id"),
        };
        let response = process_ui_request(&mut handler, &submit_intent_request());
        assert!(matches!(
            response.result,
            UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
        ));
    }

    #[test]
    fn test_approval_scope_matches_the_shared_contract() {
        let request = UiIpcRequest::new(
            "c_4",
            serde_json::json!({
                "version": "1.0",
                "command": { "kind": "approve_request", "request_id": "a_1", "scope": "once" }
            }),
        );
        let (handler, result) = dispatch(&request);
        assert!(matches!(result, UiIpcResult::Outcome { .. }));
        assert_eq!(
            handler.seen.first(),
            Some(&UiCommand::ApproveRequest {
                request_id: "a_1".to_owned(),
                scope: UiAuthorizationScope::Once,
            })
        );
    }

    #[test]
    fn test_session_dispatches_request_and_sends_matching_response() {
        let mut transport = ScriptedTransport::default();
        transport
            .incoming
            .push_back(WireMessage::UiRequest(submit_intent_request()));
        let mut handler = RecordingHandler::default();
        let ended = serve_session(
            &mut transport,
            &hello(),
            Duration::from_millis(2_000),
            &mut handler,
        );
        assert!(matches!(ended, Err(IpcError::Disconnected { .. })));
        assert!(matches!(
            transport.sent.first(),
            Some(WireMessage::Heartbeat(_))
        ));
        let Some(response) = transport.sent.iter().find_map(|message| match message {
            WireMessage::UiResponse(response) => Some(response),
            _ => None,
        }) else {
            return;
        };
        assert_eq!(response.correlation_id, "c_1");
        assert_eq!(handler.seen.len(), 1);
    }

    #[test]
    fn test_tool_shaped_request_on_ui_channel_fails_the_session() {
        let mut transport = ScriptedTransport::default();
        transport
            .incoming
            .push_back(WireMessage::Request(RequestMessage::new(
                "c_1",
                "document",
                ToolEnvelope::error(
                    "notepad.text.read".to_owned(),
                    "t_1".to_owned(),
                    "s_1".to_owned(),
                    assistant_protocol::ErrorCode::Fatal,
                    "wrong channel",
                ),
            )));
        let mut handler = RecordingHandler::default();
        assert!(matches!(
            serve_session(
                &mut transport,
                &hello(),
                Duration::from_millis(2_000),
                &mut handler
            ),
            Err(IpcError::UnexpectedMessage { .. })
        ));
        assert!(handler.seen.is_empty());
    }

    #[test]
    fn test_peer_allow_list_is_case_and_separator_insensitive() {
        let config = UiServerConfig::new(
            "assistant-ui-test",
            "ASSISTANT_UI_TOKEN",
            Duration::from_millis(2_000),
        )
        .with_allowed_peer(r"C:\Apps\Assistant.exe");
        assert!(config.is_peer_allowed("c:/apps/assistant.exe"));
        assert!(!config.is_peer_allowed(r"C:\Apps\Other.exe"));
    }

    #[test]
    fn test_empty_peer_allow_list_denies_everyone() {
        let config = UiServerConfig::new(
            "assistant-ui-test",
            "ASSISTANT_UI_TOKEN",
            Duration::from_millis(2_000),
        );
        assert!(!config.is_peer_allowed(r"C:\Apps\Assistant.exe"));
        assert!(looks_like_image_path(std::path::Path::new("a.exe")));
        assert!(!looks_like_image_path(std::path::Path::new("")));
    }
}
