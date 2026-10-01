//! Reserved pure operations used by runtime `pure` steps.
//!
//! Each operation is a closed, deterministic function with no IO or platform
//! side effects; unknown operations never reach this module.

use std::path::{Component, PathBuf};

use assistant_protocol::ToolEnvelope;
use assistant_protocol::serde_json::{Map, Value, json};
use assistant_tool_bus::CallContext;

use super::{ReservedRuntimeInvoker, argument_error, render_text_diff};
use crate::runtime::RuntimeExecutionError;

impl ReservedRuntimeInvoker<'_> {
    pub(super) fn compute_literal_replacement(
        call: &CallContext,
        step_id: &str,
        _sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let Some(arguments) = arguments else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
                call,
                step_id,
                "compute_literal_replacement requires arguments",
            ));
        };
        let Some(text) = arguments.get("text").and_then(Value::as_str) else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
                call,
                step_id,
                "text must be a string",
            ));
        };
        let Some(old_text) = arguments.get("old_text").and_then(Value::as_str) else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
                call,
                step_id,
                "old_text must be a string",
            ));
        };
        let Some(new_text) = arguments.get("new_text").and_then(Value::as_str) else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
                call,
                step_id,
                "new_text must be a string",
            ));
        };
        if old_text.is_empty() {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
                call,
                step_id,
                "old_text must not be empty",
            ));
        }
        let replacement_count = text.matches(old_text).count();
        let canonical_text_expected = text.replace(old_text, new_text);
        ReservedRuntimeInvoker::ok_with_pure_fingerprint(
            crate::runtime_tools::TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
            call,
            step_id,
            &json!({
                "canonical_text_expected": canonical_text_expected,
                "replacement_count": replacement_count,
            }),
        )
    }

    pub(super) fn build_text_diff(
        call: &CallContext,
        step_id: &str,
        _sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let Some(arguments) = arguments else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_BUILD_TEXT_DIFF,
                call,
                step_id,
                "build_text_diff requires arguments",
            ));
        };
        let Some(before) = arguments.get("before").and_then(Value::as_str) else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_BUILD_TEXT_DIFF,
                call,
                step_id,
                "before must be a string",
            ));
        };
        let Some(after) = arguments.get("after").and_then(Value::as_str) else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_BUILD_TEXT_DIFF,
                call,
                step_id,
                "after must be a string",
            ));
        };
        if arguments
            .get("format")
            .and_then(Value::as_str)
            .is_some_and(|format| format != "line_diff")
        {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_BUILD_TEXT_DIFF,
                call,
                step_id,
                "format must be `line_diff`",
            ));
        }
        ReservedRuntimeInvoker::ok_with_pure_fingerprint(
            crate::runtime_tools::TOOL_PURE_BUILD_TEXT_DIFF,
            call,
            step_id,
            &json!({ "approval_diff": render_text_diff(before, after) }),
        )
    }

    pub(super) fn validate_t1_3_inputs(
        call: &CallContext,
        step_id: &str,
        _sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let Some(arguments) = arguments else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_VALIDATE_T1_3_INPUTS,
                call,
                step_id,
                "validate_t1_3_inputs requires arguments",
            ));
        };
        if arguments.get("text").and_then(Value::as_str).is_none()
            || arguments
                .get("expected_initial_tab_count")
                .and_then(Value::as_u64)
                .is_none()
        {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_VALIDATE_T1_3_INPUTS,
                call,
                step_id,
                "text and expected_initial_tab_count are required",
            ));
        }
        let Some(target_path) = arguments.get("target_path").and_then(Value::as_str) else {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_VALIDATE_T1_3_INPUTS,
                call,
                step_id,
                "target_path must be a string",
            ));
        };
        let path = PathBuf::from(target_path);
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Ok(argument_error(
                crate::runtime_tools::TOOL_PURE_VALIDATE_T1_3_INPUTS,
                call,
                step_id,
                "target_path must be absolute and must not contain `..`",
            ));
        }
        ReservedRuntimeInvoker::ok_with_pure_fingerprint(
            crate::runtime_tools::TOOL_PURE_VALIDATE_T1_3_INPUTS,
            call,
            step_id,
            &json!({ "normalized_target_path": path.to_string_lossy() }),
        )
    }
}
