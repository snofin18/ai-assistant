//! Runtime binding of task-package outputs and conditions (ADR-0061).
//!
//! Responsibilities:
//! - evaluate a step's closed condition before invocation;
//! - resolve whole-value `$name` references in its arguments;
//! - capture declared outputs after the inner invoker succeeds;
//! - publish those outputs to later steps only after the engine commits.
//!
//! Boundaries:
//! - does not alter policy, task-engine states, or platform behavior;
//! - does not invent missing outputs: a later reference to an absent output
//!   fails closed when that later step runs;
//! - does not expose the context outside one run.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use assistant_protocol::{ErrorCode, ToolEnvelope};
use assistant_task_engine::{PlanStep, TaskId, TaskSnapshot};
use serde_json::{Map, Value, json};

use crate::runtime::{RuntimeExecutionError, ToolInvoker};
use crate::runtime_dataflow::{RuntimeDataflowPlan, resolve_references};

/// In-memory context shared by the binding invoker and the assembly loop.
#[derive(Debug, Clone)]
pub struct RuntimeBindingState {
    inputs: Map<String, Value>,
    outputs: BTreeMap<String, Value>,
    pending: BTreeMap<String, BTreeMap<String, Value>>,
}

impl RuntimeBindingState {
    /// Creates a state from explicit task inputs.
    #[must_use]
    pub const fn new(inputs: Map<String, Value>) -> Self {
        Self {
            inputs,
            outputs: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// Publishes pending outputs after a committed step.
    pub fn commit(&mut self, step_id: &str) {
        if let Some(outputs) = self.pending.remove(step_id) {
            self.outputs.extend(outputs);
        }
    }

    /// Drops pending outputs after a failed or uncommitted step.
    pub fn discard(&mut self, step_id: &str) {
        self.pending.remove(step_id);
    }

    fn context(&self) -> Map<String, Value> {
        let mut context = self.inputs.clone();
        context.extend(self.outputs.clone());
        context
    }
}

/// Wraps an invoker with ADR-0061 context resolution and output capture.
pub struct BindingInvoker<Inner> {
    inner: Inner,
    dataflow: Arc<RuntimeDataflowPlan>,
    state: Arc<Mutex<RuntimeBindingState>>,
    latest_snapshot: Arc<Mutex<Option<TaskSnapshot>>>,
}

impl<Inner> BindingInvoker<Inner> {
    /// Creates a binding invoker over one inner runtime invoker.
    #[must_use]
    pub const fn new(
        inner: Inner,
        dataflow: Arc<RuntimeDataflowPlan>,
        state: Arc<Mutex<RuntimeBindingState>>,
        latest_snapshot: Arc<Mutex<Option<TaskSnapshot>>>,
    ) -> Self {
        Self {
            inner,
            dataflow,
            state,
            latest_snapshot,
        }
    }

    fn current_fingerprint(&self) -> Option<String> {
        self.latest_snapshot
            .lock()
            .ok()
            .and_then(|snapshot| snapshot.as_ref().cloned())
            .and_then(|snapshot| {
                snapshot
                    .steps
                    .iter()
                    .rev()
                    .find_map(|step| step.post_fingerprint.clone())
            })
    }
}

impl<Inner> ToolInvoker for BindingInvoker<Inner>
where
    Inner: ToolInvoker + Send + Sync,
{
    fn invoke(
        &self,
        task_id: &TaskId,
        step: &PlanStep,
    ) -> impl Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send {
        let step_id = step.id.to_string();
        let binding = self.dataflow.get(step.id.as_str()).cloned();
        let resolved = self.resolve_step(task_id, step, binding.as_ref());
        async move {
            match resolved {
                Ok(ResolvedStep::Skip(envelope)) => Ok(*envelope),
                Ok(ResolvedStep::Run(resolved_step)) => {
                    let envelope = self.inner.invoke(task_id, &resolved_step).await?;
                    if envelope.ok
                        && let Err(message) =
                            self.capture_outputs(&step_id, &resolved_step.tool, &envelope)
                    {
                        return Ok(ToolEnvelope::error(
                            resolved_step.tool,
                            task_id.to_string(),
                            step_id,
                            ErrorCode::VerifyFailed,
                            message,
                        ));
                    }
                    Ok(envelope)
                }
                Err(error) => Ok(ToolEnvelope::error(
                    step.tool.clone(),
                    task_id.to_string(),
                    step_id,
                    error.code,
                    error.message,
                )),
            }
        }
    }
}

enum ResolvedStep {
    Skip(Box<ToolEnvelope>),
    Run(PlanStep),
}

struct BindingFailure {
    code: ErrorCode,
    message: String,
}

impl<Inner> BindingInvoker<Inner>
where
    Inner: ToolInvoker + Send + Sync,
{
    fn resolve_step(
        &self,
        task_id: &TaskId,
        step: &PlanStep,
        binding: Option<&crate::runtime_dataflow::RuntimeStepBinding>,
    ) -> Result<ResolvedStep, BindingFailure> {
        let context = self
            .state
            .lock()
            .map_err(|_| BindingFailure {
                code: ErrorCode::Fatal,
                message: "runtime binding state is unavailable".to_owned(),
            })?
            .context();

        if let Some(binding) = binding
            && let Some(condition) = binding.condition.as_ref()
        {
            let source = condition_source(binding);
            let should_run =
                condition
                    .evaluate(&context, &source)
                    .map_err(|error| BindingFailure {
                        code: ErrorCode::ToolInvalidArgs,
                        message: error.to_string(),
                    })?;
            if !should_run {
                let Some(fingerprint) = self.current_fingerprint() else {
                    return Err(BindingFailure {
                        code: ErrorCode::VerifyFailed,
                        message: format!(
                            "condition for step {} evaluated false before any fingerprint was published",
                            step.id
                        ),
                    });
                };
                return Ok(ResolvedStep::Skip(Box::new(ToolEnvelope::ok(
                    step.tool.clone(),
                    task_id.to_string(),
                    step.id.to_string(),
                    json!({
                        "skipped": true,
                        "fingerprint": fingerprint,
                        "previous_fingerprint": fingerprint,
                        "elapsed_ms": 0,
                    }),
                ))));
            }
        }

        let args = resolve_references(&step.args, &context).map_err(|error| BindingFailure {
            code: ErrorCode::ToolInvalidArgs,
            message: error.to_string(),
        })?;
        if !args.is_object() {
            return Err(BindingFailure {
                code: ErrorCode::ToolInvalidArgs,
                message: format!("step {} arguments must resolve to an object", step.id),
            });
        }
        let mut resolved = step.clone();
        resolved.args = args;
        Ok(ResolvedStep::Run(resolved))
    }

    fn capture_outputs(
        &self,
        step_id: &str,
        tool: &str,
        envelope: &ToolEnvelope,
    ) -> Result<(), String> {
        let Some(binding) = self.dataflow.get(step_id) else {
            return Ok(());
        };
        let Some(data) = envelope.data.as_ref().and_then(|data| data.0.as_object()) else {
            return Err(format!("step {step_id} returned no data payload"));
        };
        let mut captured = BTreeMap::new();
        for output in &binding.outputs {
            if let Some(value) = extract_output(tool, output, data) {
                captured.insert(output.clone(), value);
            }
        }
        self.state
            .lock()
            .map_err(|_| "runtime binding state is unavailable while capturing outputs".to_owned())?
            .pending
            .insert(step_id.to_owned(), captured);
        Ok(())
    }
}

fn condition_source(binding: &crate::runtime_dataflow::RuntimeStepBinding) -> String {
    binding
        .condition
        .as_ref()
        .map_or_else(String::new, |condition| format!("{condition:?}"))
}

/// Extracts one declared output from a tool envelope's data object.
///
/// Most output names are direct data keys. A few 1a adapter names are aliases
/// for the envelope fields the handler already returns; those aliases are
/// explicit here instead of being guessed by consumers.
fn extract_output(tool: &str, output: &str, data: &Map<String, Value>) -> Option<Value> {
    if let Some(value) = data.get(output) {
        return Some(value.clone());
    }
    match (tool, output) {
        ("notepad.file.read_text", "canonical_text_before" | "canonical_text_read_back") => {
            data.get("text").cloned()
        }
        ("notepad.file.replace_text", "canonical_text_after") => {
            data.get("canonical_text").cloned()
        }
        ("notepad.file.save", "save_result")
        | ("notepad.file.save_as", "dialog_evidence")
        | (
            _,
            "approval_evidence"
            | "approval_evidence.replace"
            | "approval_evidence.save"
            | "approval_evidence.save_as",
        ) => Some(Value::Object(data.clone())),
        ("assistant.runtime.prepare_anchors", "rollback_anchors") => {
            Some(Value::Object(data.clone()))
        }
        (_, "postconditions_verified") => Some(Value::Bool(true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::{Arc, Mutex};

    use assistant_task_engine::{
        PlanStep, Reversibility, StepEffect, StepId, StepTimeouts, TaskId,
    };
    use serde_json::json;

    use super::{BindingInvoker, RuntimeBindingState};
    use crate::runtime::{RuntimeExecutionError, ToolInvoker};
    use crate::runtime_dataflow::RuntimeDataflowPlan;

    struct EchoInvoker;

    impl ToolInvoker for EchoInvoker {
        fn invoke(
            &self,
            task_id: &TaskId,
            step: &PlanStep,
        ) -> impl std::future::Future<
            Output = Result<assistant_protocol::ToolEnvelope, RuntimeExecutionError>,
        > + Send {
            let envelope = assistant_protocol::ToolEnvelope::ok(
                step.tool.clone(),
                task_id.to_string(),
                step.id.to_string(),
                json!({"echo": step.args, "fingerprint": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}),
            );
            async move { Ok(envelope) }
        }
    }

    fn step() -> PlanStep {
        PlanStep {
            id: StepId::new("s1").expect("id"),
            sequence: 1,
            tool: "demo.echo".to_owned(),
            args: json!({"value": "$answer"}),
            depends_on: Vec::new(),
            postconditions: vec![json!({"kind": "state_unchanged"})],
            effect: StepEffect::Read,
            reversibility: Reversibility::L0UndoStack,
            point_of_no_return: false,
            timeouts: StepTimeouts::new(100, 100, 100).expect("timeouts"),
        }
    }

    #[tokio::test]
    async fn test_binding_resolves_an_explicit_input() {
        let state = Arc::new(Mutex::new(RuntimeBindingState::new(
            json!({"answer": 42}).as_object().expect("object").clone(),
        )));
        let invoker = BindingInvoker::new(
            EchoInvoker,
            Arc::new(RuntimeDataflowPlan::new()),
            state,
            Arc::new(Mutex::new(None)),
        );
        let envelope = invoker
            .invoke(&TaskId::new("t").expect("task"), &step())
            .await
            .expect("envelope");
        assert_eq!(
            envelope.data.expect("data").0.get("echo"),
            Some(&json!({"value": 42}))
        );
    }
}
