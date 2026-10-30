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

/// Absolute path to the TASK-056 browser task contract that motivated TASK-107.
fn t5_1_browser_package_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/browser.edge/tasks/t5.1.books-list-to-csv.json")
}

fn t1_1_inputs() -> serde_json::Map<String, Value> {
    inputs(&[
        ("file_size_bytes", json!(10)),
        ("max_text_bytes", json!(1024)),
        ("input.keywords", json!(["report"])),
        ("input.max_keyword_paragraphs", json!(50)),
    ])
}

/// Inputs that push T1.1 onto the large-file branch: `file_size_bytes > max_text_bytes`.
fn t1_1_large_file_inputs() -> serde_json::Map<String, Value> {
    inputs(&[
        ("file_size_bytes", json!(2_097_152)),
        ("max_text_bytes", json!(1024)),
        ("input.keywords", json!(["report"])),
        ("input.max_keyword_paragraphs", json!(50)),
    ])
}

#[test]
fn test_t1_1_package_renders_identical_plan_twice() {
    let path = t1_1_package_path();
    let first = TaskPackageProvider::from_package_file_with_inputs(&path, &t1_1_inputs())
        .expect("first render");
    let second = TaskPackageProvider::from_package_file_with_inputs(&path, &t1_1_inputs())
        .expect("second render");

    assert_eq!(
        first.plan_json(),
        second.plan_json(),
        "确定性来源必须对同一任务包产出逐字相同的 Plan JSON"
    );
    assert_eq!(first.model_id().as_str(), TASK_PACKAGE_MODEL_ID);
}

#[test]
fn test_browser_cdp_assertion_mappings_cover_edge_and_chrome() {
    let package = json!({
        "task_id": "browser.t5_1.assertion_probe",
        "steps": [
            {
                "id": "edge_navigate",
                "kind": "tool",
                "tool": "edge.cdp.navigate",
                "args": {"url": "http://127.0.0.1:8765/page-1.html", "load_condition": "network_idle"}
            },
            {
                "id": "edge_dom",
                "kind": "tool",
                "tool": "edge.cdp.dom_read",
                "args": {"selector": "main", "max_nodes": 1000}
            },
            {
                "id": "edge_idle",
                "kind": "tool",
                "tool": "edge.cdp.wait_network_idle",
                "args": {"quiet_ms": 500}
            },
            {
                "id": "chrome_navigate",
                "kind": "tool",
                "tool": "chrome.cdp.navigate",
                "args": {"url": "http://127.0.0.1:8765/page-1.html", "load_condition": "network_idle"}
            },
            {
                "id": "chrome_dom",
                "kind": "tool",
                "tool": "chrome.cdp.dom_read",
                "args": {"selector": "main", "max_nodes": 1000}
            },
            {
                "id": "chrome_idle",
                "kind": "tool",
                "tool": "chrome.cdp.wait_network_idle",
                "args": {"quiet_ms": 500}
            }
        ],
    })
    .to_string();
    let provider =
        TaskPackageProvider::from_package_json(&package).expect("browser package render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan has steps");
    assert_eq!(steps.len(), 6);

    for step in steps {
        let tool = step.get("tool").and_then(Value::as_str).expect("tool name");
        let postconditions = step
            .get("postconditions")
            .and_then(Value::as_array)
            .expect("postconditions");
        let timeouts = step
            .get("timeouts")
            .and_then(Value::as_object)
            .expect("timeouts");
        assistant_verify::parse_postconditions(postconditions)
            .expect("browser postconditions must be accepted by verify");

        if tool.ends_with(".navigate") {
            assert_eq!(timeouts.get("execute_ms"), Some(&json!(60000)));
            assert_eq!(
                postconditions,
                &vec![json!({
                    "kind": "state_changed",
                    "within_ms": 60000,
                    "fingerprint_scope": "cdp_target",
                })]
            );
        } else {
            assert_eq!(timeouts.get("execute_ms"), Some(&json!(20000)));
            assert_eq!(postconditions, &vec![json!({"kind": "state_unchanged"})]);
        }
    }
}

#[test]
fn test_browser_form_fill_assertions_pin_count_and_non_submission() {
    let package = json!({
        "task_id": "browser.t5_2.form_fill_probe",
        "steps": [{
            "id": "fill_form",
            "kind": "tool",
            "tool": "edge.cdp.form_fill",
            "args": {
                "fields": [
                    {"selector": "#name", "value": "Ada"},
                    {"selector": "#email", "value": "ada@example.test"}
                ]
            }
        }]
    })
    .to_string();
    let provider =
        TaskPackageProvider::from_package_json(&package).expect("form fill package render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let step = plan
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.first())
        .expect("form fill step");
    assert_eq!(
        step.get("postconditions"),
        Some(&json!([
            {"kind": "value_equals", "name": "filled_fields", "value": 2},
            {"kind": "value_equals", "name": "submitted", "value": false}
        ]))
    );
}

#[test]
fn test_browser_form_read_assertion_is_fixed() {
    let package = json!({
        "task_id": "browser.t5_2.form_read_probe",
        "steps": [
            {
                "id": "read_form",
                "kind": "tool",
                "tool": "edge.cdp.form_read",
                "args": {
                    "form_selector": "#profile-form",
                    "fields": [{"selector": "#name"}]
                }
            }
        ]
    })
    .to_string();
    let provider = TaskPackageProvider::from_package_json(&package).expect("form package render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan has steps");
    assert_eq!(
        steps.first().and_then(|step| step.get("postconditions")),
        Some(&json!([{"kind": "state_unchanged"}]))
    );
}

#[test]
fn test_browser_form_submit_assertion_is_state_changed() {
    let package = json!({
        "task_id": "browser.t5_2.form_submit_probe",
        "steps": [{
            "id": "submit_form",
            "kind": "tool",
            "tool": "edge.cdp.form_submit",
            "args": {
                "form_selector": "#profile-form",
                "submit_selector": "#profile-submit",
                "expected_origin": "http://127.0.0.1:43000",
                "expected_action": "http://127.0.0.1:43000/submitted",
                "expected_fields": [{"selector": "#name", "value": "Ada"}]
            }
        }]
    })
    .to_string();
    let provider =
        TaskPackageProvider::from_package_json(&package).expect("form submit package render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    assert_eq!(
        plan.get("steps")
            .and_then(Value::as_array)
            .and_then(|steps| steps.first())
            .and_then(|step| step.get("postconditions")),
        Some(&json!([{
            "kind": "state_changed",
            "within_ms": 20_000,
            "fingerprint_scope": "cdp_target"
        }]))
    );
}

#[test]
fn test_t5_1_browser_package_renders_without_missing_assertion() {
    let provider = TaskPackageProvider::from_package_file(&t5_1_browser_package_path())
        .expect("TASK-056 browser task contract must render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan has steps");
    assert_eq!(steps.len(), 6);

    let tools: Vec<&str> = steps
        .iter()
        .filter_map(|step| step.get("tool").and_then(Value::as_str))
        .collect();
    assert_eq!(
        tools,
        vec![
            "edge.cdp.navigate",
            "edge.cdp.dom_read",
            "edge.cdp.navigate",
            "edge.cdp.dom_read",
            "edge.cdp.navigate",
            "edge.cdp.dom_read",
        ]
    );

    for step in steps {
        let postconditions = step
            .get("postconditions")
            .and_then(Value::as_array)
            .expect("postconditions");
        assistant_verify::parse_postconditions(postconditions)
            .expect("rendered browser postconditions must be accepted by verify");
    }
}

#[test]
fn test_unmapped_browser_tool_still_fails_closed() {
    let package = json!({
        "task_id": "browser.unknown_assertion",
        "steps": [{
            "id": "fill",
            "kind": "tool",
            "tool": "edge.cdp.download",
            "args": {
                "target_directory": "C:\\fixture\\downloads",
                "overwrite_existing": false
            }
        }]
    })
    .to_string();

    let error = TaskPackageProvider::from_package_json(&package).expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::MissingAssertion { .. }));
}

#[test]
fn test_t1_1_plan_contains_read_step_and_pure_analysis_step() {
    let provider =
        TaskPackageProvider::from_package_file_with_inputs(&t1_1_package_path(), &t1_1_inputs())
            .expect("render");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan is JSON");
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan has a steps array");

    // T1.1's platform precondition stays out of the Plan. The file channel is now a
    // reserved host_service step, so the conditional large-file branch is represented in the
    // Plan without changing `l1_file`'s non-runtime semantics (ADR-0064). ADR-0065 adds the
    // unconditional fingerprint preamble that lets a false first condition be skipped at all.
    assert_eq!(
        steps.len(),
        4,
        "initial fingerprint + read_text + file channel + analyze enter the Plan"
    );
    let tools: Vec<&str> = steps
        .iter()
        .filter_map(|step| step.get("tool").and_then(Value::as_str))
        .collect();
    assert_eq!(
        tools,
        vec![
            "assistant.runtime.host_capture_initial_fingerprint",
            "notepad.file.read_text",
            "assistant.runtime.host_read_utf8_prefix",
            "assistant.runtime.pure_count_lines_and_keyword_paragraphs",
        ]
    );
}

/// ADR-0065 D1: the preamble must stay unconditional. If it ever gained a `when`, the
/// large-file branch would lose the only fingerprint published before it.
#[test]
fn test_t1_1_initial_fingerprint_step_is_unconditional() {
    let provider =
        TaskPackageProvider::from_package_file_with_inputs(&t1_1_package_path(), &t1_1_inputs())
            .expect("render");
    let dataflow = provider.dataflow_plan();

    let preamble = dataflow
        .get("capture_initial_fingerprint")
        .expect("the preamble must be rendered as an executable step");
    assert_eq!(
        preamble.tool, "assistant.runtime.host_capture_initial_fingerprint",
        "the preamble must map onto the reserved host_service tool"
    );
    assert!(
        preamble.condition.is_none(),
        "ADR-0065 D1: the preamble must not declare a condition"
    );
    assert!(
        dataflow
            .get("read_text")
            .expect("read_text is rendered")
            .condition
            .is_some(),
        "the small-file branch keeps its declared condition"
    );
    assert!(
        dataflow
            .get("read_file_channel")
            .expect("read_file_channel is rendered")
            .condition
            .is_some(),
        "the large-file branch keeps its declared condition"
    );
}

/// The branch choice happens at runtime through `when`, so the large-file input set renders
/// exactly the same executable steps as the small-file one (ADR-0061 D8).
#[test]
fn test_t1_1_large_file_inputs_render_the_same_steps() {
    let small =
        TaskPackageProvider::from_package_file_with_inputs(&t1_1_package_path(), &t1_1_inputs())
            .expect("small-file render");
    let large = TaskPackageProvider::from_package_file_with_inputs(
        &t1_1_package_path(),
        &t1_1_large_file_inputs(),
    )
    .expect("large-file render");

    assert_eq!(
        small.plan_json(),
        large.plan_json(),
        "输入只改变运行时条件求值，不得改变确定性 Plan"
    );
}

#[test]
fn test_rendered_postconditions_are_accepted_by_verify() {
    let provider =
        TaskPackageProvider::from_package_file_with_inputs(&t1_1_package_path(), &t1_1_inputs())
            .expect("render");
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
        "steps": [
            {"id": "open", "kind": "platform", "operation": "platform_open_file"},
            {"id": "read_file", "kind": "l1_file", "operation": "read_utf8"}
        ],
    })
    .to_string();

    let error = TaskPackageProvider::from_package_json(&package).expect_err("must fail closed");
    assert!(matches!(error, TaskPackageError::NoToolSteps { .. }));
}

#[test]
fn test_unknown_pure_operation_is_rejected_not_silently_skipped() {
    // TASK-219: a `pure` step whose operation is not in the closed set must fail at
    // construction, not disappear from the Plan (the previous silent-skip behavior).
    let package = json!({
        "task_id": "notepad.unknown-pure",
        "steps": [{
            "id": "analyze",
            "kind": "pure",
            "operation": "length_of_text",
            "args": { "text": "hello" }
        }]
    });
    let result = TaskPackageProvider::from_package_json(
        &serde_json::to_string(&package).expect("serialize package"),
    );
    assert!(
        matches!(result, Err(TaskPackageError::Malformed { .. })),
        "an unknown pure operation must be rejected, got {result:?}"
    );
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
fn test_paint_t3_2_file_steps_render_with_explicit_assertions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/com.microsoft.paint/tasks/t3.2.open-png-mark-region-save-as.json");
    let bound = inputs(&[
        ("input.opened_path", json!("C:\\fixture\\source.png")),
        ("input.target_path", json!("C:\\fixture\\marked-output.png")),
        ("input.rectangle_start", json!({"x": 64, "y": 48})),
        ("input.rectangle_end", json!({"x": 1984, "y": 1488})),
        ("input.rectangle_start.x", json!(64)),
        ("input.rectangle_start.y", json!(48)),
        ("input.rectangle_end.x", json!(1984)),
        ("input.rectangle_end.y", json!(1488)),
        ("input.foreground_red", json!(237)),
        ("input.foreground_green", json!(28)),
        ("input.foreground_blue", json!(36)),
        ("input.layer_id", json!("layer-1")),
        ("input.expected_layer_name", json!("Layer 1")),
        ("input.viewport_offset_x_px", json!(0)),
        ("input.viewport_offset_y_px", json!(0)),
        (
            "input.coordinate_space",
            json!({"kind": "physical_pixels", "monitor_id": "\\\\.\\DISPLAY1", "scale": 1.0}),
        ),
    ]);
    let provider = TaskPackageProvider::from_package_file_with_inputs(&path, &bound)
        .expect("T3.2 package renders");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan json");
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .expect("plan steps");
    let tools: Vec<&str> = steps
        .iter()
        .filter_map(|step| step.get("tool").and_then(Value::as_str))
        .collect();
    assert!(tools.contains(&"paint.file.open"));
    assert!(tools.contains(&"paint.file.save_as"));
    let open = steps
        .iter()
        .find(|step| step.get("tool").and_then(Value::as_str) == Some("paint.file.open"))
        .expect("open step");
    assert!(
        open.get("postconditions")
            .and_then(Value::as_array)
            .is_some_and(|assertions| !assertions.is_empty())
    );
    let save = steps
        .iter()
        .find(|step| step.get("tool").and_then(Value::as_str) == Some("paint.file.save_as"))
        .expect("save-as step");
    assert!(
        save.get("postconditions")
            .and_then(Value::as_array)
            .is_some_and(|assertions| !assertions.is_empty())
    );
}

#[test]
fn test_paint_t3_3_rollback_renders_value_and_visual_assertions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/com.microsoft.paint/tasks/t3.3.open-png-draw-rollback.json");
    let bound = inputs(&[
        ("input.opened_path", json!("C:\\fixture\\source.png")),
        ("input.rectangle_start", json!({"x": 64, "y": 48})),
        ("input.rectangle_end", json!({"x": 1984, "y": 1488})),
        ("input.rectangle_start.x", json!(64)),
        ("input.rectangle_start.y", json!(48)),
        ("input.rectangle_end.x", json!(1984)),
        ("input.rectangle_end.y", json!(1488)),
        ("input.foreground_red", json!(237)),
        ("input.foreground_green", json!(28)),
        ("input.foreground_blue", json!(36)),
        ("input.layer_id", json!("layer-1")),
        ("input.expected_layer_name", json!("Layer 1")),
        ("input.viewport_offset_x_px", json!(0)),
        ("input.viewport_offset_y_px", json!(0)),
        (
            "input.coordinate_space",
            json!({"kind": "physical_pixels", "monitor_id": "\\\\.\\DISPLAY1", "scale": 1.0}),
        ),
        ("input.expected_undo_steps", json!(1)),
    ]);
    let provider = TaskPackageProvider::from_package_file_with_inputs(&path, &bound)
        .expect("T3.3 package renders");
    let plan: Value = serde_json::from_str(provider.plan_json()).expect("plan json");
    let rollback = plan
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| {
            steps.iter().find(|step| {
                step.get("tool").and_then(Value::as_str) == Some("paint.canvas.rollback_last_write")
            })
        })
        .expect("rollback step");
    let postconditions = rollback
        .get("postconditions")
        .and_then(Value::as_array)
        .expect("rollback postconditions");
    assert_eq!(postconditions.len(), 2);
    assert!(postconditions.iter().any(|assertion| {
        assertion.get("kind").and_then(Value::as_str) == Some("value_equals")
            && assertion.get("name").and_then(Value::as_str) == Some("undo_steps_completed")
            && assertion.get("value").and_then(Value::as_u64) == Some(1)
    }));
    assert!(postconditions.iter().any(|assertion| {
        assertion.get("kind").and_then(Value::as_str) == Some("visual_assert")
            && assertion.get("op").and_then(Value::as_str) == Some("mean_abs_diff_within")
            && assertion.get("max_mean_abs_diff").and_then(Value::as_u64) == Some(2)
            && assertion.get("confidence_min").and_then(Value::as_f64) == Some(0.9)
    }));
}

#[test]
fn test_paint_file_assertions_fail_closed_without_required_arguments() {
    for (tool, args) in [
        ("paint.file.open", json!({})),
        ("paint.file.save_as", json!({})),
    ] {
        let package = json!({
            "task_id": "paint.file.missing-assertion-argument",
            "steps": [{
                "id": "file_step",
                "kind": "tool",
                "tool": tool,
                "args": args,
            }],
        })
        .to_string();
        let error = TaskPackageProvider::from_package_json(&package)
            .expect_err("missing file assertion arguments must fail closed");
        assert!(
            matches!(error, TaskPackageError::MissingAssertion { .. }),
            "unexpected error for {tool}: {error:?}"
        );
    }
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

/// The remaining non-runtime kinds are **recorded**, not silently dropped.
#[test]
fn test_t1_1_plan_records_the_declared_not_executed_kinds() {
    let provider =
        TaskPackageProvider::from_package_file_with_inputs(&t1_1_package_path(), &t1_1_inputs())
            .expect("render");
    let kinds: Vec<&str> = provider
        .declared_not_executed()
        .iter()
        .filter_map(|entry| entry.get("kind").and_then(Value::as_str))
        .collect();
    assert_eq!(kinds, vec!["platform"]);

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
    let provider =
        TaskPackageProvider::from_package_file_with_inputs(&t1_1_package_path(), &t1_1_inputs())
            .expect("render");
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
