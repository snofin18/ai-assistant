//! Reserved host operations used by runtime `host_service` steps.
//!
//! The module validates serializable arguments and delegates to the injected
//! host operations; it does not decide policy or advance task state.

use assistant_protocol::serde_json::{Map, Value};
use assistant_protocol::{ErrorCode, ToolEnvelope};
use assistant_tool_bus::CallContext;

use super::{ReservedRuntimeInvoker, argument_error, capability_error};
use crate::runtime::RuntimeExecutionError;

impl ReservedRuntimeInvoker<'_> {
    pub(super) fn inspect_target_path(
        &self,
        call: &CallContext,
        step_id: &str,
        sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let Some(target_path) = arguments
            .and_then(|arguments| arguments.get("target_path"))
            .and_then(Value::as_str)
        else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_HOST_INSPECT_TARGET_PATH,
                call,
                step_id,
                "target_path must be a string",
            ));
        };
        let Some(operations) = self.host_operations.as_ref() else {
            return Ok(capability_error(
                crate::runtime_tools::TOOL_HOST_INSPECT_TARGET_PATH,
                call,
                step_id,
                "host target-path inspection is not assembled",
            ));
        };
        let data = match operations.inspect_target_path(target_path) {
            Ok(data) => data,
            Err(message) => {
                return Ok(ToolEnvelope::error(
                    crate::runtime_tools::TOOL_HOST_INSPECT_TARGET_PATH.to_owned(),
                    call.task_id().to_owned(),
                    step_id.to_owned(),
                    ErrorCode::ToolInvalidArgs,
                    message,
                ));
            }
        };
        self.ok_with_current_fingerprint(
            crate::runtime_tools::TOOL_HOST_INSPECT_TARGET_PATH,
            call,
            step_id,
            sequence,
            &data,
        )
    }

    pub(super) fn set_editor_value(
        &self,
        call: &CallContext,
        step_id: &str,
        sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let Some(text) = arguments
            .and_then(|arguments| arguments.get("text"))
            .and_then(Value::as_str)
        else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_HOST_SET_EDITOR_VALUE,
                call,
                step_id,
                "text must be a string",
            ));
        };
        let Some(operations) = self.host_operations.as_ref() else {
            return Ok(capability_error(
                crate::runtime_tools::TOOL_HOST_SET_EDITOR_VALUE,
                call,
                step_id,
                "host text writing is not assembled",
            ));
        };
        let data = match operations.set_editor_value(text) {
            Ok(data) => data,
            Err(message) => {
                return Ok(ToolEnvelope::error(
                    crate::runtime_tools::TOOL_HOST_SET_EDITOR_VALUE.to_owned(),
                    call.task_id().to_owned(),
                    step_id.to_owned(),
                    ErrorCode::VerifyFailed,
                    message,
                ));
            }
        };
        self.ok_with_current_fingerprint(
            crate::runtime_tools::TOOL_HOST_SET_EDITOR_VALUE,
            call,
            step_id,
            sequence,
            &data,
        )
    }
}
