//! Coverage contract for the public verification API.
//!
//! This file deliberately exercises public parse/evaluate entry points instead of internal
//! helpers. The tests cover fail-closed parsing, the three-valued evaluation contract, and the
//! visual module's image/hash boundaries without changing production code.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_platform_api::Fingerprint;
use assistant_protocol::serde_json::{Value, json};
use assistant_verify::{
    AssertValue, FileSnapshot, FileTransition, GrayImage, Observation, ObservedElement,
    Postcondition, VisualError, VisualField, VisualObservation, VisualOp, VisualTolerance,
    VisualVerdict, compute_dhash, compute_phash, evaluate_postcondition, evaluate_visual_assert,
    hamming_distance, parse_postconditions, parse_visual_assert,
};

fn fingerprint(digit: char) -> Fingerprint {
    Fingerprint::parse(format!("sha256:{}", digit.to_string().repeat(64)))
        .expect("test fingerprint must parse")
}

fn observation() -> Observation {
    Observation::new("document.body", "untitled", fingerprint('1'))
}

fn parse_one(value: Value) -> Postcondition {
    let mut parsed = parse_postconditions(&[value]).expect("postcondition must parse");
    assert_eq!(parsed.len(), 1);
    parsed.pop().expect("one parsed postcondition")
}

fn evaluate(value: Value, observation: &Observation) -> assistant_verify::AssertionOutcome {
    evaluate_postcondition(&parse_one(value), observation)
}

fn image(width: u32, height: u32, value: u8) -> GrayImage {
    GrayImage::new(width, height, vec![value; width as usize * height as usize])
        .expect("uniform test image must be valid")
}

fn mean_assertion() -> assistant_verify::VisualAssert {
    parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "mean_abs_diff_within",
            "max_mean_abs_diff": 1,
            "confidence_min": 0.5
        }),
    )
    .expect("mean assertion")
}

fn ratio_assertion() -> assistant_verify::VisualAssert {
    parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "changed_ratio_within",
            "pixel_delta_threshold": 1,
            "max_changed_ratio": 0.5,
            "confidence_min": 0.5
        }),
    )
    .expect("ratio assertion")
}

fn rich_observation() -> Observation {
    let mut observation = observation();
    "exact title".clone_into(&mut observation.title);
    "alpha beta".clone_into(&mut observation.text);
    observation.elements.insert(
        "present".to_owned(),
        ObservedElement::new(true, "Button", "Save", true, false),
    );
    observation.elements.insert(
        "gone".to_owned(),
        ObservedElement::new(false, "Button", "Old", false, false),
    );
    observation
        .values
        .insert("enabled".to_owned(), AssertValue::Bool(true));
    observation
        .values
        .insert("count".to_owned(), AssertValue::Number(10.into()));
    observation
        .app_reported
        .insert("ok".to_owned(), AssertValue::Bool(false));
    observation
}

fn assert_not_evaluable(value: Value, observation: &Observation) {
    let outcome = evaluate(value, observation);
    assert!(outcome.is_not_evaluable(), "outcome was {outcome:?}");
}

fn assert_satisfaction(value: Value, observation: &Observation, expected_satisfied: bool) {
    let outcome = evaluate(value, observation);
    assert_eq!(outcome.is_satisfied(), expected_satisfied, "{outcome:?}");
    assert_eq!(outcome.is_falsified(), !expected_satisfied, "{outcome:?}");
}

#[test]
fn test_postcondition_parser_rejects_malformed_inputs_fail_closed() {
    let cases = [
        json!([1]),
        json!({"kind": null}),
        json!({"kind": "state_assert", "field": "unknown", "op": "equals", "value": "x"}),
        json!({"kind": "state_assert", "field": "text", "op": "unknown", "value": "x"}),
        json!({"kind": "state_assert", "field": "text", "op": "equals", "value": 1}),
        json!({"kind": "state_assert", "field": "fingerprint", "op": "contains", "value": "x"}),
        json!({"kind": "state_assert", "field": "fingerprint", "op": "equals", "value": "bad"}),
        json!({"kind": "text_contains"}),
        json!({"kind": "text_contains", "value": 1}),
        json!({"kind": "text_not_contains", "value": []}),
        json!({"kind": "state_changed", "within_ms": 0}),
        json!({"kind": "state_changed", "within_ms": "soon"}),
        json!({"kind": "state_unchanged", "fingerprint_scope": 7}),
        json!({"kind": "element_exists", "selector": ""}),
        json!({"kind": "element_exists", "selector": 7}),
        json!({"kind": "value_equals", "name": "", "value": true}),
        json!({"kind": "value_in_range", "name": "count", "min": 2, "max": 1}),
        json!({"kind": "value_in_range", "name": "count", "min": "low", "max": 1}),
        json!({"kind": "file_changed", "path": "a.txt", "expect": "mode"}),
        json!({"kind": "app_reported", "key": "ok", "value": []}),
        json!({"kind": "text_contains", "value": "x", "extra": true}),
        json!({"kind": "target_resolvable"}),
        json!({"kind": "unknown"}),
    ];

    for value in cases {
        let error = parse_postconditions(&[value]).expect_err("case must be rejected");
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn test_state_evaluation_covers_not_evaluable_reasons() {
    let mut with_previous = observation();
    with_previous.previous_fingerprint = Some(fingerprint('0'));

    let mut scope_mismatch = with_previous.clone();
    scope_mismatch.elapsed_since_previous_ms = Some(5);

    for (value, observation) in [
        (
            json!({"kind": "state_changed", "within_ms": 10}),
            observation(),
        ),
        (
            json!({"kind": "state_changed", "within_ms": 10}),
            with_previous,
        ),
        (
            json!({"kind": "state_changed", "within_ms": 10, "fingerprint_scope": "other"}),
            scope_mismatch,
        ),
        (json!({"kind": "state_unchanged"}), observation()),
    ] {
        assert_not_evaluable(value, &observation);
    }
}

#[test]
fn test_resource_evaluation_covers_not_evaluable_reasons() {
    let mut probed_values = observation();
    probed_values
        .values
        .insert("count".to_owned(), AssertValue::Text("five".to_owned()));
    let mut probed_files = observation();
    probed_files.files.insert(
        "a.txt".to_owned(),
        FileTransition::new(FileSnapshot::absent(), FileSnapshot::present(1, 1, None)),
    );
    probed_files.files.insert(
        "b.txt".to_owned(),
        FileTransition::new(
            FileSnapshot::present(1, 1, Some("before".to_owned())),
            FileSnapshot::present(2, 2, None),
        ),
    );

    for (value, observation) in [
        (
            json!({"kind": "element_exists", "selector": "missing"}),
            observation(),
        ),
        (
            json!({"kind": "value_equals", "name": "missing", "value": 1}),
            observation(),
        ),
        (
            json!({"kind": "value_in_range", "name": "count", "min": 0, "max": 10}),
            probed_values,
        ),
        (
            json!({"kind": "value_in_range", "name": "missing", "min": 0, "max": 10}),
            observation(),
        ),
        (
            json!({"kind": "file_changed", "path": "missing", "expect": "any"}),
            observation(),
        ),
        (
            json!({"kind": "file_changed", "path": "a.txt", "expect": "size"}),
            probed_files.clone(),
        ),
        (
            json!({"kind": "file_changed", "path": "b.txt", "expect": "digest"}),
            probed_files,
        ),
        (
            json!({"kind": "app_reported", "key": "missing", "value": true}),
            observation(),
        ),
        (
            json!({
                "kind": "visual_assert",
                "field": "pixels",
                "op": "mean_abs_diff_within",
                "max_mean_abs_diff": 1,
                "confidence_min": 0.5
            }),
            observation(),
        ),
    ] {
        assert_not_evaluable(value, &observation);
    }
}

#[test]
fn test_elements_and_values_cover_satisfied_and_falsified_paths() {
    let observation = rich_observation();
    for (value, expected_satisfied) in [
        (
            json!({"kind": "state_assert", "field": "title", "op": "not_equals", "value": "other"}),
            true,
        ),
        (
            json!({"kind": "state_assert", "field": "text", "op": "not_contains", "value": "gamma"}),
            true,
        ),
        (
            json!({"kind": "state_assert", "field": "text", "op": "contains", "value": "missing"}),
            false,
        ),
        (
            json!({"kind": "element_exists", "selector": "present"}),
            true,
        ),
        (json!({"kind": "element_exists", "selector": "gone"}), false),
        (json!({"kind": "element_gone", "selector": "gone"}), true),
        (
            json!({"kind": "element_gone", "selector": "present"}),
            false,
        ),
        (
            json!({"kind": "value_equals", "name": "enabled", "value": true}),
            true,
        ),
        (
            json!({"kind": "value_in_range", "name": "count", "min": 10, "max": 10}),
            true,
        ),
        (
            json!({"kind": "value_in_range", "name": "count", "min": 0, "max": 5}),
            false,
        ),
        (
            json!({"kind": "app_reported", "key": "ok", "value": false}),
            true,
        ),
        (
            json!({"kind": "app_reported", "key": "ok", "value": true}),
            false,
        ),
    ] {
        assert_satisfaction(value, &observation, expected_satisfied);
    }

    let type_mismatch = evaluate(
        json!({"kind": "value_equals", "name": "enabled", "value": "true"}),
        &observation,
    );
    assert!(type_mismatch.is_not_evaluable(), "{type_mismatch:?}");
}

#[test]
fn test_state_changed_and_unchanged_cover_timing_paths() {
    let mut changed = rich_observation();
    changed.previous_fingerprint = Some(fingerprint('0'));
    changed.elapsed_since_previous_ms = Some(5);
    assert!(evaluate(json!({"kind": "state_changed", "within_ms": 5}), &changed).is_satisfied());
    assert!(evaluate(json!({"kind": "state_changed", "within_ms": 1}), &changed).is_falsified());

    let mut unchanged = changed;
    unchanged.previous_fingerprint = Some(unchanged.fingerprint.clone());
    assert!(evaluate(json!({"kind": "state_unchanged"}), &unchanged).is_satisfied());
    assert!(evaluate(json!({"kind": "state_changed", "within_ms": 5}), &unchanged).is_falsified());
}

#[test]
fn test_file_changed_contract_covers_creation_size_mtime_and_digest() {
    let mut observation = observation();
    observation.files.insert(
        "created.txt".to_owned(),
        FileTransition::new(FileSnapshot::absent(), FileSnapshot::present(3, 10, None)),
    );
    observation.files.insert(
        "sized.txt".to_owned(),
        FileTransition::new(
            FileSnapshot::present(1, 10, Some("a".to_owned())),
            FileSnapshot::present(2, 10, Some("a".to_owned())),
        ),
    );
    observation.files.insert(
        "mtime.txt".to_owned(),
        FileTransition::new(
            FileSnapshot::present(2, 10, Some("a".to_owned())),
            FileSnapshot::present(2, 20, Some("a".to_owned())),
        ),
    );
    observation.files.insert(
        "digest.txt".to_owned(),
        FileTransition::new(
            FileSnapshot::present(2, 10, Some("a".to_owned())),
            FileSnapshot::present(2, 10, Some("b".to_owned())),
        ),
    );

    for (path, expect) in [
        ("created.txt", "any"),
        ("sized.txt", "size"),
        ("mtime.txt", "mtime"),
        ("digest.txt", "digest"),
    ] {
        let outcome = evaluate(
            json!({"kind": "file_changed", "path": path, "expect": expect}),
            &observation,
        );
        assert!(outcome.is_satisfied(), "{path}: {outcome:?}");
    }
}

#[test]
fn test_visual_assert_parser_rejects_each_malformed_shape() {
    let cases = [
        json!([]),
        json!({"kind": 7}),
        json!({"kind": "visual_assert"}),
        json!({"kind": "visual_assert", "field": 7}),
        json!({"kind": "visual_assert", "field": "ocr"}),
        json!({"kind": "visual_assert", "field": "pixels", "op": 7}),
        json!({"kind": "visual_assert", "field": "pixels", "op": "unknown"}),
        json!({"kind": "visual_assert", "field": "pixels", "op": "mean_abs_diff_within"}),
        json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "mean_abs_diff_within",
            "max_mean_abs_diff": 256,
            "confidence_min": 0.5
        }),
        json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "changed_ratio_within",
            "pixel_delta_threshold": 256,
            "max_changed_ratio": 0.5,
            "confidence_min": 0.5
        }),
        json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "changed_ratio_within",
            "pixel_delta_threshold": 1,
            "max_changed_ratio": -0.1,
            "confidence_min": 0.5
        }),
        json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": 25,
            "confidence_min": 0.5
        }),
        json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "hamming_within",
            "max_hamming_distance": 1,
            "confidence_min": 0.5
        }),
        json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": 1,
            "confidence_min": 1.1
        }),
        json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": 1,
            "confidence_min": 0.5,
            "extra": true
        }),
    ];

    for value in cases {
        assert!(parse_visual_assert(0, &value).is_err(), "{value}");
    }
}

#[test]
fn test_visual_image_and_hash_boundaries_are_explicit() {
    let one_by_one = image(1, 1, 42);
    let changed = image(1, 1, 99);
    let phash = compute_phash(&one_by_one).expect("pHash must support a one-pixel image");
    let dhash = compute_dhash(&one_by_one).expect("dHash must support a one-pixel image");
    assert_eq!(phash.bits(), phash.bits());
    assert_eq!(phash.to_hex().len(), 16);
    assert_eq!(hamming_distance(phash, phash), 0);
    assert_eq!(
        hamming_distance(phash, compute_phash(&changed).expect("pHash")),
        hamming_distance(compute_phash(&changed).expect("pHash"), phash)
    );
    assert_eq!(dhash.to_hex().len(), 16);

    assert!(VisualObservation::new(one_by_one.clone(), one_by_one.clone(), -0.1).is_err());
    assert!(VisualObservation::new(one_by_one.clone(), one_by_one.clone(), 1.1).is_err());
    assert!(VisualObservation::new(one_by_one.clone(), one_by_one, f64::NAN).is_err());
    assert!(image(2, 2, 0).pixel_at(0, 2).is_err());
}

#[test]
fn test_visual_error_codes_are_explicit() {
    for error in [
        VisualError::EmptyDimensions {
            width: 0,
            height: 1,
        },
        VisualError::EmptyBuffer {
            width: 1,
            height: 1,
        },
        VisualError::BufferSizeMismatch {
            width: 1,
            height: 1,
            expected: 1,
            actual: 0,
        },
        VisualError::ImageTooLarge {
            width: 2,
            height: 2,
            max_pixels: 1,
        },
        VisualError::DimensionsOverflow {
            width: u32::MAX,
            height: u32::MAX,
        },
        VisualError::DimensionNotRepresentable { value: u32::MAX },
        VisualError::PixelOutOfBounds {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        },
        VisualError::ConfidenceOutOfRange { confidence: 1.5 },
        VisualError::Inconsistent {
            operation: "coverage",
            reason: "explicit".to_owned(),
        },
    ] {
        assert!(!format!("{:?}", error.error_code()).is_empty());
    }
}

#[test]
fn test_visual_labels_and_low_confidence_projection_are_explicit() {
    let mean = mean_assertion();
    let identical = VisualObservation::new(image(2, 2, 10), image(2, 2, 10), 1.0)
        .expect("identical observation");
    assert!(
        evaluate_visual_assert(&mean, &identical)
            .expect("mean evaluation")
            .is_satisfied()
    );

    let ratio = ratio_assertion();
    let ratio_observation =
        VisualObservation::new(image(2, 2, 10), image(2, 2, 20), 1.0).expect("ratio observation");
    assert!(
        evaluate_visual_assert(&ratio, &ratio_observation)
            .expect("ratio evaluation")
            .is_falsified()
    );

    assert_eq!(VisualField::Pixels.label(), "pixels");
    assert_eq!(VisualField::Phash.label(), "phash");
    assert_eq!(VisualField::Dhash.label(), "dhash");
    assert_eq!(VisualOp::MeanAbsDiffWithin.label(), "mean_abs_diff_within");
    assert_eq!(VisualOp::ChangedRatioWithin.label(), "changed_ratio_within");
    assert_eq!(VisualOp::HammingWithin.label(), "hamming_within");
    assert_eq!(mean.tolerance.op(), VisualOp::MeanAbsDiffWithin);
    assert_eq!(mean.tolerance.label(), "mean_abs_diff_within");
    assert!(matches!(
        mean.tolerance,
        VisualTolerance::MeanAbsDiffWithin { .. }
    ));
    let low_confidence = VisualObservation::new(image(2, 2, 10), image(2, 2, 10), 0.1)
        .expect("low-confidence observation");
    let verdict = evaluate_visual_assert(&mean, &low_confidence).expect("low-confidence verdict");
    assert!(matches!(verdict, VisualVerdict::NeedsHuman { .. }));
    assert!(matches!(
        verdict.to_assertion_outcome(),
        assistant_verify::AssertionOutcome::NotEvaluable { .. }
    ));
}
