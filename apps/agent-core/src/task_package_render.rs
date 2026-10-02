//! Deterministic renderer from a declared task package to Planner JSON.
//!
//! Responsibilities: validate step kinds, references, conditions, outputs, and
//! postconditions, then emit the executable step subset.
//!
//! Boundaries: no model calls, no execution, and no mutation of package files.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::{
    DeclaredPackage, DeclaredStep, STATE_CHANGED_WINDOW_MS, STEP_EXECUTE_TIMEOUT_MS,
    STEP_RESOLVE_TIMEOUT_MS, STEP_VERIFY_TIMEOUT_MS, TaskPackageError,
};
use crate::runtime_dataflow::{
    ConditionExpr, DataflowError, RuntimeDataflowPlan, RuntimeStepBinding, parse_condition,
};

pub(super) fn render_plan(
    package: &DeclaredPackage,
    inputs: &serde_json::Map<String, Value>,
) -> Result<(String, Vec<Value>, RuntimeDataflowPlan), TaskPackageError> {
    let mut steps = Vec::new();
    let mut declared_not_executed = Vec::new();
    let mut dataflow = RuntimeDataflowPlan::new();
    let mut known_outputs: BTreeMap<String, String> = BTreeMap::new();
    let mut sequence: u32 = 1;
    for step in &package.steps {
        if let Some(rendered) = render_step(
            package,
            step,
            inputs,
            &mut known_outputs,
            &mut dataflow,
            sequence,
            &mut declared_not_executed,
        )? {
            steps.push(rendered);
            sequence = sequence.saturating_add(1);
        }
    }
    if steps.is_empty() {
        return Err(TaskPackageError::NoToolSteps {
            task_id: package.task_id.clone(),
        });
    }
    let plan_json = serde_json::to_string(&json!({ "steps": steps })).map_err(|error| {
        TaskPackageError::Malformed {
            reason: error.to_string(),
        }
    })?;
    Ok((plan_json, declared_not_executed, dataflow))
}

#[allow(clippy::too_many_arguments)]
fn render_step(
    package: &DeclaredPackage,
    step: &DeclaredStep,
    inputs: &serde_json::Map<String, Value>,
    known_outputs: &mut BTreeMap<String, String>,
    dataflow: &mut RuntimeDataflowPlan,
    sequence: u32,
    declared_not_executed: &mut Vec<Value>,
) -> Result<Option<Value>, TaskPackageError> {
    let condition = parse_step_condition(package, step)?;
    if !matches!(
        step.kind.as_str(),
        "tool" | "hitl" | "host_service" | "verify" | "pure"
    ) {
        match step.kind.as_str() {
            "platform" | "l1_file" | "policy" => {
                declared_not_executed.push(json!({ "id": step.id, "kind": step.kind }));
                return Ok(None);
            }
            other => {
                return Err(TaskPackageError::UnknownStepKind {
                    task_id: package.task_id.clone(),
                    step_id: step.id.clone(),
                    kind: other.to_owned(),
                });
            }
        }
    }
    let tool = resolve_step_tool(package, step)?;
    validate_condition_references(package, step, condition.as_ref(), inputs, known_outputs)?;
    let output_names = known_outputs.keys().cloned().collect::<BTreeSet<_>>();
    let arguments = resolve_arguments(&package.task_id, step, inputs, &output_names)?;
    let Some(postconditions) = assertion_table(tool, &arguments) else {
        return Err(TaskPackageError::MissingAssertion {
            task_id: package.task_id.clone(),
            step_id: step.id.clone(),
            tool: tool.to_owned(),
        });
    };
    register_step_dataflow(step, tool, condition, known_outputs, dataflow)?;
    let point_of_no_return = tool == "notepad.file.save_as";
    Ok(Some(json!({
        "id": step.id,
        "sequence": sequence,
        "tool": tool,
        "args": arguments,
        "depends_on": [],
        "postconditions": postconditions,
        "point_of_no_return": point_of_no_return,
        "timeouts": {
            "resolve_ms": STEP_RESOLVE_TIMEOUT_MS,
            "execute_ms": STEP_EXECUTE_TIMEOUT_MS,
            "verify_ms": STEP_VERIFY_TIMEOUT_MS,
        },
    })))
}

fn resolve_step_tool<'a>(
    package: &DeclaredPackage,
    step: &'a DeclaredStep,
) -> Result<&'a str, TaskPackageError> {
    match step.kind.as_str() {
        "tool" => step
            .tool
            .as_deref()
            .ok_or_else(|| TaskPackageError::Malformed {
                reason: format!(
                    "task package step `{}` is kind=tool but declares no tool",
                    step.id
                ),
            }),
        kind => crate::runtime_tools::tool_for_step(kind, step.operation.as_deref()).ok_or_else(
            || {
                step.operation
                    .as_deref()
                    .map_or_else(|| TaskPackageError::MissingOperation {
                        task_id: package.task_id.clone(),
                        step_id: step.id.clone(),
                        kind: kind.to_owned(),
                    }, |operation| TaskPackageError::Malformed {
                    reason: format!(
                        "task package step `{}` uses unsupported kind/operation `{kind}/{operation}`",
                        step.id
                    ),
                })
            },
        ),
    }
}

fn register_step_dataflow(
    step: &DeclaredStep,
    tool: &str,
    condition: Option<ConditionExpr>,
    known_outputs: &mut BTreeMap<String, String>,
    dataflow: &mut RuntimeDataflowPlan,
) -> Result<(), TaskPackageError> {
    for output in &step.outputs {
        known_outputs.insert(output.clone(), step.id.clone());
    }
    dataflow
        .insert(
            step.id.clone(),
            RuntimeStepBinding {
                tool: tool.to_owned(),
                condition,
                outputs: step.outputs.clone(),
            },
        )
        .map_err(|error| match error {
            DataflowError::DuplicateStep { step_id } => TaskPackageError::Malformed {
                reason: format!("duplicate executable step `{step_id}`"),
            },
            other => TaskPackageError::Malformed {
                reason: other.to_string(),
            },
        })
}

fn resolve_arguments(
    task_id: &str,
    step: &DeclaredStep,
    inputs: &serde_json::Map<String, Value>,
    known_outputs: &BTreeSet<String>,
) -> Result<Value, TaskPackageError> {
    fn walk(
        value: &Value,
        task_id: &str,
        step_id: &str,
        inputs: &serde_json::Map<String, Value>,
        known_outputs: &BTreeSet<String>,
    ) -> Result<Value, TaskPackageError> {
        match value {
            Value::String(text) if text.starts_with('$') => {
                let name = text.trim_start_matches('$');
                if let Some(value) = inputs.get(name) {
                    return Ok(value.clone());
                }
                if known_outputs.contains(name) {
                    return Ok(Value::String(text.clone()));
                }
                if inputs.is_empty() {
                    return Err(TaskPackageError::UnboundArguments {
                        task_id: task_id.to_owned(),
                        step_id: step_id.to_owned(),
                    });
                }
                Err(TaskPackageError::MissingInput {
                    task_id: task_id.to_owned(),
                    step_id: step_id.to_owned(),
                    reference: text.clone(),
                })
            }
            Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => Ok(value.clone()),
            Value::Array(items) => items
                .iter()
                .map(|item| walk(item, task_id, step_id, inputs, known_outputs))
                .collect::<Result<Vec<Value>, _>>()
                .map(Value::Array),
            Value::Object(fields) => fields
                .iter()
                .map(|(key, item)| {
                    walk(item, task_id, step_id, inputs, known_outputs)
                        .map(|resolved| (key.clone(), resolved))
                })
                .collect::<Result<serde_json::Map<String, Value>, _>>()
                .map(Value::Object),
        }
    }

    if step.args.is_null() {
        return Ok(json!({}));
    }
    if !step.args.is_object() {
        return Err(TaskPackageError::Malformed {
            reason: format!(
                "task package step `{}` arguments must be a JSON object",
                step.id
            ),
        });
    }
    walk(&step.args, task_id, &step.id, inputs, known_outputs)
}

fn parse_step_condition(
    package: &DeclaredPackage,
    step: &DeclaredStep,
) -> Result<Option<ConditionExpr>, TaskPackageError> {
    let Some(source) = step.when.as_deref() else {
        return Ok(None);
    };
    parse_condition(source).map(Some).map_err(|error| {
        let reason = match error {
            DataflowError::InvalidCondition { reason, .. } => reason,
            other => other.to_string(),
        };
        TaskPackageError::InvalidCondition {
            task_id: package.task_id.clone(),
            step_id: step.id.clone(),
            condition: source.to_owned(),
            reason,
        }
    })
}

fn validate_condition_references(
    package: &DeclaredPackage,
    step: &DeclaredStep,
    condition: Option<&ConditionExpr>,
    inputs: &serde_json::Map<String, Value>,
    known_outputs: &BTreeMap<String, String>,
) -> Result<(), TaskPackageError> {
    let Some(condition) = condition else {
        return Ok(());
    };
    for reference in condition.references() {
        if inputs.contains_key(&reference) || known_outputs.contains_key(&reference) {
            continue;
        }
        return Err(TaskPackageError::MissingInput {
            task_id: package.task_id.clone(),
            step_id: step.id.clone(),
            reference: format!("${reference}"),
        });
    }
    Ok(())
}

pub(super) fn normalize_task_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn assertion_table(tool: &str, arguments: &Value) -> Option<Vec<Value>> {
    match tool {
        "notepad.file.read_text" => Some(vec![json!({ "kind": "state_unchanged" })]),
        "notepad.file.replace_text" => {
            let new_text = arguments.get("new_text")?.as_str()?;
            Some(vec![
                json!({ "kind": "text_contains", "value": new_text }),
                json!({ "kind": "state_changed", "within_ms": STATE_CHANGED_WINDOW_MS }),
            ])
        }
        "notepad.file.save" | "notepad.tab.new" => Some(vec![
            json!({ "kind": "state_changed", "within_ms": STATE_CHANGED_WINDOW_MS }),
        ]),
        "notepad.file.save_as" => Some(vec![json!({
            "kind": "value_equals",
            "name": "file_created",
            "value": true,
        })]),
        crate::runtime_tools::TOOL_HOST_SET_EDITOR_VALUE => Some(vec![json!({
            "kind": "state_changed",
            "within_ms": STATE_CHANGED_WINDOW_MS,
        })]),
        other if other.starts_with("assistant.runtime.pure_") => Some(vec![json!({
            "kind": "value_equals",
            "name": "pure_result",
            "value": true,
        })]),
        other if crate::runtime_tools::RESERVED_RUNTIME_TOOLS.contains(&other) => {
            Some(vec![json!({ "kind": "state_unchanged" })])
        }
        _ => None,
    }
}
