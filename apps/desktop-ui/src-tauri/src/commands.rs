//! Typed Tauri commands exposed to the webview.
//!
//! The webview has zero system permissions and is an untrusted input source, so
//! every command validates its payload before anything else happens: unknown
//! fields, unknown kinds, and version drift are rejected locally, and Core
//! validates the same payload again on its side.
//!
//! Boundaries:
//! - this process does not link Core and never executes a tool;
//! - forwarding happens through an injected [`CoreCommandTransport`], so the
//!   UI process stays a thin, permissionless shell.

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};
use tauri::State;

/// Contract version accepted from the webview.
const SUPPORTED_VERSION: &str = "1.0";

/// Error code for a payload that failed local validation.
pub const UI_COMMAND_INVALID: &str = "ui_command_invalid";

/// Error code for a missing Core transport.
pub const CORE_TRANSPORT_UNAVAILABLE: &str = "core_transport_unavailable";

/// Structured failure raised by a [`CoreCommandTransport`].
///
/// `code` is either [`CORE_TRANSPORT_UNAVAILABLE`] (the UI could not reach
/// Core) or the `ErrorCode` name Core itself reported for a rejection. Keeping
/// the two apart is what lets the webview tell "Core refused this command" from
/// "Core is not reachable" — collapsing them would be a silent failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UiTransportFailure {
    /// Machine-readable code.
    pub code: String,
    /// Human-readable reason.
    pub message: String,
}

impl UiTransportFailure {
    /// Builds an "unreachable" failure.
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: CORE_TRANSPORT_UNAVAILABLE.to_owned(),
            message: message.into(),
        }
    }

    /// Builds a failure carrying `code` (typically an `ErrorCode` name).
    #[must_use]
    pub fn with_code(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Boundary that forwards one validated command to the Core process.
///
/// The UI process never links Core; a real implementation talks to
/// `assistant-core` over the authenticated IPC transport. Until that transport
/// is wired, [`CommandState::unavailable`] makes every forward fail explicitly
/// instead of pretending the command succeeded.
pub trait CoreCommandTransport: Send + Sync + 'static {
    /// Forwards one validated command and returns Core's structured outcome.
    ///
    /// # Errors
    ///
    /// Returns a structured failure when the command cannot be forwarded or
    /// Core rejects it.
    fn send(&self, command: Value) -> Result<Value, UiTransportFailure>;
}

struct UnavailableTransport;

impl CoreCommandTransport for UnavailableTransport {
    fn send(&self, _command: Value) -> Result<Value, UiTransportFailure> {
        Err(UiTransportFailure::unavailable(
            "the Core IPC transport is not wired yet",
        ))
    }
}

/// Shared state holding the Core transport.
pub struct CommandState {
    transport: Arc<dyn CoreCommandTransport>,
}

impl CommandState {
    /// Creates state with no Core transport installed.
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            transport: Arc::new(UnavailableTransport),
        }
    }

    /// Creates state over an injected transport.
    #[must_use]
    pub fn with_transport(transport: Arc<dyn CoreCommandTransport>) -> Self {
        Self { transport }
    }
}

/// Structured rejection returned to the webview.
#[derive(Debug, Serialize)]
pub struct UiCommandRejection {
    /// Stable machine-readable code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

impl UiCommandRejection {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: UI_COMMAND_INVALID.to_owned(),
            message: message.into(),
        }
    }
}

/// Validates and forwards one UI command payload.
///
/// # Errors
///
/// Returns [`UiCommandRejection`] with [`UI_COMMAND_INVALID`] for a malformed
/// payload and with [`CORE_TRANSPORT_UNAVAILABLE`] when the forward fails.
#[tauri::command]
pub fn send_ui_command(
    state: State<'_, CommandState>,
    envelope: Value,
) -> Result<Value, UiCommandRejection> {
    validate_envelope(&envelope)?;
    state
        .transport
        .send(envelope)
        .map_err(|failure| UiCommandRejection {
            code: failure.code,
            message: failure.message,
        })
}

/// Rejects anything the contract does not allow before it leaves the UI process.
fn validate_envelope(value: &Value) -> Result<(), UiCommandRejection> {
    let envelope = value
        .as_object()
        .ok_or_else(|| UiCommandRejection::invalid("the payload must be a JSON object"))?;
    reject_unknown_fields(envelope, &["version", "command"])?;
    let version = envelope
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| UiCommandRejection::invalid("missing contract version"))?;
    if version != SUPPORTED_VERSION {
        return Err(UiCommandRejection::invalid(format!(
            "unsupported contract version {version:?}"
        )));
    }
    let command = envelope
        .get("command")
        .and_then(Value::as_object)
        .ok_or_else(|| UiCommandRejection::invalid("missing the `command` object"))?;
    let kind = command
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| UiCommandRejection::invalid("missing the `kind` discriminator"))?;
    let allowed = allowed_fields(kind)
        .ok_or_else(|| UiCommandRejection::invalid(format!("unknown command kind {kind:?}")))?;
    reject_unknown_fields(command, allowed)
}

fn allowed_fields(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "submit_intent" => Some(&["kind", "intent_id", "goal"]),
        "approve_request" => Some(&["kind", "request_id", "scope"]),
        "deny_request" => Some(&["kind", "request_id", "reason"]),
        "pause_task" | "cancel_task" | "take_over_task" => Some(&["kind", "task_id"]),
        _ => None,
    }
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), UiCommandRejection> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(UiCommandRejection::invalid(format!(
                "unknown field {key:?}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{UI_COMMAND_INVALID, allowed_fields, validate_envelope};

    #[test]
    fn test_valid_envelope_is_accepted() {
        let value = json!({
            "version": "1.0",
            "command": {"kind": "approve_request", "request_id": "a_1", "scope": "once"},
        });
        assert!(validate_envelope(&value).is_ok());
    }

    #[test]
    fn test_unknown_field_is_rejected() {
        let value = json!({
            "version": "1.0",
            "command": {"kind": "pause_task", "task_id": "t_1", "extra": "attacker"},
        });
        let rejection = validate_envelope(&value).err().map(|error| error.code);
        assert_eq!(rejection, Some(UI_COMMAND_INVALID.to_owned()));
    }

    #[test]
    fn test_unknown_kind_and_version_are_rejected() {
        assert!(
            validate_envelope(&json!({
                "version": "1.0",
                "command": {"kind": "run_shell", "command_line": "whoami"},
            }))
            .is_err()
        );
        assert!(
            validate_envelope(&json!({
                "version": "9.9",
                "command": {"kind": "pause_task", "task_id": "t_1"},
            }))
            .is_err()
        );
    }

    #[test]
    fn test_every_kind_has_an_allow_list() {
        for kind in [
            "submit_intent",
            "approve_request",
            "deny_request",
            "pause_task",
            "cancel_task",
            "take_over_task",
        ] {
            assert!(allowed_fields(kind).is_some(), "{kind}");
        }
    }
}
