//! Public contract coverage for the visual verification module.
//!
//! These tests stay outside the crate and call only exported APIs. They pin the fail-closed
//! parser branches, the three-valued verdict projection, and the image-indexing errors that are
//! reachable without touching private invariants or production source.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use assistant_protocol::serde_json::{Value, json};
use assistant_verify::{
    AssertionOutcome, GrayImage, VisualError, VisualObservation, VisualOp, VisualTolerance,
    evaluate_visual_assert, parse_visual_assert,
};

/// Builds a uniform image for the visual contract fixtures.
fn uniform_image(width: u32, height: u32, value: u8) -> GrayImage {
    let pixel_count = width as usize * height as usize;
    GrayImage::new(width, height, vec![value; pixel_count])
        .expect("uniform image dimensions and buffer must be valid")
}

/// Parses a visual assertion and returns the failure message on rejection.
fn rejection_message(value: &Value) -> String {
    match parse_visual_assert(0, value) {
        Ok(assertion) => panic!("the malformed visual assertion parsed as {assertion:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn test_visual_tolerance_op_tracks_each_parsed_operator() {
    let mean = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "mean_abs_diff_within",
            "max_mean_abs_diff": 12,
            "confidence_min": 0.5,
        }),
    )
    .expect("mean absolute difference shape must parse");
    assert_eq!(mean.tolerance.op(), VisualOp::MeanAbsDiffWithin);
    assert_eq!(mean.tolerance.label(), "mean_abs_diff_within");

    let changed_ratio = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "changed_ratio_within",
            "pixel_delta_threshold": 4,
            "max_changed_ratio": 0.25,
            "confidence_min": 0.5,
        }),
    )
    .expect("changed-ratio shape must parse");
    assert_eq!(changed_ratio.tolerance.op(), VisualOp::ChangedRatioWithin);

    let hamming = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "dhash",
            "op": "hamming_within",
            "max_hamming_distance": 8,
            "confidence_min": 0.5,
        }),
    )
    .expect("hamming shape must parse");
    assert_eq!(hamming.tolerance.op(), VisualOp::HammingWithin);
    assert!(matches!(
        hamming.tolerance,
        VisualTolerance::HammingWithin {
            max_hamming_distance: 8
        }
    ));
}

#[test]
fn test_parse_visual_assert_rejects_wrong_kind_string() {
    let message = rejection_message(&json!({
        "kind": "state_assert",
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 1,
        "confidence_min": 0.5,
    }));
    assert!(message.contains("visual_assert"), "message was {message}");
    assert!(message.contains("state_assert"), "message was {message}");
}

#[test]
fn test_parse_visual_assert_rejects_missing_kind() {
    let message = rejection_message(&json!({
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 1,
        "confidence_min": 0.5,
    }));
    assert!(message.contains("kind"), "message was {message}");
    assert!(message.contains("required"), "message was {message}");
}

#[test]
fn test_parse_visual_assert_rejects_mean_abs_diff_above_byte_range() {
    let message = rejection_message(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 256,
        "confidence_min": 0.5,
    }));
    assert!(
        message.contains("max_mean_abs_diff"),
        "message was {message}"
    );
    assert!(message.contains("0..=255"), "message was {message}");
}

#[test]
fn test_parse_visual_assert_rejects_changed_ratio_threshold_above_byte_range() {
    let message = rejection_message(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "changed_ratio_within",
        "pixel_delta_threshold": 256,
        "max_changed_ratio": 0.25,
        "confidence_min": 0.5,
    }));
    assert!(
        message.contains("pixel_delta_threshold"),
        "message was {message}"
    );
    assert!(message.contains("0..=255"), "message was {message}");
}

#[test]
fn test_evaluate_changed_ratio_reports_satisfied_metric() {
    let expected = uniform_image(8, 8, 100);
    let mut observed_pixels = vec![100_u8; 64];
    observed_pixels[0] = 120;
    let observed =
        GrayImage::new(8, 8, observed_pixels).expect("the observed buffer must be valid");
    let observation = VisualObservation::new(expected, observed, 1.0)
        .expect("the visual observation must be valid");
    let assertion = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "changed_ratio_within",
            "pixel_delta_threshold": 10,
            "max_changed_ratio": 0.02,
            "confidence_min": 0.5,
        }),
    )
    .expect("changed-ratio shape must parse");

    let verdict = evaluate_visual_assert(&assertion, &observation)
        .expect("well-formed images must evaluate without an internal error");
    assert!(verdict.is_satisfied(), "verdict was {verdict:?}");
    match verdict {
        assistant_verify::VisualVerdict::Satisfied { metric, confidence } => {
            assert!((confidence - 1.0).abs() < f64::EPSILON);
            assert!(matches!(
                metric,
                assistant_verify::VisualMetric::ChangedPixelRatio {
                    changed_pixels: 1,
                    total_pixels: 64,
                    ..
                }
            ));
        }
        other => panic!("expected Satisfied, got {other:?}"),
    }
}

#[test]
fn test_evaluate_hamming_reports_falsified_metric() {
    let expected = uniform_image(32, 32, 0);
    let mut checkerboard_pixels = Vec::with_capacity(32 * 32);
    for y in 0..32_u32 {
        for x in 0..32_u32 {
            checkerboard_pixels.push(if (x + y).is_multiple_of(2) { 0 } else { 255 });
        }
    }
    let observed =
        GrayImage::new(32, 32, checkerboard_pixels).expect("the checkerboard buffer must be valid");
    let observation = VisualObservation::new(expected, observed, 1.0)
        .expect("the visual observation must be valid");
    let assertion = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": 0,
            "confidence_min": 0.5,
        }),
    )
    .expect("phash hamming shape must parse");

    let verdict = evaluate_visual_assert(&assertion, &observation)
        .expect("well-formed images must evaluate without an internal error");
    assert!(verdict.is_falsified(), "verdict was {verdict:?}");
    match &verdict {
        assistant_verify::VisualVerdict::Falsified {
            metric: assistant_verify::VisualMetric::HammingDistance { distance, .. },
            expected,
            actual,
            ..
        } => {
            assert!(*distance > 0, "the images must not hash identically");
            assert!(
                expected.contains("hamming_distance"),
                "expected was {expected}"
            );
            assert!(actual.contains("hamming_distance"), "actual was {actual}");
        }
        other => panic!("expected a hamming falsification, got {other:?}"),
    }
    assert!(matches!(
        verdict.to_assertion_outcome(),
        AssertionOutcome::Falsified { .. }
    ));
}

#[test]
fn test_pixel_at_reports_row_multiplication_overflow() {
    let image = uniform_image(2, 1, 7);
    let error = image
        .pixel_at(0, usize::MAX)
        .expect_err("an out-of-range row must fail closed");
    assert!(matches!(
        error,
        VisualError::Inconsistent {
            operation: "read image row",
            ..
        }
    ));
}

#[test]
fn test_pixel_at_reports_row_end_addition_overflow() {
    let image = uniform_image(16, 1, 7);
    let row_index = usize::MAX / 16;
    let error = image
        .pixel_at(0, row_index)
        .expect_err("an overflowing row end must fail closed");
    assert!(matches!(
        error,
        VisualError::Inconsistent {
            operation: "read image row",
            ..
        }
    ));
}
