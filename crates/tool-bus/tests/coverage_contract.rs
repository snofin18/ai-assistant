//! 公共契约覆盖率：参数校验、信封组装、错误分类与挂载选择边界。
//!
//! 这些用例只调用 crate 已公开的 API，不触碰生产实现细节；目标是让注册期无法强制、
//! 但调用期必须 fail-closed 的分支都有可回归证据。
//!
//! 测试 lint 例外仅由本文件顶部一行显式放开（AGENTS.md §5.3）。

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod common;

use assistant_protocol::{ErrorCode, SourceKind, TruncationReason};
use assistant_tool_bus::{
    EnvelopeRequest, EvidenceDescriptor, MountSelection, SourceDescriptor, ToolBusConfig,
    ToolBusError, assemble_envelope, error_envelope, validate_arguments,
};
use serde_json::{Map, Value, json};

fn arguments(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(arguments) => arguments,
        other => panic!("arguments must be a JSON object, got {other}"),
    }
}

fn property_schema(property_schema: Value) -> Value {
    let mut properties = Map::new();
    properties.insert("value".to_owned(), property_schema);
    json!({ "type": "object", "properties": properties })
}

fn violations(schema: &Value, instance: Value) -> Vec<String> {
    validate_arguments(schema, &arguments(instance))
}

fn assert_rejected(schema: &Value, instance: Value, fragment: &str) {
    let actual = violations(schema, instance);
    assert!(
        actual.iter().any(|line| line.contains(fragment)),
        "expected {fragment:?}, got {actual:?}"
    );
}

fn assert_accepted(schema: &Value, instance: Value) {
    let actual = violations(schema, instance);
    assert!(actual.is_empty(), "expected no violations, got {actual:?}");
}

#[test]
#[rustfmt::skip]
fn test_validate_arguments_type_matrix() {
    let cases = [
        (json!({"type": "object"}), json!({}), true, ""),
        (json!({"type": "array"}), json!([]), true, ""),
        (json!({"type": "string"}), json!("x"), true, ""),
        (json!({"type": "number"}), json!(1), true, ""),
        (json!({"type": "integer"}), json!(1), true, ""),
        (json!({"type": "integer"}), json!(1.0), true, ""),
        (json!({"type": "integer"}), json!(1.5), false, "expected type `integer`"),
        (json!({"type": "boolean"}), json!(true), true, ""),
        (json!({"type": "null"}), Value::Null, true, ""),
        (json!({"type": ["string", "null"]}), Value::Null, true, ""),
        (json!({"type": ["string", 7]}), json!(1), false, "expected type `string`"),
        (json!({"type": "mystery"}), json!(1), false, "expected type `mystery`"),
        (json!({"type": 7}), json!(1), false, "expected type `<malformed>`"),
    ];
    for (schema, instance, accepted, fragment) in cases {
        let schema = property_schema(schema);
        if accepted {
            assert_accepted(&schema, json!({"value": instance}));
        } else {
            assert_rejected(&schema, json!({"value": instance}), fragment);
        }
    }
}

#[test]
fn test_validate_arguments_number_and_string_bounds() {
    let number = property_schema(json!({
        "type": "number",
        "minimum": 5,
        "maximum": 10
    }));
    assert_rejected(&number, json!({"value": 4}), "below `minimum`");
    assert_rejected(&number, json!({"value": 11}), "above `maximum`");

    let exclusive = property_schema(json!({
        "type": "number",
        "exclusiveMinimum": 5,
        "exclusiveMaximum": 10
    }));
    assert_rejected(
        &exclusive,
        json!({"value": 5}),
        "must be > `exclusiveMinimum`",
    );
    assert_rejected(
        &exclusive,
        json!({"value": 10}),
        "must be < `exclusiveMaximum`",
    );

    let string = property_schema(json!({
        "type": "string",
        "minLength": 2,
        "maxLength": 3
    }));
    assert_rejected(&string, json!({"value": "a"}), "shorter than `minLength`");
    assert_rejected(&string, json!({"value": "abcd"}), "longer than `maxLength`");
    assert_accepted(&string, json!({"value": "汉字"}));
}

#[test]
fn test_validate_arguments_array_and_object_keywords() {
    let array = property_schema(json!({
        "type": "array",
        "minItems": 1,
        "maxItems": 2,
        "items": { "type": "string" }
    }));
    assert_rejected(&array, json!({"value": []}), "fewer than `minItems`");
    assert_rejected(
        &array,
        json!({"value": ["a", "b", "c"]}),
        "more than `maxItems`",
    );
    assert_rejected(
        &array,
        json!({"value": ["ok", 7]}),
        "/value/1: expected type `string`",
    );

    let object = json!({
        "type": "object",
        "minProperties": 2,
        "maxProperties": 3,
        "required": ["required"],
        "properties": {
            "required": { "type": "string" },
            "a/b": { "type": "number" },
            "a~b": { "type": "boolean" }
        },
        "additionalProperties": false
    });
    let missing = validate_arguments(&object, &arguments(json!({})));
    assert!(
        missing
            .iter()
            .any(|line| line.contains("fewer than `minProperties`"))
    );
    assert!(
        missing
            .iter()
            .any(|line| line.contains("missing required property"))
    );
    let escaped = validate_arguments(
        &object,
        &arguments(json!({"required": "x", "a/b": "wrong", "a~b": "wrong"})),
    );
    assert!(escaped.iter().any(|line| line.contains("/a~1b")));
    assert!(escaped.iter().any(|line| line.contains("/a~0b")));
    let extra = validate_arguments(
        &object,
        &arguments(json!({"required": "x", "a/b": 1, "a~b": true, "extra": 1})),
    );
    assert!(
        extra
            .iter()
            .any(|line| line.contains("more than `maxProperties`"))
    );
    assert!(
        extra
            .iter()
            .any(|line| line.contains("additional property `extra`"))
    );

    let additional = json!({
        "type": "object",
        "additionalProperties": { "type": "string" }
    });
    assert_rejected_value(&additional, json!({"extra": 7}), "/extra");
}

fn assert_rejected_value(schema: &Value, instance: Value, fragment: &str) {
    let actual = validate_arguments(schema, &arguments(instance));
    assert!(
        actual.iter().any(|line| line.contains(fragment)),
        "expected {fragment:?}, got {actual:?}"
    );
}

#[test]
fn test_validate_arguments_combinators_and_depth_limit() {
    let all_of = property_schema(json!({"allOf": [{"type": "number"}, {"minimum": 5}]}));
    assert_rejected(&all_of, json!({"value": 4}), "below `minimum`");
    let any_of = property_schema(json!({"anyOf": [{"type": "string"}, {"type": "number"}]}));
    assert_rejected(
        &any_of,
        json!({"value": true}),
        "matches none of the `anyOf` branches",
    );
    assert_accepted(&any_of, json!({"value": 1}));

    let overlap = property_schema(json!({"oneOf": [{"type": "number"}, {"minimum": 0}]}));
    assert_rejected(
        &overlap,
        json!({"value": 1}),
        "matches 2 `oneOf` branch(es)",
    );
    let disjoint = property_schema(json!({"oneOf": [{"type": "number"}, {"type": "string"}]}));
    assert_rejected(
        &disjoint,
        json!({"value": true}),
        "matches 0 `oneOf` branch(es)",
    );
    let not = property_schema(json!({"not": {"const": 1}}));
    assert_rejected(&not, json!({"value": 1}), "must not match the `not` schema");

    let mut nested_schema = Value::Bool(true);
    let mut nested_instance = json!("leaf");
    for _ in 0..70 {
        nested_schema = json!({"items": nested_schema.clone()});
        nested_instance = Value::Array(vec![nested_instance]);
    }
    assert_rejected(
        &property_schema(nested_schema),
        json!({"value": nested_instance}),
        "nests deeper than 64 levels",
    );
}

#[test]
#[rustfmt::skip]
fn test_assemble_envelope_success_evidence_and_source_tokens() {
    let evidence = EvidenceDescriptor { snapshot_id: Some("snapshot-1".to_owned()), screenshot: Some("blob://screenshot-1".to_owned()), extra: Some(json!({"confidence": 0.9})) };
    let request = EnvelopeRequest::new("demo.echo.echo", "task-1", "step-1", json!({"x": 1})).with_evidence(evidence).with_metrics(42, 0);
    let envelope = assemble_envelope(&request).expect("envelope must assemble");
    assert!(envelope.error.is_none());
    assert_eq!(envelope.metrics.as_ref().expect("metrics").attempts, 1);
    let wire = serde_json::to_value(&envelope).expect("envelope serializes");
    assert_eq!(wire.get("evidence").and_then(|value| value.get("snapshot_id")), Some(&json!("snapshot-1")));

    let sources = [
        (SourceKind::AppContent, "app_content"), (SourceKind::UserInput, "user_input"),
        (SourceKind::WebContent, "web_content"), (SourceKind::FileArtifact, "file_artifact"),
        (SourceKind::Screenshot, "screenshot"), (SourceKind::Ocr, "ocr"),
        (SourceKind::Clipboard, "clipboard"), (SourceKind::Unknown, "unknown"),
    ];
    for (kind, token) in sources {
        let source = SourceDescriptor { kind, app_id: Some("com.example.app".to_owned()), target: None };
        let envelope = assemble_envelope(&EnvelopeRequest::new("demo.echo.echo", "task-1", "step-1", json!({})).untrusted(source)).expect("source envelope must assemble");
        let wire = serde_json::to_value(&envelope).expect("envelope serializes");
        assert_eq!(wire.get("source").and_then(|value| value.get("kind")).and_then(Value::as_str), Some(token));
    }
}

#[test]
#[rustfmt::skip]
fn test_assemble_envelope_failure_and_truncation_paths() {
    let mut untrusted = EnvelopeRequest::new("demo.echo.echo", "task-1", "step-1", json!({"x": 1}));
    untrusted.untrusted = true;
    assert!(matches!(assemble_envelope(&untrusted), Err(ToolBusError::UntrustedWithoutSource { .. })));
    let scalar = EnvelopeRequest::new("demo.echo.echo", "task-1", "step-1", json!([1, 2]));
    assert!(matches!(assemble_envelope(&scalar), Err(ToolBusError::EnvelopeAssembly { .. })));

    let oversized = json!({"alpha": "A".repeat(4000), "beta": "B".repeat(10)});
    let envelope = assemble_envelope(&EnvelopeRequest::new("demo.echo.big", "task-1", "step-1", oversized).with_max_bytes(256)).expect("oversized multi-leaf payload must truncate");
    let truncated = envelope.truncated.as_ref().expect("truncation must be explicit");
    assert!(truncated.occurred);
    assert_eq!(truncated.reason, TruncationReason::MaxBytes);
    assert!(truncated.original_bytes.is_some());

    let no_leaves = json!({"count": 1, "ratio": 2, "flag": true});
    assert!(matches!(assemble_envelope(&EnvelopeRequest::new("demo.echo.numbers", "task-1", "step-1", no_leaves).with_max_bytes(8)), Err(ToolBusError::PayloadTooLarge { .. })));
    assert!(assemble_envelope(&EnvelopeRequest::new("demo.echo.small", "task-1", "step-1", json!({"x": 1})).with_max_bytes(1024)).expect("small payload passes").truncated.is_none());
}

#[test]
#[rustfmt::skip]
fn test_error_envelope_and_error_code_mapping() {
    let envelope = error_envelope("demo.echo.echo", "task-1", "step-1", ErrorCode::ToolInvalidArgs, "invalid arguments");
    assert!(!envelope.ok);
    assert!(envelope.metrics.is_none());
    assert_eq!(envelope.error.as_ref().expect("error").code, ErrorCode::ToolInvalidArgs);

    let t = tool;
    let cases = [
        (ToolBusError::DuplicateTool { tool: t() }, ErrorCode::Fatal),
        (ToolBusError::InvalidToolName { name: t() }, ErrorCode::Fatal),
        (ToolBusError::UnsupportedSchemaKeyword { tool: t(), problems: Vec::new() }, ErrorCode::Fatal),
        (ToolBusError::RiskAnnotationMismatch { tool: t(), declared: t(), required: t() }, ErrorCode::Fatal),
        (ToolBusError::ToolBehaviorMismatch { tool: t(), declared: t(), required: t() }, ErrorCode::Fatal),
        (ToolBusError::InvalidArguments { tool: t(), reason: t() }, ErrorCode::ToolInvalidArgs),
        (ToolBusError::ToolNotMounted { tool: t() }, ErrorCode::TargetNotFound),
        (ToolBusError::UnknownTool { tool: t() }, ErrorCode::TargetNotFound),
        (ToolBusError::PayloadTooLarge { tool: t(), actual_bytes: 9, max_bytes: 8 }, ErrorCode::Fatal),
        (ToolBusError::UntrustedWithoutSource { tool: t() }, ErrorCode::Fatal),
        (ToolBusError::EnvelopeAssembly { tool: t(), reason: t() }, ErrorCode::Fatal),
        (ToolBusError::EnvelopeMissing { tool: t() }, ErrorCode::Fatal),
        (ToolBusError::SchemaAssembly { tool: t(), reason: t() }, ErrorCode::Fatal),
        (ToolBusError::AuditEventAssembly { reason: t() }, ErrorCode::Fatal),
        (ToolBusError::FingerprintSerialization { reason: t() }, ErrorCode::Fatal),
        (ToolBusError::Transport { reason: t() }, ErrorCode::Transient),
        (ToolBusError::Mcp { code: -32601, message: t() }, ErrorCode::CapabilityMissing),
        (ToolBusError::Mcp { code: -32000, message: t() }, ErrorCode::Transient),
    ];
    for (error, expected) in cases {
        assert_eq!(error.error_code(), expected);
        assert!(!error.to_string().is_empty());
    }
}

fn tool() -> String {
    "demo.echo.echo".to_owned()
}

#[test]
fn test_tool_bus_config_and_mount_selection_constructors() {
    let config = ToolBusConfig::new("session-1")
        .with_default_max_bytes(128)
        .with_client_response_cache(true);
    assert_eq!(config.session_id(), "session-1");
    assert_eq!(config.default_max_bytes(), Some(128));
    assert!(config.client_response_cache());
    assert_eq!(
        MountSelection::names(vec![tool()]),
        MountSelection::Names(vec![tool()])
    );
    assert_eq!(
        MountSelection::apps(vec!["demo".to_owned()]),
        MountSelection::Apps(vec!["demo".to_owned()])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[rustfmt::skip]
async fn test_mount_selection_names_and_apps_cover_success_and_failure() {
    let mut registry = assistant_tool_bus::ToolRegistry::new();
    common::register(&mut registry, common::demo_definition("zeta", "zeta tool"), common::echo_handler());
    common::register(&mut registry, common::demo_definition("alpha", "alpha tool"), common::echo_handler());
    let (bus, report) = assistant_tool_bus::ToolBus::start(
        registry,
        MountSelection::names(vec!["demo.zeta.run".to_owned(), "demo.zeta.run".to_owned(), "demo.alpha.run".to_owned()]),
        ToolBusConfig::new("selected"),
        common::clock(),
    ).await.expect("known names must mount");
    assert_eq!(report.mounted, vec!["demo.alpha.run".to_owned(), "demo.zeta.run".to_owned(), "toolset.list".to_owned(), "toolset.search".to_owned()]);
    bus.shutdown().await.expect("bus must shut down");

    let error = match assistant_tool_bus::ToolBus::start(
        assistant_tool_bus::ToolRegistry::new(),
        MountSelection::names(vec!["demo.missing.run".to_owned()]),
        ToolBusConfig::new("missing"),
        common::clock(),
    ).await {
        Err(error) => error,
        Ok((bus, _report)) => { bus.shutdown().await.expect("valid mount"); panic!("unknown name must fail"); }
    };
    assert!(matches!(error, ToolBusError::UnknownTool { .. }));

    let mut registry = assistant_tool_bus::ToolRegistry::new();
    common::register(&mut registry, common::demo_definition("alpha", "alpha tool"), common::echo_handler());
    common::register(&mut registry, common::demo_definition("beta", "beta tool"), common::echo_handler());
    let (bus, report) = assistant_tool_bus::ToolBus::start(
        registry,
        MountSelection::apps(vec!["demo".to_owned(), "demo".to_owned()]),
        ToolBusConfig::new("apps"),
        common::clock(),
    ).await.expect("app prefix must mount");
    assert_eq!(report.mounted, vec!["demo.alpha.run".to_owned(), "demo.beta.run".to_owned(), "toolset.list".to_owned(), "toolset.search".to_owned()]);
    bus.shutdown().await.expect("bus must shut down");

    let error = match assistant_tool_bus::ToolBus::start(
        assistant_tool_bus::ToolRegistry::new(),
        MountSelection::apps(vec!["missing".to_owned()]),
        ToolBusConfig::new("app-missing"),
        common::clock(),
    ).await {
        Err(error) => error,
        Ok((bus, _report)) => { bus.shutdown().await.expect("valid mount"); panic!("empty app prefix must fail"); }
    };
    assert!(matches!(error, ToolBusError::UnknownTool { .. }));
}
