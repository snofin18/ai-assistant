//! Deterministic task-package Plan source (ADR-0058 D2 / DRIFT-214-1).
//!
//! Responsibilities:
//! - turn one declared task package into the plan JSON that
//!   `assistant_core::Planner` parses, with no model, no network, and no
//!   randomness;
//! - own the **1a assertion table** that translates a declared postcondition
//!   into the `kind` shapes `crates/verify` accepts.
//!
//! Boundaries:
//! - does not select a model, retry, or fall back to another source;
//! - does not grant permission, execute a step, or touch the platform;
//! - does not rewrite the task package, and does not invent an assertion for a
//!   tool the table does not cover.
//!
//! Invariants:
//! 1. the same package always renders byte-identical plan JSON;
//! 2. a package that declares no executable tool step is rejected rather than
//!    rendered as an empty plan;
//! 3. a tool step whose assertion is unknown, or whose arguments are still
//!    unbound (`$input.` placeholders), is rejected instead of guessed;
//! 4. every rendered postcondition is accepted by
//!    `assistant_verify::parse_postconditions` (proved by the crate tests).
//!
//! Why the assertion table lives here (DRIFT-214-1): the task packages and
//! `adapters/com.microsoft.notepad/tools/tools.json` carry *prose*
//! postcondition identifiers such as `read_only_no_state_change`, and a
//! deterministic provider must not invent the concrete expected values that
//! `crates/verify` evaluates. Keeping the mapping explicit and reviewable in
//! the binary layer is what makes the 1a assertion set a decision instead of a
//! fabrication.
//!
//! Related documents: `docs/adr/0058-production-composition-root-and-plan-source.md`,
//! `docs/spec/runtime-execution.md`, `tasks/TASK-214-production-composition-root-notepad-handlers.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use assistant_model_gateway::{
    CancellationToken, CompletionEvent, CompletionRequest, CompletionStream, DurationMs,
    FinishReason, Message, ModelGatewayError, ModelId, ModelProvider, ModelResult, Pricing,
    ProviderCapabilities, ProviderFeatures, TokenCount, Usage,
};
use assistant_task_engine::TaskId;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::runtime_dataflow::{
    ConditionExpr, DataflowError, RuntimeDataflowPlan, RuntimeStepBinding, parse_condition,
};

/// Model identity reported by the deterministic 1a Plan source.
pub const TASK_PACKAGE_MODEL_ID: &str = "task_package";

/// Phase timeouts applied to every rendered step.
///
/// They mirror the pilot budgets in `docs/spec/runtime-execution.md` section 5
/// and stay well inside the 30 s task budget the task packages declare.
const STEP_RESOLVE_TIMEOUT_MS: u64 = 3_000;
const STEP_EXECUTE_TIMEOUT_MS: u64 = 20_000;
const STEP_VERIFY_TIMEOUT_MS: u64 = 5_000;

/// Deadline used by `state_changed` assertions for write steps.
const STATE_CHANGED_WINDOW_MS: u64 = 2_000;

/// Why a declared task package cannot become a deterministic Plan.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TaskPackageError {
    /// The package file could not be read.
    #[error("task package at `{path}` could not be read: {reason}")]
    Read {
        /// Path that was attempted.
        path: String,
        /// Underlying I/O reason.
        reason: String,
    },

    /// The package body is not a JSON object with the expected fields.
    #[error("task package is not a valid declaration: {reason}")]
    Malformed {
        /// Which field or shape was wrong.
        reason: String,
    },

    /// The package declares no executable tool step.
    #[error("task package `{task_id}` declares no executable tool step")]
    NoToolSteps {
        /// Declared task identifier.
        task_id: String,
    },

    /// A tool step still carries `$input.` bindings instead of concrete values.
    #[error("task package `{task_id}` step `{step_id}` still carries unbound arguments")]
    UnboundArguments {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
    },

    /// A `$input.` reference has no value in the supplied input map.
    #[error(
        "task package `{task_id}` step `{step_id}` references `{reference}`, which the supplied inputs do not define"
    )]
    MissingInput {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// The unresolved reference, as written in the package.
        reference: String,
    },

    /// A `$`-prefixed reference this binding layer does not support.
    #[error(
        "task package `{task_id}` step `{step_id}` uses `{reference}`, which the 1a input binding layer does not support"
    )]
    UnsupportedReference {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// The unsupported reference, as written in the package.
        reference: String,
    },

    /// A step kind inside the runtime coverage set that has no executor yet.
    #[error(
        "task package `{task_id}` step `{step_id}` uses kind `{kind}`, which ADR-0059 puts in the runtime coverage set but no executor implements yet (CapabilityMissing)"
    )]
    UnexecutedStepKind {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// The step kind.
        kind: String,
    },

    /// A step kind outside every declared set.
    #[error(
        "task package `{task_id}` step `{step_id}` uses unknown kind `{kind}`; the closed set is `tool`/`hitl`/`host_service`/`verify` plus the declared-not-executed kinds (ToolInvalidArgs)"
    )]
    UnknownStepKind {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// The step kind.
        kind: String,
    },

    /// A step needs an operation to select its reserved runtime tool.
    #[error("task package `{task_id}` step `{step_id}` uses kind `{kind}` without an operation")]
    MissingOperation {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// The step kind.
        kind: String,
    },

    /// A `when` condition is outside the closed predicate subset.
    #[error(
        "task package `{task_id}` step `{step_id}` has invalid condition `{condition}`: {reason}"
    )]
    InvalidCondition {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// Original condition.
        condition: String,
        /// Parser reason.
        reason: String,
    },

    /// A tool step has no entry in the 1a assertion table.
    #[error(
        "task package `{task_id}` step `{step_id}` calls `{tool}`, which the 1a assertion table does not cover"
    )]
    MissingAssertion {
        /// Declared task identifier.
        task_id: String,
        /// Declared step identifier.
        step_id: String,
        /// Tool the step calls.
        tool: String,
    },
}

/// One declared task package, restricted to the fields the Plan source reads.
#[derive(Debug, Deserialize)]
struct DeclaredPackage {
    task_id: String,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    steps: Vec<DeclaredStep>,
}

/// One declared step. Non-`tool` kinds are task preconditions or pure
/// post-processing and are deliberately not executable `PlanStep`s.
#[derive(Debug, Deserialize)]
struct DeclaredStep {
    id: String,
    kind: String,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    args: Value,
    #[serde(default)]
    when: Option<String>,
    #[serde(default)]
    outputs: Vec<String>,
}

/// Deterministic `ModelProvider` over one declared task package.
///
/// The rendered plan JSON never changes for a given package: there is no clock,
/// no random source, and no I/O after construction. `complete` therefore always
/// returns the same single text delta.
#[derive(Debug, Clone)]
pub struct TaskPackageProvider {
    model_id: ModelId,
    declared_task_id: String,
    task_id: TaskId,
    goal: String,
    plan_json: String,
    declared_not_executed: Vec<Value>,
    dataflow: RuntimeDataflowPlan,
}

impl TaskPackageProvider {
    /// Builds a provider from an in-memory task package declaration.
    ///
    /// # Errors
    ///
    /// Returns [`TaskPackageError`] when the body is malformed, declares no
    /// executable tool step, keeps `$input.` bindings, or names a tool the 1a
    /// assertion table does not cover.
    pub fn from_package_json(package_json: &str) -> Result<Self, TaskPackageError> {
        Self::from_package_json_with_inputs(package_json, &serde_json::Map::new())
    }

    /// Builds a provider with an explicit task input map (TASK-216 slice A).
    ///
    /// `$input.<name>` placeholders inside a step's `args` are replaced by the
    /// supplied value before the plan is rendered. Only that form is supported:
    /// any other `$`-prefixed reference (for example a derived `$normalized_*`
    /// value this layer cannot compute) is rejected rather than left in the plan
    /// as a literal.
    ///
    /// # Errors
    ///
    /// Adds [`TaskPackageError::MissingInput`] when a reference has no supplied
    /// value, and [`TaskPackageError::UnsupportedReference`] for any other
    /// `$`-prefixed reference. Every error of [`Self::from_package_json`] applies.
    pub fn from_package_json_with_inputs(
        package_json: &str,
        inputs: &serde_json::Map<String, Value>,
    ) -> Result<Self, TaskPackageError> {
        let package: DeclaredPackage =
            serde_json::from_str(package_json).map_err(|error| TaskPackageError::Malformed {
                reason: error.to_string(),
            })?;
        // Task packages use dotted ids for human readability, while the task
        // engine accepts only ASCII letters, digits, `_`, and `-`. Normalize
        // deterministically at the binary boundary and retain the declared id
        // for diagnostics; this is not a model-generated value.
        let declared_task_id = package.task_id.clone();
        let normalized_task_id = normalize_task_id(&declared_task_id);
        let task_id =
            TaskId::new(normalized_task_id).map_err(|error| TaskPackageError::Malformed {
                reason: format!("task package task_id is invalid after normalization: {error}"),
            })?;
        let goal = package
            .goal
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("Execute task package {}", task_id.as_str()));
        let model_id =
            ModelId::new(TASK_PACKAGE_MODEL_ID).map_err(|error| TaskPackageError::Malformed {
                reason: error.to_string(),
            })?;
        let (plan_json, declared_not_executed, dataflow) = render_plan(&package, inputs)?;
        Ok(Self {
            model_id,
            declared_task_id,
            task_id,
            goal,
            plan_json,
            declared_not_executed,
            dataflow,
        })
    }

    /// Builds a provider from a task package file on disk.
    ///
    /// # Errors
    ///
    /// Returns [`TaskPackageError::Read`] when the file cannot be read, and the
    /// same errors as [`Self::from_package_json`] otherwise.
    pub fn from_package_file(path: &Path) -> Result<Self, TaskPackageError> {
        Self::from_package_file_with_inputs(path, &serde_json::Map::new())
    }

    /// Builds a provider from a task package file plus explicit inputs.
    ///
    /// # Errors
    ///
    /// Returns [`TaskPackageError::Read`] when the file cannot be read, and the
    /// same errors as [`Self::from_package_json_with_inputs`] otherwise.
    pub fn from_package_file_with_inputs(
        path: &Path,
        inputs: &serde_json::Map<String, Value>,
    ) -> Result<Self, TaskPackageError> {
        let body = std::fs::read_to_string(path).map_err(|error| TaskPackageError::Read {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        Self::from_package_json_with_inputs(&body, inputs)
    }

    /// Returns the rendered plan JSON handed to the Planner.
    #[must_use]
    pub fn plan_json(&self) -> &str {
        &self.plan_json
    }

    /// Returns the package steps this runtime does **not** execute.
    ///
    /// ADR-0059 (D6 as amended) keeps four kinds - `platform`, `l1_file`, `pure`
    /// and `policy` - owned elsewhere. They are reported here rather than inside
    /// the plan JSON, because the Planner accepts exactly one field (`steps`) and
    /// rejects anything else; the assembly point can log them instead.
    #[must_use]
    pub fn declared_not_executed(&self) -> &[Value] {
        &self.declared_not_executed
    }

    /// Returns the side-channel dataflow contract for the rendered Plan.
    #[must_use]
    pub const fn dataflow_plan(&self) -> &RuntimeDataflowPlan {
        &self.dataflow
    }

    /// Returns the task identifier declared by the package.
    #[must_use]
    pub const fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Returns the original, human-readable task identifier declared by the package.
    #[must_use]
    pub fn declared_task_id(&self) -> &str {
        &self.declared_task_id
    }

    /// Returns the goal declared by the package.
    #[must_use]
    pub fn goal(&self) -> &str {
        &self.goal
    }
}

impl ModelProvider for TaskPackageProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::new(ProviderFeatures::STREAMING, TokenCount::new(8_192))
    }

    fn pricing(&self) -> Pricing {
        // A local deterministic source consumes no paid capacity.
        Pricing::new(0, 0, 0)
    }

    fn count_tokens(&self, messages: &[Message]) -> ModelResult<TokenCount> {
        let characters: u64 = messages
            .iter()
            .map(|message| u64::try_from(message.content.chars().count()).unwrap_or(u64::MAX))
            .sum();
        Ok(TokenCount::new(characters.max(1)))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        if cancellation.is_cancelled() {
            return Err(ModelGatewayError::Cancelled {
                model_id: Some(self.model_id.clone()),
                elapsed: DurationMs::new(0),
            });
        }
        Ok(Box::new(TaskPackageStream {
            step: 0,
            plan_json: self.plan_json.clone(),
        }))
    }
}

/// Single-delta stream: plan JSON, terminal usage, then a stop event.
struct TaskPackageStream {
    step: u8,
    plan_json: String,
}

impl CompletionStream for TaskPackageStream {
    fn next_event(
        &mut self,
        _cancellation: &CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        let event = match self.step {
            0 => CompletionEvent::TextDelta(self.plan_json.clone()),
            1 => CompletionEvent::Usage(Usage::new(
                TokenCount::new(1),
                TokenCount::new(1),
                TokenCount::new(1),
            )),
            2 => CompletionEvent::Finished(FinishReason::Stop),
            _ => return Ok(None),
        };
        self.step = self.step.saturating_add(1);
        Ok(Some(event))
    }
}

/// Renders the executable subset of a declared package as Planner input.
fn render_plan(
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
    // The model output carries exactly one field (`steps`); the Planner rejects
    // anything else, so the not-executed kinds travel back out of band instead of
    // being smuggled into the plan.
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
    if step.kind == "pure"
        && step.operation.as_deref() == Some("count_lines_and_keyword_paragraphs")
    {
        declared_not_executed.push(json!({ "id": step.id, "kind": step.kind }));
        return Ok(None);
    }
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
    Ok(Some(json!({
        "id": step.id,
        "sequence": sequence,
        "tool": tool,
        "args": arguments,
        "depends_on": [],
        "postconditions": postconditions,
        "point_of_no_return": false,
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
        // A later step may deliberately rebind an output name (for example
        // "expected count" -> "observed count"). References always resolve to
        // the latest producer that has already committed.
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

/// Resolves the step arguments against the supplied task inputs.
///
/// Explicit inputs are substituted immediately. References to previously
/// declared step outputs remain as whole-value `$name` placeholders for the
/// runtime binder. An empty input map keeps the older "unbound declaration"
/// error for input-like references.
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
            // One rule for every reference form the packages use (`$input.*`,
            // `$rollback.*`, `$approval_diff`, ...): strip the leading `$` and
            // resolve it from the injected map. Anything the caller did not
            // supply fails closed - this layer never computes a derived value and
            // never leaves a literal behind.
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

fn normalize_task_id(value: &str) -> String {
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

/// The 1a assertion table (DRIFT-214-1).
///
/// Every arm returns postconditions in the exact shape
/// `assistant_verify::parse_postconditions` accepts, and every value is either
/// fixed or taken from the step's own arguments. A tool absent from this table
/// has no 1a assertion and is rejected at construction time.
fn assertion_table(tool: &str, arguments: &Value) -> Option<Vec<Value>> {
    match tool {
        // A read must leave the document, title, and tab set untouched.
        "notepad.file.read_text" => Some(vec![json!({ "kind": "state_unchanged" })]),
        // A replace must actually produce the new text and change state.
        "notepad.file.replace_text" => {
            let new_text = arguments.get("new_text")?.as_str()?;
            Some(vec![
                json!({ "kind": "text_contains", "value": new_text }),
                json!({ "kind": "state_changed", "within_ms": STATE_CHANGED_WINDOW_MS }),
            ])
        }
        // Save and new-tab are state writes whose postcondition is the
        // transition itself; the file-level proof belongs to the save-as tool.
        "notepad.file.save" | "notepad.tab.new" => Some(vec![
            json!({ "kind": "state_changed", "within_ms": STATE_CHANGED_WINDOW_MS }),
        ]),
        // Save-as is only complete when the target file changed.
        "notepad.file.save_as" => {
            let target_path = arguments.get("target_path")?.as_str()?;
            Some(vec![json!({
                "kind": "file_changed",
                "path": target_path,
                "expect": "any",
            })])
        }
        // The host text write is the one reserved step with a real application
        // side effect; every other reserved operation is read-only/runtime-only.
        crate::runtime_tools::TOOL_HOST_SET_EDITOR_VALUE => Some(vec![json!({
            "kind": "state_changed",
            "within_ms": STATE_CHANGED_WINDOW_MS,
        })]),
        // ADR-0060 / ADR-0061: reserved runtime and pure operations do not alter
        // the application, so they must prove the fingerprint stayed unchanged.
        other if crate::runtime_tools::RESERVED_RUNTIME_TOOLS.contains(&other) => {
            Some(vec![json!({ "kind": "state_unchanged" })])
        }
        _ => None,
    }
}
