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

use std::path::Path;

use assistant_model_gateway::{
    CancellationToken, CompletionEvent, CompletionRequest, CompletionStream, DurationMs,
    FinishReason, Message, ModelGatewayError, ModelId, ModelProvider, ModelResult, Pricing,
    ProviderCapabilities, ProviderFeatures, TokenCount, Usage,
};
use assistant_task_engine::TaskId;
use serde::Deserialize;
use serde_json::{Value, json};

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
    tool: Option<String>,
    #[serde(default)]
    args: Value,
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
        let plan_json = render_plan(&package)?;
        Ok(Self {
            model_id,
            declared_task_id,
            task_id,
            goal,
            plan_json,
        })
    }

    /// Builds a provider from a task package file on disk.
    ///
    /// # Errors
    ///
    /// Returns [`TaskPackageError::Read`] when the file cannot be read, and the
    /// same errors as [`Self::from_package_json`] otherwise.
    pub fn from_package_file(path: &Path) -> Result<Self, TaskPackageError> {
        let body = std::fs::read_to_string(path).map_err(|error| TaskPackageError::Read {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        Self::from_package_json(&body)
    }

    /// Returns the rendered plan JSON handed to the Planner.
    #[must_use]
    pub fn plan_json(&self) -> &str {
        &self.plan_json
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
fn render_plan(package: &DeclaredPackage) -> Result<String, TaskPackageError> {
    let mut steps = Vec::new();
    let mut sequence: u32 = 1;
    for step in &package.steps {
        if step.kind != "tool" {
            continue;
        }
        let Some(tool) = step.tool.as_deref() else {
            return Err(TaskPackageError::Malformed {
                reason: format!(
                    "task package step `{}` is kind=tool but declares no tool",
                    step.id
                ),
            });
        };
        let arguments = normalize_arguments(&package.task_id, step)?;
        let Some(postconditions) = assertion_table(tool, &arguments) else {
            return Err(TaskPackageError::MissingAssertion {
                task_id: package.task_id.clone(),
                step_id: step.id.clone(),
                tool: tool.to_owned(),
            });
        };
        steps.push(json!({
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
        }));
        sequence = sequence.saturating_add(1);
    }
    if steps.is_empty() {
        return Err(TaskPackageError::NoToolSteps {
            task_id: package.task_id.clone(),
        });
    }
    serde_json::to_string(&json!({ "steps": steps })).map_err(|error| TaskPackageError::Malformed {
        reason: error.to_string(),
    })
}

/// Returns the step arguments, refusing declarations that still bind inputs.
fn normalize_arguments(task_id: &str, step: &DeclaredStep) -> Result<Value, TaskPackageError> {
    if contains_placeholder(&step.args) {
        return Err(TaskPackageError::UnboundArguments {
            task_id: task_id.to_owned(),
            step_id: step.id.clone(),
        });
    }
    if step.args.is_object() {
        return Ok(step.args.clone());
    }
    if step.args.is_null() {
        return Ok(json!({}));
    }
    Err(TaskPackageError::Malformed {
        reason: format!(
            "task package step `{}` arguments must be a JSON object",
            step.id
        ),
    })
}

/// True when the declaration still carries an `$input.` binding anywhere.
fn contains_placeholder(value: &Value) -> bool {
    match value {
        Value::String(text) => text.starts_with("$input."),
        Value::Array(items) => items.iter().any(contains_placeholder),
        Value::Object(fields) => fields.values().any(contains_placeholder),
        _ => false,
    }
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
        _ => None,
    }
}
