//! Reserved runtime tools for the coverage-set step kinds (ADR-0060).
//!
//! Responsibilities:
//! - own the reserved tool names that represent `hitl`, `host_service`,
//!   `verify` and `pure` steps;
//! - build their Planner-catalog schemas.
//!
//! Boundaries:
//! - these tools are **never** registered in the model-visible `ToolBus`; the
//!   model must not be able to call approval or rollback directly (ADR-0060 D2);
//! - they do not execute anything here; execution belongs to the binary-layer
//!   handlers this module only names.
//!
//! Invariants:
//! 1. the reserved names are a closed set and are contract, not convention;
//! 2. a package step kind maps onto exactly one reserved tool, or none;
//! 3. the schemas are built from explicit JSON, never from inferred defaults.

use assistant_protocol::ToolSchema;
use serde_json::{Value, json};

/// Reserved tool representing a `hitl` step.
pub const TOOL_REQUEST_APPROVAL: &str = "assistant.runtime.request_approval";
/// Reserved tool representing a `host_service` rollback-anchor step.
pub const TOOL_PREPARE_ANCHORS: &str = "assistant.runtime.prepare_anchors";
/// Reserved tool representing a `verify` step.
pub const TOOL_VERIFY_POSTCONDITIONS: &str = "assistant.runtime.verify_postconditions";
/// Reserved pure operation: compute literal replacement and its count.
pub const TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT: &str =
    "assistant.runtime.pure_compute_literal_replacement";
/// Reserved pure operation: build a deterministic text diff.
pub const TOOL_PURE_BUILD_TEXT_DIFF: &str = "assistant.runtime.pure_build_text_diff";
/// Reserved pure operation: validate and normalize T1.3 inputs.
pub const TOOL_PURE_VALIDATE_T1_3_INPUTS: &str = "assistant.runtime.pure_validate_t1_3_inputs";
/// Reserved pure operation: count lines and keyword paragraphs in canonical text.
pub const TOOL_PURE_COUNT_LINES_AND_KEYWORD_PARAGRAPHS: &str =
    "assistant.runtime.pure_count_lines_and_keyword_paragraphs";
/// Reserved host operation: observe the initial target fingerprint (ADR-0065).
///
/// A task package puts this step first and **without** a `when`, so a later step whose
/// condition is false can be skipped against a fingerprint a committed step really published.
pub const TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT: &str =
    "assistant.runtime.host_capture_initial_fingerprint";
/// Reserved host operation: inspect a target path without modifying it.
pub const TOOL_HOST_INSPECT_TARGET_PATH: &str = "assistant.runtime.host_inspect_target_path";
/// Reserved host operation: set the editor value through the injected platform.
pub const TOOL_HOST_SET_EDITOR_VALUE: &str = "assistant.runtime.host_set_editor_value";
/// Reserved host operation: read a bounded UTF-8 prefix from a target file.
pub const TOOL_HOST_READ_UTF8_PREFIX: &str = "assistant.runtime.host_read_utf8_prefix";

/// The closed set of reserved runtime tool names.
pub const RESERVED_RUNTIME_TOOLS: &[&str] = &[
    TOOL_REQUEST_APPROVAL,
    TOOL_PREPARE_ANCHORS,
    TOOL_VERIFY_POSTCONDITIONS,
    TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
    TOOL_PURE_BUILD_TEXT_DIFF,
    TOOL_PURE_VALIDATE_T1_3_INPUTS,
    TOOL_PURE_COUNT_LINES_AND_KEYWORD_PARAGRAPHS,
    TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT,
    TOOL_HOST_INSPECT_TARGET_PATH,
    TOOL_HOST_SET_EDITOR_VALUE,
    TOOL_HOST_READ_UTF8_PREFIX,
];

/// Maps a declared package step kind + operation onto its reserved tool.
#[must_use]
pub fn tool_for_step(kind: &str, operation: Option<&str>) -> Option<&'static str> {
    match kind {
        "hitl" => Some(TOOL_REQUEST_APPROVAL),
        "host_service" => match operation {
            Some("prepare_rollback_anchors") => Some(TOOL_PREPARE_ANCHORS),
            Some("capture_initial_fingerprint") => Some(TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT),
            Some("inspect_target_path") => Some(TOOL_HOST_INSPECT_TARGET_PATH),
            Some("set_editor_value") => Some(TOOL_HOST_SET_EDITOR_VALUE),
            Some("read_utf8_prefix") => Some(TOOL_HOST_READ_UTF8_PREFIX),
            _ => None,
        },
        "verify" => Some(TOOL_VERIFY_POSTCONDITIONS),
        "pure" => match operation {
            Some("compute_literal_replacement") => Some(TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT),
            Some("build_text_diff") => Some(TOOL_PURE_BUILD_TEXT_DIFF),
            Some("validate_t1_3_inputs") => Some(TOOL_PURE_VALIDATE_T1_3_INPUTS),
            Some("count_lines_and_keyword_paragraphs") => {
                Some(TOOL_PURE_COUNT_LINES_AND_KEYWORD_PARAGRAPHS)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Builds the Planner-catalog schemas for the reserved runtime tools.
///
/// # Errors
///
/// Returns the serde reason when a schema cannot be assembled; a reserved tool
/// with a malformed schema must stop assembly rather than disappear.
pub fn planner_schemas() -> Result<Vec<ToolSchema>, String> {
    let declarations = RESERVED_TOOL_DECLARATIONS;
    let mut schemas = Vec::with_capacity(declarations.len());
    for (name, description, risk_level, effect, reversibility, requires_approval) in declarations {
        let value: Value = json!({
            "version": "1.0",
            "name": name,
            "description": description,
            "input": {"type": "object"},
            "output": {},
            "risk_level": risk_level,
            "effect": effect,
            "reversibility": reversibility,
            "requires_approval": requires_approval,
            "idempotent": false,
        });
        schemas.push(
            serde_json::from_value(value)
                .map_err(|error| format!("reserved runtime tool `{name}`: {error}"))?,
        );
    }
    Ok(schemas)
}

type ToolDeclaration = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
);

const RESERVED_TOOL_DECLARATIONS: [ToolDeclaration; 11] = {
    [
        (
            TOOL_REQUEST_APPROVAL,
            "Ask a human to approve the step that follows (runtime-owned; not callable by the model).",
            "medium",
            "write",
            "l2_compensation",
            true,
        ),
        // PITFALL(runtime-tools): the protocol's `ToolReversibility` has no
        // `none_readonly` variant (the adapter declaration does), so the two
        // read-only reserved tools carry `l0_undo_stack` exactly like the
        // Notepad registry normalises `none_readonly` (DRIFT-214-2).
        (
            TOOL_PREPARE_ANCHORS,
            "Prepare rollback anchors before a write step (runtime-owned; not callable by the model).",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_VERIFY_POSTCONDITIONS,
            "Assert that every prior tool step was verified and committed (runtime-owned; not callable by the model).",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_PURE_COMPUTE_LITERAL_REPLACEMENT,
            "Compute a literal replacement and count without touching the application.",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_PURE_BUILD_TEXT_DIFF,
            "Build a deterministic text diff for approval display.",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_PURE_VALIDATE_T1_3_INPUTS,
            "Validate and normalize T1.3 input values.",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_PURE_COUNT_LINES_AND_KEYWORD_PARAGRAPHS,
            "Count lines and keyword paragraphs in canonical text.",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT,
            "Observe the initial target fingerprint through the injected platform (runtime-owned; not callable by the model).",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_HOST_INSPECT_TARGET_PATH,
            "Inspect whether a target path already exists without modifying it.",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_HOST_SET_EDITOR_VALUE,
            "Set the editor value through the injected platform boundary.",
            "medium",
            "write",
            "l0_undo_stack",
            false,
        ),
        (
            TOOL_HOST_READ_UTF8_PREFIX,
            "Read a bounded UTF-8 prefix from a target file without modifying it.",
            "low",
            "read",
            "l0_undo_stack",
            false,
        ),
    ]
};

#[cfg(test)]
mod tests {
    use super::{RESERVED_RUNTIME_TOOLS, planner_schemas, tool_for_step};

    #[test]
    fn test_step_kind_mapping_is_closed_and_explicit() {
        assert_eq!(
            tool_for_step("hitl", Some("request_approval")),
            Some("assistant.runtime.request_approval")
        );
        assert_eq!(
            tool_for_step("host_service", Some("prepare_rollback_anchors")),
            Some("assistant.runtime.prepare_anchors")
        );
        assert_eq!(
            tool_for_step("verify", Some("verify_postconditions")),
            Some("assistant.runtime.verify_postconditions")
        );
        assert_eq!(
            tool_for_step("host_service", Some("read_utf8_prefix")),
            Some("assistant.runtime.host_read_utf8_prefix")
        );
        assert_eq!(
            tool_for_step("host_service", Some("capture_initial_fingerprint")),
            Some("assistant.runtime.host_capture_initial_fingerprint")
        );
        assert_eq!(tool_for_step("tool", None), None);
        assert_eq!(tool_for_step("pure", None), None);
        assert_eq!(tool_for_step("frobnicate", None), None);
    }

    #[test]
    fn test_planner_schemas_cover_every_reserved_name() {
        let Ok(schemas) = planner_schemas() else {
            return;
        };
        assert_eq!(schemas.len(), RESERVED_RUNTIME_TOOLS.len());
        for name in RESERVED_RUNTIME_TOOLS {
            assert!(
                schemas.iter().any(|schema| schema.name == *name),
                "{name} must have a Planner schema"
            );
        }
    }
}
