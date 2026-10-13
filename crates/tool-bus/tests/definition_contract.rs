//! Public contract for `ToolDefinition` construction, registration-time
//! validation, and the builder chain (`registry.rs`).
//!
//! Why an integration test: every call below is a **public** entry point. The
//! existing suites cover the mount / call / fingerprint path with valid
//! definitions; this file pins the name rule, the fail-closed schema rejection,
//! and every builder's success / failure branch.
//!
//! Test lint exceptions are opened by the single line below (AGENTS.md §5.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_protocol::{RiskLevel, ToolEffect, ToolReversibility};
use assistant_tool_bus::{ToolBusError, ToolDefinition};
use rmcp::model::ToolAnnotations;
use serde_json::{Value, json};

fn input() -> Value {
    json!({"type": "object"})
}

fn definition() -> ToolDefinition {
    ToolDefinition::new(
        "demo.contract.run",
        "contract probe",
        RiskLevel::Low,
        ToolEffect::Read,
        ToolReversibility::L0UndoStack,
        input(),
    )
    .expect("a three-segment name with an object schema must be valid")
}

/// The name rule is the three-segment form plus the two closed meta-tool
/// literals; everything else is rejected before the schema is looked at.
#[test]
fn test_new_rejects_inadmissible_names() {
    for name in [
        "", "bare", "a.b", "a.b.c.d", "A.b.c", "a.B.c", "a.b.c-d", "1a.b.c", "a..c",
    ] {
        let error = ToolDefinition::new(
            name,
            "d",
            RiskLevel::Low,
            ToolEffect::Read,
            ToolReversibility::L0UndoStack,
            input(),
        )
        .expect_err("an inadmissible name must be rejected");
        assert!(
            matches!(error, ToolBusError::InvalidToolName { .. }),
            "{name:?}: {error:?}"
        );
    }
    for name in ["toolset.list", "toolset.search"] {
        assert!(
            ToolDefinition::new(
                name,
                "d",
                RiskLevel::Low,
                ToolEffect::Read,
                ToolReversibility::L0UndoStack,
                input()
            )
            .is_ok(),
            "{name} is the closed meta-tool exception"
        );
    }
}

/// A non-object root schema, or any keyword this crate cannot enforce, is
/// rejected at registration time.
#[test]
fn test_new_rejects_non_object_and_unenforceable_input() {
    for schema in [json!(true), json!([]), json!("x"), json!(7)] {
        let error = ToolDefinition::new(
            "demo.contract.run",
            "d",
            RiskLevel::Low,
            ToolEffect::Read,
            ToolReversibility::L0UndoStack,
            schema.clone(),
        )
        .expect_err("a non-object root schema must be rejected");
        assert!(
            matches!(error, ToolBusError::UnsupportedSchemaKeyword { .. }),
            "{schema}: {error:?}"
        );
    }
    let error = ToolDefinition::new(
        "demo.contract.run",
        "d",
        RiskLevel::Low,
        ToolEffect::Read,
        ToolReversibility::L0UndoStack,
        json!({"type": "object", "pattern": "^a$"}),
    )
    .expect_err("an unenforceable keyword must be rejected");
    assert!(matches!(
        error,
        ToolBusError::UnsupportedSchemaKeyword { .. }
    ));
}

/// `with_output_schema` only accepts an object or `null`; `with_idempotent`
/// flips the flag and the derived annotation together.
#[test]
fn test_output_schema_and_idempotent_builders() {
    let error = definition()
        .with_output_schema(json!(7))
        .expect_err("a scalar output schema must be rejected");
    assert!(matches!(
        error,
        ToolBusError::UnsupportedSchemaKeyword { .. }
    ));

    let with_object = definition()
        .with_output_schema(json!({"type": "string"}))
        .expect("an object output schema is accepted");
    assert_eq!(with_object.output_schema(), &json!({"type": "string"}));
    let with_null = definition()
        .with_output_schema(Value::Null)
        .expect("a null output schema is accepted");
    assert_eq!(with_null.output_schema(), &Value::Null);

    let idempotent = definition()
        .with_idempotent(true)
        .expect("the idempotent builder never fails");
    assert!(idempotent.is_idempotent());
    assert_eq!(idempotent.annotations().idempotent_hint, Some(true));
    let not_idempotent = idempotent
        .with_idempotent(false)
        .expect("the idempotent builder never fails");
    assert!(!not_idempotent.is_idempotent());
    assert_eq!(not_idempotent.annotations().idempotent_hint, Some(false));
}

/// `critical` / `L3Irreversible` default to `requires_approval = true`; the
/// builder can only tighten that, and `with_tags` replaces the tag list.
#[test]
fn test_requires_approval_defaults_and_tags() {
    let low = definition();
    assert_eq!(low.risk_level(), RiskLevel::Low);
    assert!(!low.schema().requires_approval);

    let critical = ToolDefinition::new(
        "demo.contract.run",
        "d",
        RiskLevel::Critical,
        ToolEffect::Write,
        ToolReversibility::L2Compensation,
        input(),
    )
    .expect("a critical definition is valid");
    assert!(
        critical.schema().requires_approval,
        "critical defaults to requires_approval"
    );
    assert!(
        critical.with_requires_approval(true).is_ok(),
        "tightening an already-true flag is allowed"
    );

    let irreversible = ToolDefinition::new(
        "demo.contract.run",
        "d",
        RiskLevel::Medium,
        ToolEffect::Write,
        ToolReversibility::L3Irreversible,
        input(),
    )
    .expect("an irreversible definition is valid");
    assert!(irreversible.schema().requires_approval);

    let tagged = low
        .with_tags(vec!["alpha".to_owned(), "beta".to_owned()])
        .with_requires_approval(true)
        .expect("a low-risk tool may opt into approval");
    assert!(tagged.schema().requires_approval);
    assert_eq!(
        tagged.schema().tags,
        vec!["alpha".to_owned(), "beta".to_owned()]
    );
}

/// Explicit MCP annotations that match the derived ones are accepted; the
/// mismatch / self-contradiction paths are covered by `toolset_governance.rs`.
#[test]
fn test_matching_annotations_are_accepted() {
    let annotations = ToolAnnotations::new()
        .read_only(true)
        .destructive(false)
        .idempotent(false)
        .open_world(false);
    let with_annotations = definition()
        .with_annotations(annotations)
        .expect("annotations that match the declaration are accepted");
    assert_eq!(with_annotations.annotations().read_only_hint, Some(true));
    assert_eq!(with_annotations.name(), "demo.contract.run");
    assert_eq!(with_annotations.effect(), ToolEffect::Read);
    assert_eq!(
        with_annotations.reversibility(),
        ToolReversibility::L0UndoStack
    );
    assert_eq!(with_annotations.input_schema(), &input());
}
