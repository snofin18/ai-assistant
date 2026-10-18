//! Private white-box coverage for `bus.rs` failure boundaries.
//!
//! The public `ToolBus` happy path is exercised by `tests/tool_bus.rs` and
//! `tests/server_contract.rs`. The remaining lines sit behind private conversion
//! helpers: malformed MCP results, service-error mapping, and transport failures.
//! Keep these tests at the module boundary, with no new public API or production
//! behavior change.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::*;
use assistant_protocol::ToolEnvelope;
use rmcp::model::{CallToolResult, ContentBlock, ErrorCode as RmcpErrorCode, ErrorData};
use serde_json::json;

fn sample_envelope() -> ToolEnvelope {
    ToolEnvelope::ok(
        "demo.echo.echo".to_owned(),
        "task-316".to_owned(),
        "step-316".to_owned(),
        json!({ "echo": "ok" }),
    )
}

#[test]
fn test_extract_envelope_prefers_structured_content() {
    let envelope = sample_envelope();
    let mut result = CallToolResult::success(Vec::new());
    result.structured_content =
        Some(serde_json::to_value(&envelope).expect("envelope must serialize"));

    let extracted =
        extract_envelope("demo.echo.echo", &result).expect("structured envelope must parse");

    assert_eq!(extracted, envelope);
}

#[test]
fn test_extract_envelope_rejects_malformed_structured_content() {
    let mut result = CallToolResult::success(Vec::new());
    result.structured_content = Some(json!({ "not": "an-envelope" }));

    let error =
        extract_envelope("demo.echo.echo", &result).expect_err("malformed envelope must fail");

    assert!(matches!(error, ToolBusError::EnvelopeAssembly { .. }));
}

#[test]
fn test_extract_envelope_falls_back_to_first_parseable_text_block() {
    let envelope = sample_envelope();
    let encoded = serde_json::to_string(&envelope).expect("envelope must serialize");
    let result = CallToolResult::success(vec![
        ContentBlock::text("not-json"),
        ContentBlock::text(encoded),
    ]);

    let extracted = extract_envelope("demo.echo.echo", &result).expect("text fallback must parse");

    assert_eq!(extracted, envelope);
}

#[test]
fn test_extract_envelope_fails_closed_without_envelope_payload() {
    let result = CallToolResult::default();

    let error =
        extract_envelope("demo.echo.echo", &result).expect_err("missing envelope must fail");

    assert!(matches!(
        error,
        ToolBusError::EnvelopeMissing { tool } if tool == "demo.echo.echo"
    ));
}

#[test]
fn test_map_service_error_preserves_mcp_code_and_message() {
    let error = rmcp::ServiceError::McpError(ErrorData::new(
        RmcpErrorCode::INVALID_PARAMS,
        "invalid arguments",
        None,
    ));

    let mapped = map_service_error(error);

    let ToolBusError::Mcp { code, message } = mapped else {
        panic!("MCP errors must keep the MCP branch");
    };
    assert_eq!(code, RmcpErrorCode::INVALID_PARAMS.0);
    assert!(message.contains("invalid arguments"));
}

#[test]
fn test_map_service_error_treats_non_mcp_errors_as_transport() {
    let mapped = map_service_error(rmcp::ServiceError::TransportClosed);

    let ToolBusError::Transport { reason } = mapped else {
        panic!("transport failures must map to the transport branch");
    };
    assert!(reason.contains("Transport closed"));
}

#[test]
fn test_transport_failure_preserves_display_text() {
    let mapped = transport_failure("socket closed");

    let ToolBusError::Transport { reason } = mapped else {
        panic!("transport_failure must map to the transport branch");
    };
    assert_eq!(reason, "socket closed");
}
