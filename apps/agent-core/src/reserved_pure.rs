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

/// Default paragraph cap for `count_lines_and_keyword_paragraphs` when the caller omits it.
const DEFAULT_MAX_KEYWORD_PARAGRAPHS: usize = 50;

impl ReservedRuntimeInvoker<'_> {
    /// Counts logical lines and paragraphs that contain one of the requested literal keywords.
    ///
    /// This is the T1.1 `analyze` operation. It is pure: it reads only its arguments and never
    /// touches the platform. An empty keyword list or a non-`LF` canonical form is refused
    /// (`ToolInvalidArgs`) rather than silently ignored.
    pub(super) fn count_lines_and_keyword_paragraphs(
        call: &CallContext,
        step_id: &str,
        _sequence: u32,
        arguments: Option<&Map<String, Value>>,
    ) -> Result<ToolEnvelope, RuntimeExecutionError> {
        let tool = crate::runtime_tools::TOOL_PURE_COUNT_LINES_AND_KEYWORD_PARAGRAPHS;
        let Some(arguments) = arguments else {
            return Ok(argument_error(
                tool,
                call,
                step_id,
                "count_lines_and_keyword_paragraphs requires arguments",
            ));
        };
        let Some(text) = arguments.get("text").and_then(Value::as_str) else {
            return Ok(argument_error(tool, call, step_id, "text must be a string"));
        };
        let literals = match parse_keyword_literals(arguments) {
            Ok(literals) => literals,
            Err(message) => return Ok(argument_error(tool, call, step_id, &message)),
        };
        if let Some(canonical_form) = arguments.get("canonical_form").and_then(Value::as_str)
            && canonical_form != "LF"
        {
            return Ok(argument_error(
                tool,
                call,
                step_id,
                "canonical_form must be `LF`",
            ));
        }
        let max_keyword_paragraphs = match parse_max_keyword_paragraphs(arguments) {
            Ok(value) => value,
            Err(message) => return Ok(argument_error(tool, call, step_id, &message)),
        };
        let line_count_total = text.lines().count();
        let matching: Vec<&str> = text
            .split("\n\n")
            .map(str::trim)
            .filter(|paragraph| !paragraph.is_empty())
            .filter(|paragraph| literals.iter().any(|keyword| paragraph.contains(keyword)))
            .collect();
        let paragraphs_truncated = matching.len() > max_keyword_paragraphs;
        let keyword_paragraphs: Vec<&str> = matching
            .iter()
            .take(max_keyword_paragraphs)
            .copied()
            .collect();
        ReservedRuntimeInvoker::ok_with_pure_fingerprint(
            tool,
            call,
            step_id,
            &json!({
                "line_count_total": line_count_total,
                "line_count_analyzed": line_count_total,
                "keyword_paragraphs": keyword_paragraphs,
                "paragraphs_truncated": paragraphs_truncated,
            }),
        )
    }

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

/// Parses the `keywords` argument into a non-empty list of literal substrings.
///
/// # Errors
///
/// Returns a message when `keywords` is missing, not an array, contains a non-string or empty
/// entry, or is empty.
fn parse_keyword_literals(arguments: &Map<String, Value>) -> Result<Vec<String>, String> {
    let Some(keywords) = arguments.get("keywords").and_then(Value::as_array) else {
        return Err("keywords must be an array of strings".to_owned());
    };
    let mut literals = Vec::with_capacity(keywords.len());
    for keyword in keywords {
        let Some(keyword) = keyword.as_str() else {
            return Err("keywords entries must be strings".to_owned());
        };
        if keyword.is_empty() {
            return Err("keywords entries must not be empty".to_owned());
        }
        literals.push(keyword.to_owned());
    }
    if literals.is_empty() {
        return Err("keywords must not be empty".to_owned());
    }
    Ok(literals)
}

/// Parses the optional positive `max_keyword_paragraphs` argument.
///
/// # Errors
///
/// Returns a message when the supplied value is not a positive integer.
fn parse_max_keyword_paragraphs(arguments: &Map<String, Value>) -> Result<usize, String> {
    arguments
        .get("max_keyword_paragraphs")
        .map_or(Ok(DEFAULT_MAX_KEYWORD_PARAGRAPHS), |value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value >= 1)
                .ok_or_else(|| "max_keyword_paragraphs must be a positive integer".to_owned())
        })
}
