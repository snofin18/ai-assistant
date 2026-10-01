//! Binary-layer runtime execution coordinator.
//!
//! Responsibilities:
//! - advance one `PlanStep` through the ADR-0056 order;
//! - enforce policy and approval checks before tool execution;
//! - commit only with an opaque verification receipt.
//!
//! Boundaries:
//! - no platform calls, policy semantics, tool semantics, or UI rendering;
//! - no SQL and no direct audit writes in this module;
//! - callers inject the tool invoker and observation collector.
//!
//! Invariants:
//! - a step is never committed unless verification minted a receipt;
//! - a tool envelope with `ok = false` fails the step instead of being verified;
//! - an unknown invocation outcome enters `NeedsHuman` and never retries.

use std::future::Future;

use assistant_platform_api::Fingerprint;
use assistant_policy::Decision;
use assistant_protocol::{ErrorCode, ToolEnvelope};
use assistant_task_engine::{
    CheckpointStore, PlanStep, StepCommit, StepId, TaskEngine, TaskEvent, TaskId, TaskSnapshot,
    TaskStatus,
};
use assistant_tool_bus::{CallContext, ToolBus};
use assistant_verify::{
    Observation, VerifyOutcome, parse_postconditions, verify_postconditions_with_receipt,
};
use thiserror::Error;

/// Policy gate used by the runtime executor.
pub trait StepPolicy {
    /// Returns the policy decision for one step.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when policy evaluation cannot produce a
    /// decision.
    fn decide(&self, step: &PlanStep) -> Result<Decision, RuntimeExecutionError>;
}

/// Tool invocation boundary used by the runtime executor.
///
/// The method returns a future so the executor can await the real in-process
/// MCP tool bus. It is deliberately not `async fn` in the trait: an explicit
/// `impl Future` keeps the returned future nameable without the
/// `async_fn_in_trait` lint that a public trait would otherwise trigger.
pub trait ToolInvoker {
    /// Invokes one registered tool and returns its structured envelope.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when the invocation boundary cannot produce a
    /// usable envelope.
    fn invoke(
        &self,
        task_id: &TaskId,
        step: &PlanStep,
    ) -> impl Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send;
}

/// Converts a tool result into the observation consumed by `verify`.
pub trait ObservationCollector {
    /// Collects a deterministic observation for one step result.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when the observation cannot be collected.
    fn observe(
        &self,
        step: &PlanStep,
        envelope: &ToolEnvelope,
    ) -> Result<Observation, RuntimeExecutionError>;
}

/// Structured runtime execution failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RuntimeExecutionError {
    /// The task engine rejected an operation.
    #[error(transparent)]
    TaskEngine(#[from] assistant_task_engine::TaskEngineError),

    /// The requested task is not currently running.
    #[error("task {task_id} is not running: {status:?}")]
    TaskNotRunning {
        /// Task identifier.
        task_id: String,
        /// Observed status.
        status: TaskStatus,
    },

    /// The requested step is absent from the task plan.
    #[error("step {step_id} is not present in task {task_id}")]
    StepNotFound {
        /// Task identifier.
        task_id: String,
        /// Missing step identifier.
        step_id: String,
    },

    /// Policy evaluation failed.
    #[error("policy evaluation failed: {reason}")]
    Policy {
        /// Human-readable failure reason.
        reason: String,
    },

    /// Tool invocation failed.
    #[error("tool invocation failed: {reason}")]
    Tool {
        /// Human-readable failure reason.
        reason: String,
    },

    /// Postconditions could not be parsed.
    #[error("postcondition parsing failed: {reason}")]
    Postconditions {
        /// Human-readable failure reason.
        reason: String,
    },

    /// Observation collection failed.
    #[error("observation collection failed: {reason}")]
    Observation {
        /// Human-readable failure reason.
        reason: String,
    },
}

/// Terminal outcome of one `advance` call.
#[derive(Debug)]
#[non_exhaustive]
pub enum StepExecutionOutcome {
    /// The step was verified and committed.
    Committed(TaskSnapshot),
    /// Policy denied the step.
    PolicyDenied(TaskSnapshot),
    /// Human approval is required before execution can continue.
    AwaitingApproval,
    /// The tool ran but returned a structured error envelope.
    ToolFailed {
        /// Latest task snapshot.
        snapshot: TaskSnapshot,
        /// Error code carried by the tool envelope.
        code: ErrorCode,
        /// Human-readable failure message from the envelope.
        message: String,
    },
    /// Verification failed or the execution outcome is unknown.
    NeedsHuman {
        /// Latest task snapshot.
        snapshot: TaskSnapshot,
        /// Why automation cannot safely continue.
        reason: String,
    },
    /// Verification returned a non-success outcome.
    VerificationFailed {
        /// Latest task snapshot.
        snapshot: TaskSnapshot,
        /// Original verify outcome.
        outcome: VerifyOutcome,
    },
}

/// Single-step runtime coordinator.
pub struct RuntimeExecutor<Store, Policy, Invoker, Observer> {
    engine: TaskEngine<Store>,
    policy: Policy,
    invoker: Invoker,
    observer: Observer,
}

impl<Store, Policy, Invoker, Observer> RuntimeExecutor<Store, Policy, Invoker, Observer>
where
    Store: CheckpointStore,
    Policy: StepPolicy,
    Invoker: ToolInvoker,
    Observer: ObservationCollector,
{
    /// Creates an executor over injected components.
    #[must_use]
    pub const fn new(
        engine: TaskEngine<Store>,
        policy: Policy,
        invoker: Invoker,
        observer: Observer,
    ) -> Self {
        Self {
            engine,
            policy,
            invoker,
            observer,
        }
    }

    /// Executes one ready step through the ADR-0056 order.
    ///
    /// # Errors
    ///
    /// Returns a structured error when policy, tool invocation, observation,
    /// postcondition parsing, or task-engine state transitions fail.
    pub async fn advance(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        now_ms: i64,
    ) -> Result<StepExecutionOutcome, RuntimeExecutionError> {
        let step = self.running_step(task_id, step_id)?;
        self.engine.begin_step(task_id, step_id, now_ms)?;
        if let Some(outcome) = self.apply_policy(task_id, step_id, &step, now_ms)? {
            return Ok(outcome);
        }
        self.engine.approve_step(task_id, step_id, now_ms)?;
        self.engine.begin_execute(task_id, step_id, now_ms)?;
        let envelope = match self.invoker.invoke(task_id, &step).await {
            Ok(envelope) => envelope,
            Err(error) => {
                return self.enter_needs_human(
                    task_id,
                    now_ms,
                    format!("tool outcome unknown: {error}"),
                );
            }
        };
        if !envelope.ok {
            return self.fail_from_envelope(task_id, step_id, &envelope, now_ms);
        }
        self.verify_and_commit(task_id, step_id, &step, &envelope, now_ms)
    }

    /// Loads the step about to run, failing closed when the task is not running
    /// or the step is absent from the plan.
    fn running_step(
        &self,
        task_id: &TaskId,
        step_id: &StepId,
    ) -> Result<PlanStep, RuntimeExecutionError> {
        let snapshot = self.engine.load_snapshot(task_id)?;
        if snapshot.status != TaskStatus::Running {
            return Err(RuntimeExecutionError::TaskNotRunning {
                task_id: task_id.to_string(),
                status: snapshot.status,
            });
        }
        snapshot
            .plan
            .steps
            .iter()
            .find(|candidate| &candidate.id == step_id)
            .cloned()
            .ok_or_else(|| RuntimeExecutionError::StepNotFound {
                task_id: task_id.to_string(),
                step_id: step_id.to_string(),
            })
    }

    /// Applies the policy gate. Returns `Some(outcome)` when the step must not
    /// execute and `None` only for an unconditional allow.
    fn apply_policy(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        step: &PlanStep,
        now_ms: i64,
    ) -> Result<Option<StepExecutionOutcome>, RuntimeExecutionError> {
        match self.policy.decide(step)? {
            Decision::Deny { .. } => {
                let snapshot = self.engine.deny_step(task_id, step_id, now_ms)?;
                Ok(Some(StepExecutionOutcome::PolicyDenied(snapshot)))
            }
            Decision::AllowWithConfirmation { .. } => {
                Ok(Some(StepExecutionOutcome::AwaitingApproval))
            }
            Decision::Allow { .. } => Ok(None),
        }
    }

    /// Escalates to a human without retrying a step whose outcome is unknown.
    fn enter_needs_human(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
        reason: String,
    ) -> Result<StepExecutionOutcome, RuntimeExecutionError> {
        let snapshot = self
            .engine
            .apply_task_event(task_id, TaskEvent::RequireHuman, now_ms)?;
        Ok(StepExecutionOutcome::NeedsHuman { snapshot, reason })
    }

    /// Fails a step whose tool returned a structured error envelope.
    fn fail_from_envelope(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        envelope: &ToolEnvelope,
        now_ms: i64,
    ) -> Result<StepExecutionOutcome, RuntimeExecutionError> {
        let code = envelope
            .error
            .as_ref()
            .map_or(ErrorCode::Fatal, |error| error.code);
        let message = envelope.error.as_ref().map_or_else(
            || "tool reported failure without an error payload".to_owned(),
            |error| error.message.clone(),
        );
        let snapshot = self.engine.fail_step(task_id, step_id, code, now_ms)?;
        Ok(StepExecutionOutcome::ToolFailed {
            snapshot,
            code,
            message,
        })
    }

    /// Verifies the observation and commits only when verify minted a receipt.
    fn verify_and_commit(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        step: &PlanStep,
        envelope: &ToolEnvelope,
        now_ms: i64,
    ) -> Result<StepExecutionOutcome, RuntimeExecutionError> {
        self.engine.begin_verify(task_id, step_id, now_ms)?;
        let postconditions = parse_postconditions(&step.postconditions).map_err(|error| {
            RuntimeExecutionError::Postconditions {
                reason: error.to_string(),
            }
        })?;
        let observation = match self.observer.observe(step, envelope) {
            Ok(observation) => observation,
            Err(error) => {
                return self.enter_needs_human(
                    task_id,
                    now_ms,
                    format!("observation unavailable: {error}"),
                );
            }
        };

        match verify_postconditions_with_receipt(&postconditions, &observation) {
            Ok(receipt) => {
                let post_fingerprint = Some(observation.fingerprint.as_str().to_owned());
                let snapshot = self.engine.commit_step(
                    task_id,
                    step_id,
                    StepCommit::new(receipt, post_fingerprint, false),
                    now_ms,
                )?;
                Ok(StepExecutionOutcome::Committed(snapshot))
            }
            Err(outcome) => {
                let snapshot =
                    self.engine
                        .fail_step(task_id, step_id, ErrorCode::VerifyFailed, now_ms)?;
                Ok(StepExecutionOutcome::VerificationFailed { snapshot, outcome })
            }
        }
    }

    /// Consumes the coordinator and returns the task engine.
    #[must_use]
    pub fn into_engine(self) -> TaskEngine<Store> {
        self.engine
    }
}

/// Invokes tools through the in-process MCP [`ToolBus`].
///
/// The executor owns this adapter by value; the bus itself is borrowed for the
/// duration of one run so the assembly point keeps ownership of the transport
/// and its shutdown path. Only serializable arguments and the returned
/// envelope cross this boundary, so no platform handle can leak into the core.
pub struct ToolBusInvoker<'bus> {
    bus: &'bus ToolBus,
}

impl<'bus> ToolBusInvoker<'bus> {
    /// Borrows a started tool bus for one runtime.
    #[must_use]
    pub const fn new(bus: &'bus ToolBus) -> Self {
        Self { bus }
    }
}

impl ToolInvoker for ToolBusInvoker<'_> {
    fn invoke(
        &self,
        task_id: &TaskId,
        step: &PlanStep,
    ) -> impl Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send {
        let tool = step.tool.clone();
        let arguments = step.args.as_object().cloned();
        let call = CallContext::new(task_id.as_str(), step.id.as_str());
        let step_id = step.id.to_string();
        async move {
            let Some(arguments) = arguments else {
                return Err(RuntimeExecutionError::Tool {
                    reason: format!("step {step_id} arguments must be a JSON object"),
                });
            };
            self.bus
                .call_tool(&tool, arguments, &call)
                .await
                .map_err(|error| RuntimeExecutionError::Tool {
                    reason: error.to_string(),
                })
        }
    }
}

/// Builds a verification observation from a successful tool envelope.
///
/// The Host must return a `fingerprint` string and may return `text` in the
/// envelope `data`. A missing fingerprint is an explicit observation failure
/// rather than a fabricated default, so an incomplete tool result escalates
/// instead of being verified against invented state.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvelopeObservationCollector;

impl ObservationCollector for EnvelopeObservationCollector {
    fn observe(
        &self,
        step: &PlanStep,
        envelope: &ToolEnvelope,
    ) -> Result<Observation, RuntimeExecutionError> {
        let data = envelope
            .data
            .as_ref()
            .ok_or_else(|| RuntimeExecutionError::Observation {
                reason: format!("tool {} returned no data payload", envelope.tool),
            })?;
        let fingerprint_value = data
            .0
            .get("fingerprint")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| RuntimeExecutionError::Observation {
                reason: format!("tool {} returned no fingerprint", envelope.tool),
            })?;
        let fingerprint = Fingerprint::parse(fingerprint_value).map_err(|error| {
            RuntimeExecutionError::Observation {
                reason: format!(
                    "tool {} returned an invalid fingerprint: {error}",
                    envelope.tool
                ),
            }
        })?;
        let text = data
            .0
            .get("text")
            .or_else(|| data.0.get("canonical_text"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let previous_fingerprint = match data
            .0
            .get("previous_fingerprint")
            .and_then(serde_json::Value::as_str)
        {
            Some(value) => Some(Fingerprint::parse(value).map_err(|error| {
                RuntimeExecutionError::Observation {
                    reason: format!(
                        "tool {} returned an invalid previous fingerprint: {error}",
                        envelope.tool
                    ),
                }
            })?),
            None => None,
        };
        let elapsed_since_previous_ms =
            data.0.get("elapsed_ms").and_then(serde_json::Value::as_u64);
        let mut observation =
            Observation::new(step.tool.clone(), envelope.tool.clone(), fingerprint);
        observation.text = text;
        observation.previous_fingerprint = previous_fingerprint;
        observation.elapsed_since_previous_ms = elapsed_since_previous_ms;
        if let Some(files) = data.0.get("files") {
            observation.files = serde_json::from_value(files.clone()).map_err(|error| {
                RuntimeExecutionError::Observation {
                    reason: format!(
                        "tool {} returned invalid file observations: {error}",
                        envelope.tool
                    ),
                }
            })?;
        }
        Ok(observation)
    }
}
