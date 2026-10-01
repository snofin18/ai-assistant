//! Binary-layer invoker for the reserved runtime tools (ADR-0060).
//!
//! Responsibilities:
//! - recognise the reserved runtime tool names and handle them locally instead
//!   of forwarding them to the model-visible `ToolBus`;
//! - forward every other tool unchanged, keeping the bus the single path for
//!   ordinary tools.
//!
//! Boundaries:
//! - does not implement the three executors yet; a reserved step currently
//!   returns a **known** failure envelope rather than being mistaken for an
//!   unknown outcome;
//! - does not decide policy, resolve targets, or advance task state.
//!
//! Invariants:
//! 1. a reserved name never reaches `ToolBus::call_tool` (it is not mounted);
//! 2. a reserved step always yields `Ok(envelope)` - an explicit `ErrorCode`,
//!    never `Err`, so the executor reports `ToolFailed` and not `NeedsHuman`;
//! 3. an ordinary tool behaves exactly as `ToolBusInvoker` did.

use std::future::Future;
use std::sync::{Arc, Mutex};

use assistant_protocol::serde_json::{Map, Value, json};
use assistant_protocol::{ErrorCode, ToolEnvelope};
use assistant_task_engine::{PlanStep, StepStatus, TaskId, TaskSnapshot};
use assistant_tool_bus::{CallContext, ToolBus};

use crate::runtime::{RuntimeExecutionError, ToolInvoker};

/// Reason reported while a reserved runtime executor is still missing.
pub const RESERVED_EXECUTOR_MISSING: &str = "reserved runtime tool executor is not implemented yet";

/// Routes reserved runtime tools locally and everything else to the tool bus.
pub struct ReservedRuntimeInvoker<'bus> {
    bus: &'bus ToolBus,
    latest_snapshot: Arc<Mutex<Option<TaskSnapshot>>>,
}

impl<'bus> ReservedRuntimeInvoker<'bus> {
    /// Borrows a started tool bus plus the assembly-owned snapshot the runtime
    /// publishes after every committed step.
    #[must_use]
    pub const fn new(
        bus: &'bus ToolBus,
        latest_snapshot: Arc<Mutex<Option<TaskSnapshot>>>,
    ) -> Self {
        Self {
            bus,
            latest_snapshot,
        }
    }
}

impl ToolInvoker for ReservedRuntimeInvoker<'_> {
    fn invoke(
        &self,
        task_id: &TaskId,
        step: &PlanStep,
    ) -> impl Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send {
        let tool = step.tool.clone();
        let arguments = step.args.as_object().cloned();
        let call = CallContext::new(task_id.as_str(), step.id.as_str());
        let step_id = step.id.to_string();
        let sequence = step.sequence;
        async move {
            if tool == crate::runtime_tools::TOOL_VERIFY_POSTCONDITIONS {
                return self.verify_postconditions(&call, &step_id, sequence);
            }
            if tool == crate::runtime_tools::TOOL_PREPARE_ANCHORS {
                return self.prepare_anchors(&call, &step_id, sequence, arguments.as_ref());
            }
            if crate::runtime_tools::RESERVED_RUNTIME_TOOLS.contains(&tool.as_str()) {
                return Ok(ToolEnvelope::error(
                    tool,
                    call.task_id().to_owned(),
                    step_id,
                    ErrorCode::CapabilityMissing,
                    RESERVED_EXECUTOR_MISSING,
                ));
            }
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

impl ReservedRuntimeInvoker<'_> {
    /// ADR-0060 D5: assert every earlier step already committed, then report the
    /// fingerprint the app is still sitting at.
    ///
    /// A reserved step has **no application side effect**, so the honest "current"
    /// fingerprint is the one the last committed step recorded. Reading it from
    /// the snapshot keeps this executor free of platform calls - which is also
    /// what makes `state_unchanged` evaluable at all (the collector requires a
    /// fingerprint in the envelope).
    ///
    /// Every failure is a `ToolEnvelope` carrying an `ErrorCode`, never `Err`, so
    /// the executor reports `ToolFailed` rather than "unknown outcome".
    fn verify_postconditions(
        &self,
        call: &CallContext,
        step_id: &str,
        sequence: u32,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let refused = |code: ErrorCode, message: String| {
            ToolEnvelope::error(
                crate::runtime_tools::TOOL_VERIFY_POSTCONDITIONS.to_owned(),
                call.task_id().to_owned(),
                step_id.to_owned(),
                code,
                message,
            )
        };
        let Some(snapshot) = self.current_snapshot()? else {
            return Ok(refused(
                ErrorCode::VerifyFailed,
                "no task snapshot has been published yet; cannot verify prior steps".to_owned(),
            ));
        };
        let unmet: Vec<String> = snapshot
            .steps
            .iter()
            .filter(|prior| prior.sequence < sequence && prior.status != StepStatus::Committed)
            .map(|prior| format!("{}={:?}", prior.id.as_str(), prior.status))
            .collect();
        if !unmet.is_empty() {
            return Ok(refused(
                ErrorCode::VerifyFailed,
                format!("prior steps are not committed: {}", unmet.join(", ")),
            ));
        }
        let fingerprint = snapshot
            .steps
            .iter()
            .filter(|prior| prior.sequence < sequence)
            .filter_map(|prior| prior.post_fingerprint.clone())
            .next_back();
        let Some(fingerprint) = fingerprint else {
            return Ok(refused(
                ErrorCode::VerifyFailed,
                "no prior step recorded a post fingerprint; cannot prove state is unchanged"
                    .to_owned(),
            ));
        };
        Ok(ToolEnvelope::ok(
            crate::runtime_tools::TOOL_VERIFY_POSTCONDITIONS.to_owned(),
            call.task_id().to_owned(),
            step_id.to_owned(),
            json!({ "fingerprint": fingerprint }),
        ))
    }

    /// ADR-0059 D3 / ADR-0060 D4: decide and **validate** the anchors a write step
    /// requires, and refuse early when they cannot be honoured, so the step never
    /// reaches Execute.
    ///
    /// Scope, stated plainly: this executor validates the declared levels and the
    /// recipe references and reports the plan. **Capturing the physical snapshot is
    /// the Host/adapter's job at Execute time.** What this step guarantees is the
    /// contract's real content - *do not proceed without a viable anchor plan* -
    /// and it guarantees it by failing here.
    fn prepare_anchors(
        &self,
        call: &CallContext,
        step_id: &str,
        sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let refused = |code: ErrorCode, message: String| {
            ToolEnvelope::error(
                crate::runtime_tools::TOOL_PREPARE_ANCHORS.to_owned(),
                call.task_id().to_owned(),
                step_id.to_owned(),
                code,
                message,
            )
        };
        let Some(arguments) = arguments else {
            return Ok(refused(
                ErrorCode::ToolInvalidArgs,
                "prepare_anchors requires arguments".to_owned(),
            ));
        };
        let Some(levels) = arguments.get("required_levels").and_then(Value::as_array) else {
            return Ok(refused(
                ErrorCode::ToolInvalidArgs,
                "required_levels must be an array".to_owned(),
            ));
        };
        if levels.is_empty() {
            return Ok(refused(
                ErrorCode::ToolInvalidArgs,
                "required_levels must not be empty".to_owned(),
            ));
        }
        for level in levels {
            let Some(name) = level.as_str() else {
                return Ok(refused(
                    ErrorCode::ToolInvalidArgs,
                    "required_levels entries must be strings".to_owned(),
                ));
            };
            match name {
                "l0_undo_stack" | "l1_snapshot" | "l2_compensation" => {}
                "l3_irreversible" => {
                    return Ok(refused(
                        ErrorCode::PolicyDenied,
                        "an irreversible step cannot be anchored; it requires human confirmation"
                            .to_owned(),
                    ));
                }
                other => {
                    return Ok(refused(
                        ErrorCode::ToolInvalidArgs,
                        format!("unknown anchor level `{other}`"),
                    ));
                }
            }
        }
        for key in ["replace_recipe", "save_recipe"] {
            match arguments.get(key).and_then(Value::as_str) {
                Some(value) if !value.trim().is_empty() => {}
                _ => {
                    return Ok(refused(
                        ErrorCode::ToolInvalidArgs,
                        format!("{key} must be a non-empty string"),
                    ));
                }
            }
        }
        let Some(fingerprint) = self.current_fingerprint(sequence)? else {
            return Ok(refused(
                ErrorCode::VerifyFailed,
                "no prior step recorded a post fingerprint; cannot prove state is unchanged"
                    .to_owned(),
            ));
        };
        Ok(ToolEnvelope::ok(
            crate::runtime_tools::TOOL_PREPARE_ANCHORS.to_owned(),
            call.task_id().to_owned(),
            step_id.to_owned(),
            json!({ "anchor_levels": levels, "fingerprint": fingerprint }),
        ))
    }

    /// The fingerprint the app is currently sitting at: the post fingerprint of
    /// the last committed step before `sequence`.
    fn current_fingerprint(&self, sequence: u32) -> Result<Option<String>, RuntimeExecutionError> {
        let Some(snapshot) = self.current_snapshot()? else {
            return Ok(None);
        };
        Ok(snapshot
            .steps
            .iter()
            .filter(|prior| prior.sequence < sequence)
            .filter_map(|prior| prior.post_fingerprint.clone())
            .next_back())
    }

    /// Copies the published snapshot out of the lock in a single expression, so
    /// the guard is released immediately instead of spanning the whole check.
    fn current_snapshot(&self) -> Result<Option<TaskSnapshot>, RuntimeExecutionError> {
        self.latest_snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| RuntimeExecutionError::Tool {
                reason: "latest task snapshot mutex is poisoned".to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::RESERVED_EXECUTOR_MISSING;
    use crate::runtime_tools::{TOOL_PREPARE_ANCHORS, TOOL_REQUEST_APPROVAL};

    #[test]
    fn test_reserved_reason_is_stable_and_names_no_model_visibility() {
        assert!(RESERVED_EXECUTOR_MISSING.contains("not implemented"));
        assert!(TOOL_REQUEST_APPROVAL.starts_with("assistant.runtime."));
        assert!(TOOL_PREPARE_ANCHORS.starts_with("assistant.runtime."));
    }
}
