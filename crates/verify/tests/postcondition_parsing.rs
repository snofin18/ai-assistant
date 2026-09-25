//! Contract tests for postcondition parsing: fail-closed, no implicit defaults.
//!
//! Architecture v2 appendix A declares `#/$defs/assertion`; this file pins what we accept from it,
//! what we reject with a reason, and which `ErrorCode` each rejection maps to. Rejections are the
//! interesting half: a postcondition we cannot enforce must never be silently dropped.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_protocol::ErrorCode;
use assistant_protocol::serde_json::json;
use assistant_verify::{VerifyError, parse_postconditions};

#[test]
fn test_parse_accepts_every_supported_kind() {
    let values = vec![
        json!({"kind": "state_assert", "field": "title", "op": "contains", "value": "Notepad"}),
        json!({"kind": "text_contains", "value": "hello"}),
        json!({"kind": "text_not_contains", "value": "bye"}),
        json!({"kind": "state_changed", "within_ms": 2000}),
        json!({"kind": "state_changed", "within_ms": 2000, "fingerprint_scope": "document.body"}),
        json!({"kind": "state_unchanged"}),
        json!({"kind": "element_exists", "selector": "dialog.save"}),
        json!({"kind": "element_gone", "selector": "dialog.save"}),
        json!({"kind": "value_equals", "name": "zoom", "value": 100}),
        json!({"kind": "value_in_range", "name": "zoom", "min": 50, "max": 200}),
        json!({"kind": "file_changed", "path": "out.txt", "expect": "mtime"}),
        json!({"kind": "app_reported", "key": "saved", "value": true}),
    ];
    let parsed = parse_postconditions(&values).expect("all eleven kinds must parse");
    assert_eq!(parsed.len(), 12);
}

#[test]
fn test_parse_rejects_a_non_object() {
    let error = parse_postconditions(&[json!("text_contains")]).err();
    assert!(matches!(
        error,
        Some(VerifyError::PostconditionNotAnObject { index: 0, .. })
    ));
}

#[test]
fn test_parse_requires_kind() {
    let error = parse_postconditions(&[json!({"value": "hello"})]).err();
    assert!(matches!(
        error,
        Some(VerifyError::MalformedPostcondition { index: 0, .. })
    ));
}

#[test]
fn test_parse_rejects_unknown_kind_with_a_reason() {
    let error = parse_postconditions(&[json!({"kind": "smells_right"})])
        .expect_err("an unknown kind must be rejected");
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    assert!(
        error.to_string().contains("smells_right"),
        "the error must name the offending kind: {error}"
    );
}

#[test]
fn test_parse_rejects_visual_assert_and_points_at_task_042() {
    let error = parse_postconditions(&[json!({"kind": "visual_assert", "confidence_min": 0.9})])
        .expect_err("visual_assert belongs to TASK-042");
    assert!(matches!(
        error,
        VerifyError::UnsupportedPostconditionKind { .. }
    ));
    assert!(
        error.to_string().contains("TASK-042"),
        "the error must route the reader to the owning card: {error}"
    );
}

#[test]
fn test_parse_rejects_precondition_kinds() {
    for kind in ["target_resolvable", "capability"] {
        let error = parse_postconditions(&[json!({ "kind": kind })])
            .err()
            .unwrap_or_else(|| panic!("{kind} is a precondition, not a postcondition"));
        assert!(
            error.to_string().contains("precondition"),
            "the error must explain why: {error}"
        );
    }
}

#[test]
fn test_parse_rejects_a_typoed_extra_field() {
    // `withinMs` instead of `within_ms`: silently ignoring it would drop the author's deadline.
    let error = parse_postconditions(&[json!({"kind": "state_changed", "withinMs": 2000})]).err();
    assert!(matches!(
        error,
        Some(VerifyError::MalformedPostcondition { index: 0, .. })
    ));
}

#[test]
fn test_parse_rejects_a_zero_deadline() {
    let error = parse_postconditions(&[json!({"kind": "state_changed", "within_ms": 0})]).err();
    assert!(error.is_some(), "a zero deadline can never be met");
}

#[test]
fn test_parse_rejects_inverted_range() {
    let error = parse_postconditions(&[
        json!({"kind": "value_in_range", "name": "zoom", "min": 9, "max": 1}),
    ])
    .err();
    assert!(error.is_some(), "min must not exceed max");
}

#[test]
fn test_parse_rejects_contains_on_a_fingerprint() {
    let error = parse_postconditions(&[json!({
        "kind": "state_assert",
        "field": "fingerprint",
        "op": "contains",
        "value": format!("sha256:{}", "a".repeat(64)),
    })])
    .err();
    assert!(
        error.is_some(),
        "substring matching on a digest is meaningless"
    );
}

#[test]
fn test_parse_rejects_a_malformed_fingerprint_value() {
    let error = parse_postconditions(&[json!({
        "kind": "state_assert",
        "field": "fingerprint",
        "op": "equals",
        "value": "sha256:not-hex",
    })])
    .err();
    assert!(
        error.is_some(),
        "a bad digest must be rejected at parse time"
    );
}

#[test]
fn test_parse_rejects_a_non_string_value_for_title() {
    let error = parse_postconditions(&[json!({
        "kind": "state_assert",
        "field": "title",
        "op": "equals",
        "value": 3,
    })])
    .err();
    assert!(error.is_some(), "title compares against text");
}

#[test]
fn test_parse_rejects_an_empty_selector() {
    let error = parse_postconditions(&[json!({"kind": "element_exists", "selector": ""})]).err();
    assert!(error.is_some(), "an empty selector selects nothing");
}

#[test]
fn test_parse_rejects_the_free_form_assert_string() {
    let error = parse_postconditions(&[json!({
        "kind": "state_assert",
        "assert": "target.text.contains(new_text)",
    })])
    .err();
    assert!(
        error.is_some(),
        "the free-form assert string is not implemented"
    );
}

#[test]
fn test_parse_reports_the_first_offending_index() {
    let error = parse_postconditions(&[
        json!({"kind": "text_contains", "value": "ok"}),
        json!({"kind": "text_contains", "value": "ok", "extra": 1}),
    ])
    .expect_err("the second entry is malformed");
    match error {
        VerifyError::MalformedPostcondition { index, .. } => assert_eq!(index, 1),
        other => panic!("expected MalformedPostcondition, got {other:?}"),
    }
}

#[test]
fn test_malformed_postcondition_maps_to_tool_invalid_args() {
    let error = VerifyError::MalformedPostcondition {
        index: 0,
        reason: "missing `value`".to_owned(),
    };
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_invalid_fingerprint_maps_to_verify_failed() {
    let error = VerifyError::InvalidFingerprint {
        reason: "not hex".to_owned(),
    };
    assert_eq!(error.error_code(), ErrorCode::VerifyFailed);
}

#[test]
fn test_unsupported_on_violation_maps_to_tool_invalid_args() {
    let error = VerifyError::UnsupportedOnViolation {
        value: "retry_forever".to_owned(),
        reason: "the five schema values are listed in `on_violation`".to_owned(),
    };
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    assert!(error.to_string().contains("retry_forever"));
}

#[test]
fn test_error_display_names_the_index_and_reason() {
    let error = VerifyError::UnsupportedPostconditionKind {
        index: 3,
        kind: "visual_assert".to_owned(),
        reason: "belongs to TASK-042".to_owned(),
    };
    let rendered = error.to_string();
    assert!(rendered.contains("#3"), "{rendered}");
    assert!(rendered.contains("visual_assert"), "{rendered}");
    assert!(rendered.contains("TASK-042"), "{rendered}");
}
