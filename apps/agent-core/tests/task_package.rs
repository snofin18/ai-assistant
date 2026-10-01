//! Contract tests for the deterministic task-package Plan source (ADR-0058 D2).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use assistant_agent_core::{TASK_PACKAGE_MODEL_ID, TaskPackageError, TaskPackageProvider};
use assistant_model_gateway::{
    CacheHints, CancellationToken, CompletionRequest, Message, MessageRole, ModelProvider,
    ToolChoice,
};
use assistant_protocol::serde_json::{Value, json};

/// Absolute path to the declared T1.1 package that stage 1a must run.
fn t1_1_package_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json")
}

#[test]
fn test_t1_1_package_renders_identical_plan_twice() {
    let path = t1_1_package_path();
    let first = TaskPackageProvider::from_package_file(&path).expect("first render");
    let second = TaskPackageProvider::from_package_file(&path).expect("second render");

    assert_eq!(
        first.plan_json(),
        second.plan_json(),
        "确定性来源必须对同一任务包产出逐字相同的 Plan JSON"
    );
    assert_eq!(first.model_id().as_str(), TASK_PACKAGE_MODEL_ID);
}

#[test]
fn test_t1_1_plan_only_contains_tool_steps() {
    let provider = TaskPackageProvider::from_package_file(&t1_1_package_path()).expect("render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan has a steps array");

    // T1.1 declares a platform precondition, a tool step, an L1 file channel,
    // and a pure analysis step. Only the tool step is executable today.
    assert_eq!(steps.len(), 1, "只有 kind=tool 的步骤进入 Plan");
    assert_eq!(
        steps
            .first()
            .and_then(|step| step.get("tool"))
            .and_then(Value::as_str),
        Some("notepad.file.read_text")
    );
}

#[test]
fn test_rendered_postconditions_are_accepted_by_verify() {
    let provider = TaskPackageProvider::from_package_file(&t1_1_package_path()).expect("render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let Some(steps) = plan.get("steps").and_then(Value::as_array) else {
        panic!("plan must contain steps");
    };

    for step in steps {
        let Some(postconditions) = step.get("postconditions").and_then(Value::as_array) else {
            panic!("every step must declare postconditions");
        };
        let parsed = assistant_verify::parse_postconditions(postconditions);
        assert!(
            parsed.is_ok(),
            "1a 断言映射表产出的 postcondition 必须被 verify 接受，实际错误：{parsed:?}"
        );
    }
}

#[test]
fn test_replace_text_assertion_uses_the_declared_new_text() {
    let package = json!({
        "task_id": "notepad.t1_2.probe",
        "steps": [{
            "id": "replace",
            "kind": "tool",
            "tool": "notepad.file.replace_text",
            "args": {"old_text": "报表", "new_text": "报告", "expected_replacements": 1},
        }],
    })
    .to_string();
    let provider = TaskPackageProvider::from_package_json(&package).expect("render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let text = plan
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.first())
        .and_then(|step| step.get("postconditions"))
        .cloned()
        .expect("postconditions");

    assert_eq!(
        text,
        json!([
            {"kind": "text_contains", "value": "报告"},
            {"kind": "state_changed", "within_ms": 2000},
        ])
    );
}

#[test]
fn test_package_without_tool_steps_is_rejected() {
    let package = json!({
        "task_id": "notepad.empty",
        "steps": [{"id": "analyze", "kind": "pure", "operation": "count_lines"}],
    })
    .to_string();

    let error = TaskPackageProvider::from_package_json(&package).expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::NoToolSteps { .. }));
}

#[test]
fn test_unbound_arguments_are_rejected() {
    let package = json!({
        "task_id": "notepad.unbound",
        "steps": [{
            "id": "save_as",
            "kind": "tool",
            "tool": "notepad.file.save_as",
            "args": {"target_path": "$input.file_path"},
        }],
    })
    .to_string();

    let error = TaskPackageProvider::from_package_json(&package).expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::UnboundArguments { .. }));
}

#[test]
fn test_tool_outside_the_assertion_table_is_rejected() {
    let package = json!({
        "task_id": "notepad.unknown",
        "steps": [{"id": "click", "kind": "tool", "tool": "notepad.file.explode"}],
    })
    .to_string();

    let error = TaskPackageProvider::from_package_json(&package).expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::MissingAssertion { .. }));
}

#[test]
fn test_malformed_body_is_rejected() {
    let error = TaskPackageProvider::from_package_json("{not json").expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::Malformed { .. }));
}

/// Builds an input map from `(name, value)` pairs.
fn inputs(pairs: &[(&str, Value)]) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    for (name, value) in pairs {
        map.insert((*name).to_owned(), value.clone());
    }
    map
}

/// Absolute path to the declared T1.2 package.
fn t1_2_package_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/com.microsoft.notepad/tasks/t1.2.replace-save-approval-undo.json")
}

/// ADR-0060: `hitl` / `host_service` / `verify` map onto **reserved runtime
/// tools**, so T1.2 renders a complete plan instead of failing closed.
#[test]
fn test_t1_2_package_renders_with_reserved_runtime_tools() {
    let body = std::fs::read_to_string(t1_2_package_path()).expect("read t1.2 package");
    let bound = inputs(&[
        ("input.old_text", json!("报表")),
        ("input.new_text", json!("报告")),
        ("input.expected_replacements", json!(1)),
        ("rollback.replace_recipe", json!("l0_undo")),
        ("rollback.save_recipe", json!("l1_snapshot")),
        ("rollback.required_anchor_levels", json!(["l0", "l1"])),
        ("approval_diff", json!("-报表\n+报告")),
    ]);

    let provider = TaskPackageProvider::from_package_json_with_inputs(&body, &bound)
        .expect("T1.2 must render once the step kinds have reserved tools");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let tools: Vec<&str> = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan has steps")
        .iter()
        .filter_map(|step| step.get("tool").and_then(Value::as_str))
        .collect();

    assert!(
        tools.contains(&"notepad.file.replace_text"),
        "T1.2 的 replace_text 步骤必须进入 Plan"
    );
    assert!(tools.contains(&"assistant.runtime.request_approval"));
    assert!(tools.contains(&"assistant.runtime.prepare_anchors"));
    assert!(tools.contains(&"assistant.runtime.verify_postconditions"));
    assert!(
        !provider.plan_json().contains("$input."),
        "绑定后不得残留 `$input.` 字面量"
    );
}

/// The four kinds owned elsewhere are **recorded**, not silently dropped.
#[test]
fn test_t1_1_plan_records_the_declared_not_executed_kinds() {
    let provider = TaskPackageProvider::from_package_file(&t1_1_package_path()).expect("render");
    let kinds: Vec<&str> = provider
        .declared_not_executed()
        .iter()
        .filter_map(|entry| entry.get("kind").and_then(Value::as_str))
        .collect();
    assert_eq!(kinds, vec!["platform", "l1_file", "pure"]);

    // The plan itself must stay a single-field object: the Planner rejects
    // anything else, so the not-executed kinds deliberately travel out of band.
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let fields = plan.as_object().expect("plan is an object");
    assert_eq!(fields.len(), 1, "plan must contain only the steps field");
}

#[test]
fn test_missing_input_is_rejected() {
    let package = json!({
        "task_id": "notepad.missing-input",
        "steps": [{
            "id": "replace",
            "kind": "tool",
            "tool": "notepad.file.replace_text",
            "args": {
                "old_text": "$input.old_text",
                "new_text": "y",
                "expected_replacements": 1,
            },
        }],
    })
    .to_string();

    let error =
        TaskPackageProvider::from_package_json_with_inputs(&package, &inputs(&[("x", json!(1))]))
            .expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::MissingInput { .. }));
}

#[test]
fn test_unknown_step_kind_is_rejected() {
    let package = json!({
        "task_id": "notepad.unknown-kind",
        "steps": [
            {"id": "read", "kind": "tool", "tool": "notepad.file.read_text"},
            {"id": "weird", "kind": "frobnicate"},
        ],
    })
    .to_string();

    let error = TaskPackageProvider::from_package_json(&package).expect_err("must fail closed");
    assert!(matches!(
        error,
        TaskPackageError::UnknownStepKind { ref kind, .. } if kind == "frobnicate"
    ));
}

#[test]
fn test_unsupported_reference_is_rejected() {
    let package = json!({
        "task_id": "notepad.derived",
        "steps": [{
            "id": "save_as",
            "kind": "tool",
            "tool": "notepad.file.save_as",
            "args": {"target_path": "$normalized_target_path", "overwrite_existing": false},
        }],
    })
    .to_string();

    // A reference the caller cannot supply must fail closed: this layer does not
    // compute derived values, and it must not leave `$normalized_target_path` in
    // the plan as a literal either.
    let error = TaskPackageProvider::from_package_json_with_inputs(
        &package,
        &inputs(&[("target_path", json!("C:/tmp/out.txt"))]),
    )
    .expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::MissingInput { .. }));
}

#[test]
fn test_stream_emits_plan_json_then_usage_then_stop() {
    let provider = TaskPackageProvider::from_package_file(&t1_1_package_path()).expect("render");
    let request = CompletionRequest::new(
        vec![Message::new(MessageRole::User, "open and read")],
        Vec::new(),
        ToolChoice::None,
        CacheHints::disabled(),
    );
    let mut stream = provider
        .complete(request, CancellationToken::new())
        .expect("complete starts");
    let cancellation = CancellationToken::new();

    let first = stream
        .next_event(&cancellation, assistant_model_gateway::DurationMs::new(50))
        .expect("first event")
        .expect("some event");
    assert!(
        matches!(first, assistant_model_gateway::CompletionEvent::TextDelta(ref text) if text == provider.plan_json()),
        "第一个事件必须是与 plan_json 逐字相同的文本增量"
    );

    let second = stream
        .next_event(&cancellation, assistant_model_gateway::DurationMs::new(50))
        .expect("second event")
        .expect("some event");
    assert!(matches!(
        second,
        assistant_model_gateway::CompletionEvent::Usage(_)
    ));
}
