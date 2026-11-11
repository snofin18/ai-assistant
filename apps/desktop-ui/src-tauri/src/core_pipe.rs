//! NamedPipe client for the Core-side UI listener (ADR-0057 D3 / D4 / D7).
//!
//! Responsibilities:
//! - own the stable configuration for the Core UI pipe;
//! - expose one transport handle backed by a single authenticated session;
//! - translate Core's structured rejection into a machine-readable failure.
//!
//! Boundaries:
//! - does not parse UI command semantics (the webview's zod boundary and Core's
//!   own allow-list already do that);
//! - does not link Core, `assistant-policy`, or `assistant-task-engine`;
//! - never retries a command automatically, because a retry could duplicate a side
//!   effect (ADR-0057 spec §7 不变量 4).

use std::sync::Arc;
use std::time::Duration;

use assistant_ipc::{UiIpcResult, WireMessage};
use serde_json::Value;

use crate::commands::{CoreCommandTransport, UiTransportFailure};
use crate::core_pipe_session::CorePipeSession;

/// Environment variable selecting the Core UI pipe name.
pub const UI_PIPE_ENVIRONMENT_VARIABLE: &str = "ASSISTANT_UI_PIPE";

/// Environment variable selecting the environment variable that contains the token.
pub const UI_TOKEN_ENVIRONMENT_VARIABLE: &str = "ASSISTANT_UI_TOKEN_ENV";

/// Default pipe name shared with Core production and browser UI listeners.
pub const DEFAULT_UI_PIPE_NAME: &str = "assistant-agent-core-ui";

/// Default token environment variable shared with Core production listeners.
pub const DEFAULT_UI_TOKEN_ENVIRONMENT_VARIABLE: &str = "ASSISTANT_AGENT_CORE_UI_TOKEN";

/// Stable failure code for invalid desktop UI transport configuration.
pub const CORE_TRANSPORT_CONFIG_INVALID: &str = "core_transport_config_invalid";

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

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

impl CorePipeConfig {
    /// Builds configuration from the desktop UI process environment.
    ///
    /// `ASSISTANT_UI_PIPE` and `ASSISTANT_UI_TOKEN_ENV` are optional and default
    /// to the values used by Core. The selected token environment variable must
    /// be present and non-empty. Error messages name the variable but never
    /// include its value.
    ///
    /// # Errors
    ///
    /// Returns [`CORE_TRANSPORT_CONFIG_INVALID`] when the pipe name is empty,
    /// the token variable name is empty, or the token value is missing/empty.
    pub fn from_environment() -> Result<Self, UiTransportFailure> {
        let pipe_name = environment_value(UI_PIPE_ENVIRONMENT_VARIABLE)
            .unwrap_or_else(|| DEFAULT_UI_PIPE_NAME.to_owned());
        let token_environment_variable = environment_value(UI_TOKEN_ENVIRONMENT_VARIABLE)
            .unwrap_or_else(|| DEFAULT_UI_TOKEN_ENVIRONMENT_VARIABLE.to_owned());
        let token_value = std::env::var(&token_environment_variable).ok();
        Self::from_values(
            Some(&pipe_name),
            Some(&token_environment_variable),
            token_value.as_deref(),
        )
    }

    fn from_values(
        pipe_name: Option<&str>,
        token_environment_variable: Option<&str>,
        token_value: Option<&str>,
    ) -> Result<Self, UiTransportFailure> {
        let pipe_name = normalize_required_value(
            pipe_name.unwrap_or(DEFAULT_UI_PIPE_NAME),
            UI_PIPE_ENVIRONMENT_VARIABLE,
            "pipe name",
        )?;
        let token_environment_variable = normalize_required_value(
            token_environment_variable.unwrap_or(DEFAULT_UI_TOKEN_ENVIRONMENT_VARIABLE),
            UI_TOKEN_ENVIRONMENT_VARIABLE,
            "token environment variable name",
        )?;
        if !matches!(token_value, Some(value) if !value.is_empty()) {
            return Err(config_error(format!(
                "{token_environment_variable} must contain the one-shot UI token"
            )));
        }
        Ok(Self {
            pipe_name,
            token_environment_variable,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        })
    }
}

fn environment_value(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn normalize_required_value(
    value: &str,
    environment_variable: &str,
    field: &str,
) -> Result<String, UiTransportFailure> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(config_error(format!(
            "{environment_variable} must contain a non-empty {field}"
        )));
    }
    Ok(normalized.to_owned())
}

fn config_error(message: impl Into<String>) -> UiTransportFailure {
    UiTransportFailure::with_code(CORE_TRANSPORT_CONFIG_INVALID, message)
}

/// Real transport over the authenticated UI pipe.
pub struct CorePipeTransport {
    session: Arc<CorePipeSession>,
}

impl CorePipeTransport {
    /// Creates a transport for one Core pipe.
    #[must_use]
    pub fn new(config: CorePipeConfig) -> Self {
        Self {
            session: Arc::new(CorePipeSession::new(config, None)),
        }
    }
}

impl CorePipeTransport {
    /// Subscribes to Core's event stream on the shared authenticated session.
    ///
    /// Events, command responses, and heartbeats are demultiplexed by the same
    /// session worker. A clean close after the human decision ends this method
    /// successfully; an unexpected disconnect is reported as a failure.
    ///
    /// # Errors
    ///
    /// Fails when connect/handshake fails, the peer sends a non-event message,
    /// or `on_event` rejects an event.
    pub fn subscribe<F>(&self, on_event: F) -> Result<(), UiTransportFailure>
    where
        F: FnMut(Value) -> Result<(), UiTransportFailure>,
    {
        self.session.subscribe(on_event)
    }
}

impl CoreCommandTransport for CorePipeTransport {
    fn send(&self, envelope: Value) -> Result<Value, UiTransportFailure> {
        self.session.send(envelope)
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use assistant_ipc::{UiIpcResponse, WireMessage};
    use serde_json::json;

    use super::{
        CORE_TRANSPORT_CONFIG_INVALID, CorePipeConfig, CorePipeTransport, DEFAULT_UI_PIPE_NAME,
        DEFAULT_UI_TOKEN_ENVIRONMENT_VARIABLE, interpret_response,
    };
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
    fn test_core_pipe_config_uses_core_compatible_defaults() {
        let config =
            CorePipeConfig::from_values(None, None, Some("test-token")).expect("default config");
        assert_eq!(config.pipe_name, DEFAULT_UI_PIPE_NAME);
        assert_eq!(
            config.token_environment_variable,
            DEFAULT_UI_TOKEN_ENVIRONMENT_VARIABLE
        );
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(2));
    }

    #[test]
    fn test_core_pipe_config_accepts_custom_pipe_and_token_variable() {
        let config = CorePipeConfig::from_values(
            Some("assistant-custom-ui"),
            Some("ASSISTANT_CUSTOM_UI_TOKEN"),
            Some("test-token"),
        )
        .expect("custom config");
        assert_eq!(config.pipe_name, "assistant-custom-ui");
        assert_eq!(
            config.token_environment_variable,
            "ASSISTANT_CUSTOM_UI_TOKEN"
        );
    }

    #[test]
    fn test_core_pipe_config_rejects_empty_pipe_name() {
        let error = CorePipeConfig::from_values(Some("  "), None, Some("test-token"))
            .expect_err("empty pipe must fail");
        assert_eq!(error.code, CORE_TRANSPORT_CONFIG_INVALID);
    }

    #[test]
    fn test_core_pipe_config_rejects_empty_token_variable_name() {
        let error = CorePipeConfig::from_values(None, Some(" "), Some("test-token"))
            .expect_err("empty token variable must fail");
        assert_eq!(error.code, CORE_TRANSPORT_CONFIG_INVALID);
    }

    #[test]
    fn test_core_pipe_config_rejects_missing_or_empty_token_value() {
        let missing =
            CorePipeConfig::from_values(None, None, None).expect_err("missing token must fail");
        let empty =
            CorePipeConfig::from_values(None, None, Some("")).expect_err("empty token must fail");
        assert_eq!(missing.code, CORE_TRANSPORT_CONFIG_INVALID);
        assert_eq!(empty.code, CORE_TRANSPORT_CONFIG_INVALID);
        assert!(!missing.message.contains("test-token"));
        assert!(!empty.message.contains("test-token"));
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
    fn test_subscribe_without_token_fails_before_connecting() {
        let transport = CorePipeTransport::new(config());
        let result = transport.subscribe(|_| Ok(()));
        assert!(matches!(
            result,
            Err(ref failure) if failure.code == "core_transport_unavailable"
        ));
    }
}
