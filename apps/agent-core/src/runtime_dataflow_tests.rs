//! Unit tests for runtime dataflow references and closed conditions.

#![allow(clippy::expect_used)]

use serde_json::{Map, json};

use super::{
    ConditionExpr, DataflowError, collect_references, parse_condition, resolve_references,
};

#[test]
fn test_whole_value_reference_preserves_type() {
    let context = Map::from_iter([("count".to_owned(), json!(3))]);
    let resolved = resolve_references(&json!({"value": "$count"}), &context).expect("resolve");
    assert_eq!(resolved, json!({"value": 3}));
}

#[test]
fn test_partial_interpolation_is_rejected() {
    let context = Map::from_iter([("count".to_owned(), json!(3))]);
    let error = resolve_references(&json!("count=$count"), &context)
        .expect_err("partial interpolation must fail");
    assert!(matches!(error, DataflowError::PartialInterpolation { .. }));
}

#[test]
fn test_closed_condition_subset_covers_the_package_forms() {
    let first = parse_condition("file_size_bytes <= max_text_bytes").expect("comparison");
    assert!(matches!(first, ConditionExpr::Compare { .. }));

    let second = parse_condition("!target_existed_before && file_created").expect("conjunction");
    assert!(matches!(second, ConditionExpr::All(_)));
}

#[test]
fn test_condition_rejects_functions_and_logical_or() {
    assert!(parse_condition("ready() && other").is_err());
    assert!(parse_condition("ready || other").is_err());
}

#[test]
fn test_condition_evaluates_negation_and_comparison() {
    let context = Map::from_iter([
        ("target_existed_before".to_owned(), json!(false)),
        ("file_created".to_owned(), json!(true)),
        ("file_size_bytes".to_owned(), json!(10)),
        ("max_text_bytes".to_owned(), json!(20)),
    ]);
    assert!(
        parse_condition("!target_existed_before && file_created")
            .expect("parse")
            .evaluate(&context, "test")
            .expect("evaluate")
    );
    assert!(
        parse_condition("file_size_bytes <= max_text_bytes")
            .expect("parse")
            .evaluate(&context, "test")
            .expect("evaluate")
    );
}

#[test]
fn test_reference_collection_is_recursive_and_deterministic() {
    let references = collect_references(&json!({"a": ["$one", {"b": "$two"}], "c": "literal"}))
        .expect("collect");
    assert_eq!(
        references.into_iter().collect::<Vec<_>>(),
        vec!["one", "two"]
    );
}
