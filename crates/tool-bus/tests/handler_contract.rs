//! Public contract coverage for handler-side call identity and tool output metadata.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use assistant_tool_bus::{
    CallContext, EvidenceDescriptor, SourceDescriptor, ToolBusResult, ToolHandler, ToolOutput,
};
use serde_json::{Map, Value, json};

#[test]
fn test_call_context_exposes_exact_identity() {
    let context = CallContext::new("task-299", String::from("step-1"));

    assert_eq!(context.task_id(), "task-299");
    assert_eq!(context.step_id(), "step-1");
}

#[test]
fn test_closure_handler_receives_context_and_arguments() {
    let handler = |context: &CallContext, arguments: &Map<String, Value>| {
        Ok(ToolOutput::json(json!({
            "task_id": context.task_id(),
            "step_id": context.step_id(),
            "argument_count": arguments.len(),
        })))
    };
    let context = CallContext::new("task-299", "step-2");
    let mut arguments = Map::new();
    arguments.insert("value".to_string(), Value::String("present".to_string()));

    let output = ToolHandler::call(&handler, &context, &arguments).expect("closure handler");
    assert_eq!(output.max_bytes(), None);
    assert_eq!(output.source(), None);
    assert_eq!(output.evidence(), None);
    assert!(!output.is_untrusted());
}

#[test]
fn test_tool_output_json_starts_without_trust_metadata_or_budget() {
    let output = ToolOutput::json(json!({"kind": "trusted"}));

    assert!(!output.is_untrusted());
    assert_eq!(output.source(), None);
    assert_eq!(output.evidence(), None);
    assert_eq!(output.max_bytes(), None);
}

#[test]
fn test_tool_output_untrusted_exposes_its_source() {
    let source = SourceDescriptor::app_content("com.example.notepad").with_target("doc-299");
    let output = ToolOutput::untrusted(source.clone(), json!({"kind": "app-content"}));

    assert!(output.is_untrusted());
    assert_eq!(output.source(), Some(&source));
    assert_eq!(output.evidence(), None);
    assert_eq!(output.max_bytes(), None);
}

#[test]
fn test_tool_output_builders_attach_evidence_and_budget() {
    let evidence = EvidenceDescriptor::from_snapshot("snapshot-299");
    let output = ToolOutput::json(Value::Null)
        .with_evidence(evidence.clone())
        .with_max_bytes(2048);

    assert_eq!(output.evidence(), Some(&evidence));
    assert_eq!(output.max_bytes(), Some(2048));
    assert_eq!(output.source(), None);
    assert!(!output.is_untrusted());
}

#[test]
fn test_tool_output_untrusted_builders_compose() {
    let source = SourceDescriptor::app_content("com.example.paint");
    let evidence = EvidenceDescriptor::from_snapshot("paint-snapshot");
    let output = ToolOutput::untrusted(source.clone(), json!({"kind": "paint-content"}))
        .with_evidence(evidence.clone())
        .with_max_bytes(0);

    assert!(output.is_untrusted());
    assert_eq!(output.source(), Some(&source));
    assert_eq!(output.evidence(), Some(&evidence));
    assert_eq!(output.max_bytes(), Some(0));
}

#[test]
fn test_handler_result_can_propagate_a_tool_error() {
    let handler = |_context: &CallContext, _arguments: &Map<String, Value>| {
        Err::<ToolOutput, _>(assistant_tool_bus::ToolBusError::UnknownTool {
            tool: "missing.tool".to_string(),
        })
    };
    let context = CallContext::new("task-299", "step-3");

    let result: ToolBusResult<ToolOutput> = ToolHandler::call(&handler, &context, &Map::new());
    assert!(result.is_err());
}
