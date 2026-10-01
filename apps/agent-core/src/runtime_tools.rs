//! Reserved runtime tools for the coverage-set step kinds (ADR-0060).
//!
//! Responsibilities:
//! - own the three reserved tool names that represent `hitl`, `host_service`
//!   and `verify` steps;
//! - build their Planner-catalog schemas.
//!
//! Boundaries:
//! - these tools are **never** registered in the model-visible `ToolBus`; the
//!   model must not be able to call approval or rollback directly (ADR-0060 D2);
//! - they do not execute anything here; execution belongs to the binary-layer
//!   handlers this module only names.
//!
//! Invariants:
//! 1. the three names are a closed set and are contract, not convention;
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

/// The closed set of reserved runtime tool names.
pub const RESERVED_RUNTIME_TOOLS: &[&str] = &[
    TOOL_REQUEST_APPROVAL,
    TOOL_PREPARE_ANCHORS,
    TOOL_VERIFY_POSTCONDITIONS,
];

/// Maps a declared package step kind onto its reserved runtime tool.
///
/// Returns `None` for kinds that are either executable as ordinary tools
/// (`tool`) or owned elsewhere (`platform` / `l1_file` / `pure` / `policy`).
#[must_use]
pub fn tool_for_step_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "hitl" => Some(TOOL_REQUEST_APPROVAL),
        "host_service" => Some(TOOL_PREPARE_ANCHORS),
        "verify" => Some(TOOL_VERIFY_POSTCONDITIONS),
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
    let declarations = [
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
    ];

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

#[cfg(test)]
mod tests {
    use super::{RESERVED_RUNTIME_TOOLS, planner_schemas, tool_for_step_kind};

    #[test]
    fn test_step_kind_mapping_is_closed_and_explicit() {
        assert_eq!(
            tool_for_step_kind("hitl"),
            Some("assistant.runtime.request_approval")
        );
        assert_eq!(
            tool_for_step_kind("host_service"),
            Some("assistant.runtime.prepare_anchors")
        );
        assert_eq!(
            tool_for_step_kind("verify"),
            Some("assistant.runtime.verify_postconditions")
        );
        assert_eq!(tool_for_step_kind("tool"), None);
        assert_eq!(tool_for_step_kind("pure"), None);
        assert_eq!(tool_for_step_kind("frobnicate"), None);
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
