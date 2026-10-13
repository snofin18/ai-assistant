//! Public contract for the binary-layer task dataflow (ADR-0061).
//!
//! Why an integration test: every call below is a **public** entry point re-exported by
//! `assistant-agent-core`. The in-crate unit tests cover the happy paths; this file pins the
//! closed-subset rejections, the full comparison-operator matrix, the fail-closed reference
//! resolution, and the duplicate-step / plan-accessor contract.
//!
//! Test lint exceptions are opened by the single line below (AGENTS.md §5.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_agent_core::{
    ConditionExpr, DataflowError, RuntimeDataflowPlan, RuntimeStepBinding, collect_references,
    parse_condition, resolve_references,
};
use assistant_protocol::serde_json::{Map, Value, json};

fn context(entries: &[(&str, Value)]) -> Map<String, Value> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn evaluate(source: &str, entries: &[(&str, Value)]) -> Result<bool, DataflowError> {
    parse_condition(source)
        .expect("condition must parse")
        .evaluate(&context(entries), source)
}

/// Plan insert rejects a duplicate id and exposes deterministic step ids.
#[test]
fn test_plan_insert_rejects_duplicates_and_lists_steps() {
    let mut plan = RuntimeDataflowPlan::new();
    let binding = RuntimeStepBinding {
        tool: "notepad.text.write".to_owned(),
        condition: None,
        outputs: vec!["written".to_owned()],
    };
    plan.insert("beta", binding.clone()).expect("first insert");
    plan.insert("alpha", binding.clone())
        .expect("second insert");
    assert_eq!(plan.step_ids(), vec!["alpha".to_owned(), "beta".to_owned()]);
    assert_eq!(plan.get("beta"), Some(&binding));
    assert!(plan.get("missing").is_none());
    assert!(matches!(
        plan.insert("beta", binding),
        Err(DataflowError::DuplicateStep { .. })
    ));
}

/// Syntax outside the closed predicate subset is rejected.
#[test]
fn test_parse_condition_rejects_syntax_outside_the_subset() {
    for source in [
        "",
        "   ",
        "ready || other",
        "(ready)",
        "ready()",
        "a &&",
        "a && && b",
        "has space",
        "a ==",
    ] {
        assert!(
            matches!(
                parse_condition(source),
                Err(DataflowError::InvalidCondition { .. })
            ),
            "{source:?} must be rejected"
        );
    }
}

/// A bare / negated boolean predicate fails closed on missing or non-boolean values.
#[test]
fn test_boolean_predicate_fails_closed() {
    assert!(matches!(
        evaluate("missing", &[]),
        Err(DataflowError::UnknownReference { .. })
    ));
    assert!(matches!(
        evaluate("flag", &[("flag", json!(1))]),
        Err(DataflowError::NonBooleanCondition { .. })
    ));
    assert!(evaluate("flag", &[("flag", json!(true))]).expect("bool"));
    assert!(!evaluate("!flag", &[("flag", json!(true))]).expect("negated"));
    assert!(evaluate("!flag", &[("flag", json!(false))]).expect("negated false"));
}

/// Every comparison operator is covered on numbers and strings, and a boolean
/// comparison resolves on the boolean branch.
#[test]
fn test_comparison_operator_matrix() {
    let numbers = [
        ("count == 3", true),
        ("count != 3", false),
        ("count < 5", true),
        ("count <= 3", true),
        ("count > 1", true),
        ("count >= 3", true),
    ];
    for (source, expected) in numbers {
        assert_eq!(
            evaluate(source, &[("count", json!(3))]).expect("compare"),
            expected,
            "{source}"
        );
    }
    assert!(evaluate("name == \"bob\"", &[("name", json!("bob"))]).expect("string equality"));
    assert!(evaluate("flag == true", &[("flag", json!(true))]).expect("bool equality"));
    assert!(evaluate("!target", &[("target", json!(false))]).expect("negation"));
}

/// A comparison across incompatible types is a fail-closed `TypeMismatch`, and
/// non-scalar operands report their JSON type.
#[test]
fn test_comparison_type_mismatch_is_reported() {
    let mismatch = evaluate("count == \"3\"", &[("count", json!(3))]).expect_err("mismatch");
    match mismatch {
        DataflowError::TypeMismatch {
            left_type,
            right_type,
            ..
        } => {
            assert_eq!(left_type, "number");
            assert_eq!(right_type, "string");
        }
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
    match evaluate("obj == 1", &[("obj", json!({}))]).expect_err("mismatch") {
        DataflowError::TypeMismatch { left_type, .. } => assert_eq!(left_type, "object"),
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
    assert!(matches!(
        evaluate("a == b", &[("a", json!(1))]),
        Err(DataflowError::UnknownReference { .. })
    ));
}

/// `references()` reports every name used by a parsed condition.
#[test]
fn test_condition_references_are_collected() {
    let condition = parse_condition("!alpha && beta == gamma").expect("parse");
    let references: Vec<String> = condition.references().into_iter().collect();
    assert_eq!(references, vec!["alpha", "beta", "gamma"]);
}

/// Reference resolution recurses through arrays / objects, passes literals through,
/// and fails closed for missing or malformed references.
#[test]
fn test_resolve_references_paths() {
    let ctx = context(&[("one", json!(1)), ("two", json!({"k": 2}))]);
    assert_eq!(
        resolve_references(&json!({"a": ["$one", {"b": "$two"}]}), &ctx).expect("resolve"),
        json!({"a": [1, {"b": {"k": 2}}]})
    );
    assert_eq!(
        resolve_references(&json!("literal"), &ctx).expect("passthrough"),
        json!("literal")
    );
    assert!(matches!(
        resolve_references(&json!("$missing"), &ctx),
        Err(DataflowError::UnknownReference { .. })
    ));
    assert!(matches!(
        resolve_references(&json!("$bad name"), &ctx),
        Err(DataflowError::PartialInterpolation { .. })
    ));
    assert!(matches!(
        resolve_references(&json!("a$b"), &ctx),
        Err(DataflowError::PartialInterpolation { .. })
    ));
}

/// Reference collection recurses and rejects malformed `$` strings.
#[test]
fn test_collect_references_paths() {
    let found: Vec<String> = collect_references(&json!({"a": ["$one", {"b": "$two"}]}))
        .expect("collect")
        .into_iter()
        .collect();
    assert_eq!(found, vec!["one", "two"]);
    assert!(matches!(
        collect_references(&json!({"x": "$bad name"})),
        Err(DataflowError::PartialInterpolation { .. })
    ));
    assert!(matches!(
        collect_references(&json!(["a$b"])),
        Err(DataflowError::PartialInterpolation { .. })
    ));
    assert!(
        collect_references(&json!({"literal": "no-ref"}))
            .expect("collect")
            .is_empty()
    );
}

/// `ConditionExpr` is fully matchable by the public API.
#[test]
fn test_condition_expr_variants_are_public() {
    let boolean = parse_condition("ready").expect("parse");
    assert!(matches!(
        boolean,
        ConditionExpr::Boolean { negated: false, .. }
    ));
    let compare = parse_condition("a < 2").expect("parse");
    assert!(matches!(compare, ConditionExpr::Compare { .. }));
    let all = parse_condition("a && b").expect("parse");
    assert!(matches!(all, ConditionExpr::All(parts) if parts.len() == 2));
}
