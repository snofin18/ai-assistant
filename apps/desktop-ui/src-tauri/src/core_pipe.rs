//! NamedPipe client for the Core-side UI listener (ADR-0057 D3 / D4 / D7).
//!
//! Responsibilities:
//! - connect to the Core UI pipe, authenticate with the one-shot token and run
//!   the handshake;
//! - send one `UiIpcRequest` per UI command and match the `UiIpcResponse` by
//!   `correlation_id`;
//! - translate Core's structured rejection into a machine-readable failure.
//!
//! Boundaries:
//! - does not parse UI command semantics (the webview's zod boundary and Core's
//!   own allow-list already do that);
//! - does not link Core, `assistant-policy`, or `assistant-task-engine`;
//! - never retries a command automatically — a retry could duplicate a side
//!   effect (ADR-0057 spec §7 不变量 4).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use assistant_ipc::{
    IpcError, NamedPipeTransport, Transport, UiIpcRequest, UiIpcResult, WireMessage,
    client_handshake,
};
use serde_json::Value;

use crate::commands::{CoreCommandTransport, UiTransportFailure};

/// How to reach the Core UI listener.
#[derive(Debug, Clone)]
pub struct CorePipeConfig {
    /// Pipe component without the `\\.\pipe\` prefix.
    pub pipe_name: String,
    /// Environment variable holding Core's one-shot token.
    pub token_environment_variable: String,
    /// Deadline for connect + handshake.
    pub connect_timeout: Duration,
    /// Deadline for one request/response exchange.
    pub request_timeout: Duration,
}

/// Real transport over the authenticated UI pipe.
pub struct CorePipeTransport {
    config: CorePipeConfig,
    sequence: AtomicU64,
}

impl CorePipeTransport {
    /// Creates a transport for one Core pipe.
    #[must_use]
    pub const fn new(config: CorePipeConfig) -> Self {
        Self {
            config,
            sequence: AtomicU64::new(0),
        }
    }

    fn next_correlation_id(&self) -> String {
        let next = self
            .sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        format!("ui_{next}")
    }
}

impl CorePipeTransport {
    /// Connects and authenticates one session.
    fn open_session(&self) -> Result<NamedPipeTransport, UiTransportFailure> {
        let token = std::env::var(&self.config.token_environment_variable).map_err(|_| {
            UiTransportFailure::unavailable(format!(
                "{} is not set",
                self.config.token_environment_variable
            ))
        })?;
        let mut transport = NamedPipeTransport::client(&self.config.pipe_name)
            .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
        transport
            .connect(self.config.connect_timeout)
            .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
        client_handshake(
            &mut transport,
            &token,
            Vec::new(),
            self.config.connect_timeout,
        )
        .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
        Ok(transport)
    }

    /// Subscribes to Core's one-way event stream until Core closes it.
    ///
    /// An idle stream is normal (Core heartbeats on the same pipe), so a read
    /// timeout just keeps waiting; a disconnect ends the subscription cleanly.
    ///
    /// # Errors
    ///
    /// Fails when connect/handshake fails, the peer sends a non-event message,
    /// or `on_event` rejects an event.
    pub fn subscribe<F>(&self, mut on_event: F) -> Result<(), UiTransportFailure>
    where
        F: FnMut(Value) -> Result<(), UiTransportFailure>,
    {
        let mut transport = self.open_session()?;
        loop {
            match transport.recv(self.config.request_timeout) {
                Ok(message) => on_event(interpret_event(message)?)?,
                Err(IpcError::Disconnected { .. }) => return Ok(()),
                Err(IpcError::Timeout { .. }) => continue,
                Err(error) => return Err(UiTransportFailure::unavailable(error.to_string())),
            }
        }
    }
}

impl CoreCommandTransport for CorePipeTransport {
    fn send(&self, envelope: Value) -> Result<Value, UiTransportFailure> {
        let mut transport = self.open_session()?;
        let correlation_id = self.next_correlation_id();
        let request = WireMessage::UiRequest(UiIpcRequest::new(correlation_id.clone(), envelope));
        transport
            .send(&request, self.config.request_timeout)
            .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
        let response = transport
            .recv(self.config.request_timeout)
            .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
        interpret_response(&correlation_id, response)
    }
}

/// Maps one Core reply onto the call result.
///
/// Split out (and made `pub(crate)`) so the correlation / rejection logic is
/// unit-testable without a live pipe.
///
/// # Errors
///
/// Returns [`UiTransportFailure`] for a wrong message kind, a mismatched
/// correlation id, or Core's own rejection code.
pub(crate) fn interpret_response(
    expected_correlation_id: &str,
    message: WireMessage,
) -> Result<Value, UiTransportFailure> {
    let WireMessage::UiResponse(response) = message else {
        return Err(UiTransportFailure::unavailable(format!(
            "unexpected {} on the UI command channel",
            message.kind()
        )));
    };
    if response.correlation_id != expected_correlation_id {
        return Err(UiTransportFailure::unavailable(
            "response correlation_id does not match the request",
        ));
    }
    match response.result {
        UiIpcResult::Outcome { outcome } => Ok(outcome),
        UiIpcResult::Rejected { code, message } => {
            Err(UiTransportFailure::with_code(code, message))
        }
        // `UiIpcResult` is `#[non_exhaustive]`: a future variant must fail closed
        // rather than be silently treated as success.
        _ => Err(UiTransportFailure::unavailable(
            "unsupported UI response variant",
        )),
    }
}

/// Maps one message from the event stream onto the event payload.
///
/// The payload stays opaque JSON here; the webview's zod boundary validates it
/// before it reaches the timeline model.
///
/// # Errors
///
/// Fails for any non-`UiEvent` message: a tool-shaped frame or a stray response
/// on the event channel must never be rendered.
pub(crate) fn interpret_event(message: WireMessage) -> Result<Value, UiTransportFailure> {
    match message {
        WireMessage::UiEvent(event) => Ok(event.event),
        other => Err(UiTransportFailure::unavailable(format!(
            "unexpected {} on the UI event channel",
            other.kind()
        ))),
    }
}
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use assistant_ipc::{UiIpcResponse, WireMessage};
    use serde_json::json;

    use super::{CorePipeConfig, CorePipeTransport, interpret_event, interpret_response};
    use crate::commands::{CoreCommandTransport, UiTransportFailure};

    fn config() -> CorePipeConfig {
        CorePipeConfig {
            pipe_name: "assistant-ui-test".to_owned(),
            token_environment_variable: "ASSISTANT_UI_TEST_TOKEN".to_owned(),
            connect_timeout: Duration::from_millis(100),
            request_timeout: Duration::from_millis(100),
        }
    }

    #[test]
    fn test_outcome_is_returned_verbatim() {
        let result = interpret_response(
            "ui_1",
            WireMessage::UiResponse(UiIpcResponse::outcome(
                "ui_1",
                json!({ "status": "intent_accepted", "intent_id": "i_1" }),
            )),
        );
        assert_eq!(
            result,
            Ok(json!({ "status": "intent_accepted", "intent_id": "i_1" }))
        );
    }

    #[test]
    fn test_rejection_keeps_cores_error_code() {
        let result = interpret_response(
            "ui_2",
            WireMessage::UiResponse(UiIpcResponse::rejected(
                "ui_2",
                "ToolInvalidArgs",
                "unknown field",
            )),
        );
        assert_eq!(
            result,
            Err(UiTransportFailure::with_code(
                "ToolInvalidArgs",
                "unknown field"
            ))
        );
    }

    #[test]
    fn test_mismatched_correlation_is_rejected() {
        let result = interpret_response(
            "ui_3",
            WireMessage::UiResponse(UiIpcResponse::outcome("ui_9", json!({}))),
        );
        assert!(matches!(result, Err(ref failure) if failure.code == "core_transport_unavailable"));
    }

    #[test]
    fn test_wrong_message_kind_is_rejected() {
        // 非 UI 响应的任何消息走这条通道都必须显式失败，而不是被当成正常结果。
        let result = interpret_response(
            "ui_4",
            WireMessage::Heartbeat(assistant_ipc::Heartbeat {
                session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
                timestamp_unix_ms: 1,
                alive: true,
            }),
        );
        assert!(matches!(result, Err(ref failure) if failure.code == "core_transport_unavailable"));
    }

    #[test]
    fn test_missing_token_fails_before_touching_the_pipe() {
        let transport = CorePipeTransport::new(config());
        let result = transport.send(json!({ "version": "1.0", "command": {} }));
        assert!(matches!(result, Err(ref failure) if failure.code == "core_transport_unavailable"));
    }

    #[test]
    fn test_correlation_ids_are_monotonic() {
        let transport = CorePipeTransport::new(config());
        assert_eq!(transport.next_correlation_id(), "ui_1");
        assert_eq!(transport.next_correlation_id(), "ui_2");
    }

    #[test]
    fn test_ui_event_payload_is_passed_through() {
        let payload = json!({ "kind": "task_state_changed", "task_id": "t_1" });
        let result = interpret_event(WireMessage::UiEvent(assistant_ipc::UiIpcEvent::new(
            payload.clone(),
        )));
        assert_eq!(result, Ok(payload));
    }

    #[test]
    fn test_non_event_message_on_event_channel_fails_closed() {
        let result = interpret_event(WireMessage::Heartbeat(assistant_ipc::Heartbeat {
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
            timestamp_unix_ms: 1,
            alive: true,
        }));
        assert!(matches!(
            result,
            Err(ref failure) if failure.code == "core_transport_unavailable"
        ));
    }

    #[test]
    fn test_subscribe_without_token_fails_before_connecting() {
        let transport = CorePipeTransport::new(config());
        let result = transport.subscribe(|_| Ok(()));
        assert!(matches!(
            result,
            Err(ref failure) if failure.code == "core_transport_unavailable"
        ));
    }
}
