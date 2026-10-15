//! Public contract coverage for the in-process MCP server boundary.
//!
//! These cases intentionally go through [`ToolBus`] instead of calling server internals so the
//! envelope behaviour is proved across the same MCP round trip production uses.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use assistant_protocol::{ErrorCode, TruncationReason};
use assistant_tool_bus::{
    CallContext, EvidenceDescriptor, MountSelection, ToolBus, ToolBusConfig, ToolBusError,
    ToolHandler, ToolOutput, ToolRegistry,
};
use common::{arguments, clock, demo_definition, echo_definition, payload, register};
use serde_json::{Map, Value, json};

/// A handler error is still a tool-level failure envelope, not a transport failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_error_returns_failure_envelope_with_error_code() {
    let handler: Arc<dyn ToolHandler> = Arc::new(
        |_call: &CallContext,
         _arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            Err(ToolBusError::ToolNotMounted {
                tool: "demo.fail.run".to_owned(),
            })
        },
    );

    let mut registry = ToolRegistry::new();
    register(
        &mut registry,
        demo_definition("fail", "returns a tool-level failure"),
        handler,
    );

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("server-handler-error"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let envelope = bus
        .call_tool(
            "demo.fail.run",
            arguments(json!({})),
            &CallContext::new("task-error", "step-error"),
        )
        .await
        .expect("a handler error must be returned as an envelope");

    assert!(!envelope.ok);
    assert_eq!(envelope.tool, "demo.fail.run");
    assert_eq!(envelope.task_id, "task-error");
    assert_eq!(envelope.step_id, "step-error");
    assert!(envelope.data.is_none());
    let error = envelope
        .error
        .as_ref()
        .expect("a failed envelope must carry an error");
    assert_eq!(error.code, ErrorCode::TargetNotFound);
    assert!(
        error.message.contains("not mounted"),
        "error message must preserve the handler reason: {:?}",
        error.message
    );

    bus.shutdown().await.expect("bus must shut down");
}

/// Evidence attached by a handler must survive the MCP round trip unchanged.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_success_evidence_round_trips_through_mcp() {
    let handler: Arc<dyn ToolHandler> = Arc::new(
        |_call: &CallContext,
         arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            let evidence = EvidenceDescriptor {
                snapshot_id: Some("snapshot-301".to_owned()),
                screenshot: Some("blob://screenshot-301".to_owned()),
                extra: Some(json!({ "confidence": 0.97 })),
            };
            Ok(ToolOutput::json(json!({
                "echoed": arguments.get("text").cloned().unwrap_or(Value::Null),
            }))
            .with_evidence(evidence))
        },
    );

    let mut registry = ToolRegistry::new();
    register(&mut registry, echo_definition("returns evidence"), handler);

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("server-evidence"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let envelope = bus
        .call_tool(
            "demo.echo.echo",
            arguments(json!({ "text": "evidence" })),
            &CallContext::new("task-evidence", "step-evidence"),
        )
        .await
        .expect("tools/call must return an envelope");

    assert!(envelope.ok);
    assert_eq!(payload(&envelope), &json!({ "echoed": "evidence" }));
    let evidence = envelope
        .evidence
        .as_ref()
        .expect("evidence must survive the MCP round trip");
    assert_eq!(evidence.snapshot_id.as_deref(), Some("snapshot-301"));
    assert_eq!(
        evidence.screenshot.as_deref(),
        Some("blob://screenshot-301")
    );
    assert_eq!(
        evidence
            .extra
            .as_ref()
            .and_then(|extra| extra.get("confidence")),
        Some(&json!(0.97))
    );

    bus.shutdown().await.expect("bus must shut down");
}

/// The server-side default budget must be applied when a handler declares no override.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_default_max_bytes_is_applied_by_server() {
    let handler: Arc<dyn ToolHandler> = Arc::new(
        |_call: &CallContext,
         _arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            Ok(ToolOutput::json(json!({
                "alpha": "A".repeat(4000),
                "beta": "B".repeat(10),
            })))
        },
    );

    let mut registry = ToolRegistry::new();
    register(
        &mut registry,
        echo_definition("returns a payload larger than the default budget"),
        handler,
    );

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("server-default-budget").with_default_max_bytes(256),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let envelope = bus
        .call_tool(
            "demo.echo.echo",
            arguments(json!({ "text": "budget" })),
            &CallContext::new("task-budget", "step-budget"),
        )
        .await
        .expect("tools/call must return an envelope");

    assert!(envelope.ok);
    let truncated = envelope
        .truncated
        .as_ref()
        .expect("the default budget must truncate an oversized payload");
    assert!(truncated.occurred);
    assert_eq!(truncated.reason, TruncationReason::MaxBytes);
    let delivered_bytes = serde_json::to_vec(payload(&envelope))
        .expect("truncated payload must serialize")
        .len();
    assert!(
        delivered_bytes <= 256,
        "truncated payload is {delivered_bytes} bytes, over the default 256-byte budget"
    );

    bus.shutdown().await.expect("bus must shut down");
}
