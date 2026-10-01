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

use assistant_protocol::{ErrorCode, ToolEnvelope};
use assistant_task_engine::{PlanStep, TaskId};
use assistant_tool_bus::{CallContext, ToolBus};

use crate::runtime::{RuntimeExecutionError, ToolInvoker};

/// Reason reported while a reserved runtime executor is still missing.
pub const RESERVED_EXECUTOR_MISSING: &str = "reserved runtime tool executor is not implemented yet";

/// Routes reserved runtime tools locally and everything else to the tool bus.
pub struct ReservedRuntimeInvoker<'bus> {
    bus: &'bus ToolBus,
}

impl<'bus> ReservedRuntimeInvoker<'bus> {
    /// Borrows a started tool bus for one run.
    #[must_use]
    pub const fn new(bus: &'bus ToolBus) -> Self {
        Self { bus }
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
        async move {
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
