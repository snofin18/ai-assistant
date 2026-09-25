//! Contract tests for postcondition evaluation and the step verdict.
//!
//! The three invariants under test are the ones section 7.4 makes non-negotiable: an unobserved fact
//! is never treated as satisfied, a type mismatch is never treated as a falsification, and neither a
//! falsified nor an unevaluable postcondition can come back as a success.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_platform_api::Fingerprint;
use assistant_protocol::ErrorCode;
use assistant_protocol::serde_json::{Number, json};
use assistant_verify::{
    AssertValue, AssertionOutcome, CompareOp, FileChangeKind, FileSnapshot, FileTransition,
    Observation, ObservedElement, Postcondition, StateField, VerifyOutcome, evaluate_postcondition,
    parse_postconditions, render_text, verify_postconditions,
};

/// Builds a fingerprint from a repeated hex digit.
fn fingerprint(hex_digit: char) -> Fingerprint {
    let value = format!("sha256:{}", hex_digit.to_string().repeat(64));
    Fingerprint::parse(value).expect("a repeated hex digit is a valid digest")
}

fn base_observation() -> Observation {
    Observation {
        text: "hello world".to_owned(),
        ..Observation::new("document.body", "untitled - Notepad", fingerprint('a'))
    }
}

fn evaluate_one(postcondition: &Postcondition, observation: &Observation) -> AssertionOutcome {
    evaluate_postcondition(postcondition, observation)
}

#[test]
fn test_render_text_truncates_at_a_character_boundary() {
    let long = "\u{4f60}".repeat(200);
    let rendered = render_text(&long);
    assert!(rendered.contains("bytes"), "{rendered}");
    assert!(rendered.len() < long.len(), "must be shorter");
}

#[test]
fn test_text_contains_and_not_contains() {
    let observation = base_observation();
    assert!(
        evaluate_one(
            &Postcondition::TextContains {
                text: "hello".to_owned()
            },
            &observation
        )
        .is_satisfied()
    );
    assert!(
        evaluate_one(
            &Postcondition::TextNotContains {
                text: "hello".to_owned()
            },
            &observation
        )
        .is_falsified()
    );
}

#[test]
fn test_unprobed_element_is_not_evaluable_not_gone() {
    let observation = base_observation();
    let outcome = evaluate_one(
        &Postcondition::ElementGone {
            selector: "dialog.save".to_owned(),
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_probed_absent_element_satisfies_element_gone() {
    let mut observation = base_observation();
    observation.elements.insert(
        "dialog.save".to_owned(),
        ObservedElement::new(false, "Dialog", "", false, false),
    );
    assert!(
        evaluate_one(
            &Postcondition::ElementGone {
                selector: "dialog.save".to_owned()
            },
            &observation
        )
        .is_satisfied()
    );
    assert!(
        evaluate_one(
            &Postcondition::ElementExists {
                selector: "dialog.save".to_owned()
            },
            &observation
        )
        .is_falsified()
    );
}

#[test]
fn test_value_kind_mismatch_is_not_evaluable() {
    let mut observation = base_observation();
    observation
        .values
        .insert("count".to_owned(), AssertValue::Text("7".to_owned()));
    let outcome = evaluate_one(
        &Postcondition::ValueEquals {
            name: "count".to_owned(),
            value: AssertValue::Number(Number::from(7)),
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_value_in_range_accepts_boundaries_and_rejects_outside() {
    let mut observation = base_observation();
    observation
        .values
        .insert("zoom".to_owned(), AssertValue::Number(Number::from(100)));
    for (min, max, satisfied) in [(100.0, 200.0, true), (0.0, 100.0, true), (0.0, 99.0, false)] {
        let outcome = evaluate_one(
            &Postcondition::ValueInRange {
                name: "zoom".to_owned(),
                min,
                max,
            },
            &observation,
        );
        assert_eq!(outcome.is_satisfied(), satisfied, "{outcome:?}");
    }
}

#[test]
fn test_value_in_range_rejects_a_non_numeric_observation() {
    let mut observation = base_observation();
    observation
        .values
        .insert("zoom".to_owned(), AssertValue::Bool(true));
    let outcome = evaluate_one(
        &Postcondition::ValueInRange {
            name: "zoom".to_owned(),
            min: 0.0,
            max: 100.0,
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_file_changed_any_detects_creation() {
    let mut observation = base_observation();
    observation.files.insert(
        "out.txt".to_owned(),
        FileTransition::new(FileSnapshot::absent(), FileSnapshot::present(3, 10, None)),
    );
    assert!(
        evaluate_one(
            &Postcondition::FileChanged {
                path: "out.txt".to_owned(),
                expect: FileChangeKind::Any
            },
            &observation
        )
        .is_satisfied()
    );
}

#[test]
fn test_file_changed_size_needs_both_snapshots() {
    let mut observation = base_observation();
    observation.files.insert(
        "out.txt".to_owned(),
        FileTransition::new(FileSnapshot::absent(), FileSnapshot::present(3, 10, None)),
    );
    let outcome = evaluate_one(
        &Postcondition::FileChanged {
            path: "out.txt".to_owned(),
            expect: FileChangeKind::Size,
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_file_changed_digest_needs_both_digests() {
    let mut observation = base_observation();
    observation.files.insert(
        "out.txt".to_owned(),
        FileTransition::new(
            FileSnapshot::present(3, 10, Some("aa".to_owned())),
            FileSnapshot::present(4, 11, None),
        ),
    );
    let outcome = evaluate_one(
        &Postcondition::FileChanged {
            path: "out.txt".to_owned(),
            expect: FileChangeKind::Digest,
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_file_changed_mtime_reports_the_transition() {
    let mut observation = base_observation();
    observation.files.insert(
        "out.txt".to_owned(),
        FileTransition::new(
            FileSnapshot::present(3, 10, None),
            FileSnapshot::present(3, 20, None),
        ),
    );
    assert!(
        evaluate_one(
            &Postcondition::FileChanged {
                path: "out.txt".to_owned(),
                expect: FileChangeKind::Mtime
            },
            &observation
        )
        .is_satisfied()
    );
}

#[test]
fn test_app_reported_missing_key_is_not_evaluable() {
    let observation = base_observation();
    let outcome = evaluate_one(
        &Postcondition::AppReported {
            key: "saved".to_owned(),
            value: AssertValue::Bool(true),
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_app_reported_matching_value_is_satisfied() {
    let mut observation = base_observation();
    observation
        .app_reported
        .insert("saved".to_owned(), AssertValue::Bool(true));
    assert!(
        evaluate_one(
            &Postcondition::AppReported {
                key: "saved".to_owned(),
                value: AssertValue::Bool(true)
            },
            &observation
        )
        .is_satisfied()
    );
}

#[test]
fn test_state_changed_needs_previous_fingerprint() {
    let observation = base_observation();
    let outcome = evaluate_one(
        &Postcondition::StateChanged {
            fingerprint_scope: None,
            within_ms: 2_000,
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_state_changed_needs_elapsed_time() {
    let mut observation = base_observation();
    observation.previous_fingerprint = Some(fingerprint('b'));
    let outcome = evaluate_one(
        &Postcondition::StateChanged {
            fingerprint_scope: None,
            within_ms: 2_000,
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_state_changed_satisfied_only_when_changed_within_deadline() {
    let mut observation = base_observation();
    observation.previous_fingerprint = Some(fingerprint('b'));
    observation.elapsed_since_previous_ms = Some(500);
    assert!(
        evaluate_one(
            &Postcondition::StateChanged {
                fingerprint_scope: None,
                within_ms: 2_000
            },
            &observation
        )
        .is_satisfied()
    );

    observation.elapsed_since_previous_ms = Some(5_000);
    assert!(
        evaluate_one(
            &Postcondition::StateChanged {
                fingerprint_scope: None,
                within_ms: 2_000
            },
            &observation
        )
        .is_falsified()
    );
}

#[test]
fn test_state_changed_falsified_when_the_fingerprint_did_not_move() {
    let mut observation = base_observation();
    observation.previous_fingerprint = Some(fingerprint('a'));
    observation.elapsed_since_previous_ms = Some(10);
    assert!(
        evaluate_one(
            &Postcondition::StateChanged {
                fingerprint_scope: None,
                within_ms: 2_000
            },
            &observation
        )
        .is_falsified()
    );
}

#[test]
fn test_state_unchanged_falsified_when_the_fingerprint_moved() {
    let mut observation = base_observation();
    observation.previous_fingerprint = Some(fingerprint('b'));
    assert!(
        evaluate_one(
            &Postcondition::StateUnchanged {
                fingerprint_scope: None
            },
            &observation
        )
        .is_falsified()
    );
}

#[test]
fn test_state_changed_scope_mismatch_is_not_evaluable() {
    let mut observation = base_observation();
    observation.previous_fingerprint = Some(fingerprint('b'));
    observation.elapsed_since_previous_ms = Some(10);
    let outcome = evaluate_one(
        &Postcondition::StateChanged {
            fingerprint_scope: Some("window.whole".to_owned()),
            within_ms: 1_000,
        },
        &observation,
    );
    assert!(outcome.is_not_evaluable(), "{outcome:?}");
}

#[test]
fn test_state_assert_on_title_compares_exactly() {
    let observation = base_observation();
    let satisfied = Postcondition::StateAssert {
        field: StateField::Title,
        op: CompareOp::Equals,
        value: AssertValue::Text("untitled - Notepad".to_owned()),
    };
    assert!(evaluate_one(&satisfied, &observation).is_satisfied());

    let violated = Postcondition::StateAssert {
        field: StateField::Title,
        op: CompareOp::Contains,
        value: AssertValue::Text("*".to_owned()),
    };
    let outcome = evaluate_one(&violated, &observation);
    assert!(outcome.is_falsified(), "{outcome:?}");
    if let AssertionOutcome::Falsified { expected, actual } = outcome {
        assert!(expected.contains("contains"), "{expected}");
        assert!(actual.contains("untitled"), "{actual}");
    }
}

#[test]
fn test_state_assert_on_fingerprint_compares_the_digest() {
    let observation = base_observation();
    let parsed = parse_postconditions(&[json!({
        "kind": "state_assert",
        "field": "fingerprint",
        "op": "not_equals",
        "value": format!("sha256:{}", "b".repeat(64)),
    })])
    .expect("valid fingerprint comparison");
    let postcondition = parsed.first().expect("one postcondition");
    assert!(
        evaluate_one(postcondition, &observation).is_satisfied(),
        "the observation fingerprint is 'a' repeated, so not_equals 'b' holds"
    );
}

#[test]
fn test_empty_postcondition_list_is_inconclusive() {
    let outcome = verify_postconditions(&[], &base_observation());
    assert!(!outcome.is_verified(), "{outcome:?}");
    assert_eq!(outcome.error_code(), Some(ErrorCode::VerifyFailed));
    match outcome {
        VerifyOutcome::Inconclusive { reason, .. } => {
            assert!(reason.contains("no postconditions"), "{reason}");
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

#[test]
fn test_all_satisfied_is_verified() {
    let postconditions = vec![
        Postcondition::TextContains {
            text: "hello".to_owned(),
        },
        Postcondition::TextNotContains {
            text: "unsaved".to_owned(),
        },
    ];
    let outcome = verify_postconditions(&postconditions, &base_observation());
    assert!(outcome.is_verified(), "{outcome:?}");
    assert_eq!(outcome.error_code(), None);
    assert!(outcome.violations().is_empty());
    assert!(outcome.unevaluable().is_empty());
}

#[test]
fn test_one_falsified_postcondition_dominates() {
    let postconditions = vec![
        Postcondition::TextContains {
            text: "hello".to_owned(),
        },
        Postcondition::ElementGone {
            selector: "never.probed".to_owned(),
        },
        Postcondition::TextContains {
            text: "absent".to_owned(),
        },
    ];
    let outcome = verify_postconditions(&postconditions, &base_observation());
    assert!(outcome.is_violated(), "{outcome:?}");
    assert_eq!(outcome.violations().len(), 1);
    assert_eq!(outcome.error_code(), Some(ErrorCode::VerifyFailed));
}

#[test]
fn test_unevaluable_never_becomes_success() {
    let postconditions = vec![
        Postcondition::TextContains {
            text: "hello".to_owned(),
        },
        Postcondition::ElementGone {
            selector: "never.probed".to_owned(),
        },
    ];
    let outcome = verify_postconditions(&postconditions, &base_observation());
    assert!(!outcome.is_verified(), "{outcome:?}");
    assert!(!outcome.is_violated(), "{outcome:?}");
    assert_eq!(outcome.unevaluable().len(), 1);
    assert_eq!(outcome.error_code(), Some(ErrorCode::VerifyFailed));
}

#[test]
fn test_verifications_keep_input_order() {
    let postconditions = vec![
        Postcondition::TextContains {
            text: "hello".to_owned(),
        },
        Postcondition::ValueEquals {
            name: "missing".to_owned(),
            value: AssertValue::Bool(true),
        },
    ];
    let outcome = verify_postconditions(&postconditions, &base_observation());
    match outcome {
        VerifyOutcome::Inconclusive { verifications, .. } => {
            assert_eq!(verifications.len(), 2);
            let indices: Vec<usize> = verifications.iter().map(|entry| entry.index).collect();
            assert_eq!(indices, vec![0, 1]);
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

#[test]
fn test_verified_outcome_serializes_without_a_failure_code() {
    let postconditions = vec![Postcondition::TextContains {
        text: "hello".to_owned(),
    }];
    let outcome = verify_postconditions(&postconditions, &base_observation());
    let json = assistant_protocol::serde_json::to_string(&outcome).expect("serialize");
    assert!(json.contains("verified"), "{json}");
    assert_eq!(outcome.error_code(), None);
}
