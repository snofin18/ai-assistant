//! UI↔Core wire envelopes (ADR-0057 D2 / D5).
//!
//! 这一层只负责**传输形状**：correlation、单向事件、以及"拒绝必须带 ErrorCode"的信封。
//! 载荷语义属于 binary 层：Rust 侧是 `apps/agent-core/src/ui_ipc.rs`，TypeScript 侧是 zod 契约。
//! 与工具形状的 `RequestMessage` / `ResponseMessage` 严格分离（ADR-0057 D2）。

use serde::{Deserialize, Serialize};

/// UI → Core command envelope (ADR-0057 D2).
///
/// The payload is carried as **opaque JSON** on purpose: `crates/ipc` must not
/// depend on the binary-layer UI contract (that lives in
/// `apps/agent-core/src/ui_ipc.rs`). This crate owns the *transport shape*
/// (correlation + framing); the *semantics and validation* stay with the two
/// sides that already implement them (Rust allow-list on the Core side, zod on
/// the UI side). Reusing the tool-shaped [`RequestMessage`] here was rejected by
/// ADR-0057 D2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UiIpcRequest {
    /// Correlation identifier the matching response must echo.
    pub correlation_id: String,
    /// `UiCommandEnvelope` as JSON; validated by the Core side.
    pub command: serde_json::Value,
}

impl UiIpcRequest {
    /// Creates a UI command envelope.
    #[must_use]
    pub fn new(correlation_id: impl Into<String>, command: serde_json::Value) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            command,
        }
    }
}

/// Result carried by [`UiIpcResponse`].
///
/// Why a result enum instead of putting an error variant into the UI command
/// outcome: the outcome type is owned by the binary-layer UI contract
/// (`apps/agent-core/src/ui_ipc.rs`), while *rejections* are a transport-layer
/// concern. Keeping them here means the UI contract keeps one job (describing
/// accepted commands) and every rejection still carries a stable `ErrorCode`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum UiIpcResult {
    /// The command was accepted and applied; payload is a `UiCommandOutcome`.
    Outcome {
        /// `UiCommandOutcome` as JSON; validated by the UI side.
        outcome: serde_json::Value,
    },
    /// The command was rejected before any side effect.
    Rejected {
        /// Stable `ErrorCode` name (e.g. `ToolInvalidArgs`).
        code: String,
        /// Human-readable reason.
        message: String,
    },
}

/// Core → UI response envelope (ADR-0057 D5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UiIpcResponse {
    /// Correlation identifier echoed from the request.
    pub correlation_id: String,
    /// Outcome or rejection.
    pub result: UiIpcResult,
}

impl UiIpcResponse {
    /// Creates a successful UI response envelope.
    #[must_use]
    pub fn outcome(correlation_id: impl Into<String>, outcome: serde_json::Value) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            result: UiIpcResult::Outcome { outcome },
        }
    }

    /// Creates a rejected UI response envelope.
    #[must_use]
    pub fn rejected(
        correlation_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            result: UiIpcResult::Rejected {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

/// Core → UI one-way event envelope (ADR-0057 D5: no correlation id).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UiIpcEvent {
    /// `UiEvent` as JSON; validated by the UI side.
    pub event: serde_json::Value,
}

impl UiIpcEvent {
    /// Creates a UI event envelope.
    #[must_use]
    pub const fn new(event: serde_json::Value) -> Self {
        Self { event }
    }
}

#[cfg(test)]
mod tests {
    use crate::handshake::WireMessage;

    // --- ADR-0057：UI↔Core 专属 wire 信封 ---

    #[test]
    fn test_ui_wire_envelope_round_trips() {
        let command = serde_json::json!({
            "version": "1.0",
            "command": { "kind": "pause_task", "task_id": "t_1" }
        });
        let request = WireMessage::UiRequest(super::UiIpcRequest::new("c_1", command));
        let Ok(bytes) = request.to_json() else {
            return;
        };
        assert_eq!(WireMessage::from_json(&bytes), Ok(request));
    }

    #[test]
    fn test_ui_wire_kinds_are_distinct_from_tool_message_kinds() {
        let ui_request =
            WireMessage::UiRequest(super::UiIpcRequest::new("c_1", serde_json::json!({})));
        let ui_response =
            WireMessage::UiResponse(super::UiIpcResponse::outcome("c_1", serde_json::json!({})));
        let ui_event = WireMessage::UiEvent(super::UiIpcEvent::new(serde_json::json!({})));
        assert_eq!(ui_request.kind(), "UiRequest");
        assert_eq!(ui_response.kind(), "UiResponse");
        assert_eq!(ui_event.kind(), "UiEvent");
        // ADR-0057 D2 的机器判据：UI 信封的 kind 不得与工具形状信封重名
        for tool_kind in ["Request", "Response", "AuditEvent"] {
            assert_ne!(ui_request.kind(), tool_kind);
            assert_ne!(ui_response.kind(), tool_kind);
            assert_ne!(ui_event.kind(), tool_kind);
        }
    }

    #[test]
    fn test_ui_wire_payload_stays_opaque_json() {
        // 载荷是"不透明 JSON"：crates/ipc 不认识 UiCommand 的字段，只搬字节。
        let request = WireMessage::UiRequest(super::UiIpcRequest::new(
            "c_2",
            serde_json::json!({ "future_field": { "nested": [1, 2, 3] } }),
        ));
        let Ok(bytes) = request.to_json() else {
            return;
        };
        assert_eq!(WireMessage::from_json(&bytes), Ok(request));
    }

    #[test]
    fn test_ui_rejection_carries_an_error_code() {
        // 拒绝必须带稳定 ErrorCode，并与"成功但载荷为空"在类型上可区分。
        let wire = WireMessage::UiResponse(super::UiIpcResponse::rejected(
            "c_9",
            "ToolInvalidArgs",
            "unknown field `approved_by`",
        ));
        let Ok(bytes) = wire.to_json() else {
            return;
        };
        let Ok(WireMessage::UiResponse(decoded)) = WireMessage::from_json(&bytes) else {
            return;
        };
        assert_eq!(decoded.correlation_id, "c_9");
        assert!(matches!(
            decoded.result,
            super::UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
        ));
    }
}
