//! Contract tests for wiring `visual_assert` into the postcondition engine (ADR-0077 / PL-110).
//!
//! The point of these tests is the fail-closed contract: a `visual_assert` that has **no** image
//! available must never be reported as satisfied, and a low-confidence result must stay
//! `NotEvaluable` — neither may fake a success or mint a receipt.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_platform_api::Fingerprint;
use assistant_protocol::serde_json::{Value, json};
use assistant_verify::{
    AssertionOutcome, GrayImage, Observation, Postcondition, VerifyOutcome, VisualObservation,
    evaluate_postcondition, evaluate_postcondition_with_visual, parse_postconditions,
    verify_postconditions, verify_postconditions_with_visual,
};

/// Builds a fingerprint from a repeated hex digit.
fn fingerprint() -> Fingerprint {
    Fingerprint::parse(format!("sha256:{}", "a".repeat(64))).expect("a repeated hex digit is valid")
}

fn observation() -> Observation {
    Observation::new("document.body", "untitled - Paint", fingerprint())
}

/// A fine-grained checkerboard: identical copies must hash identically, and the pattern has enough
/// detail that the perceptual hashes are actually computed rather than collapsing to a constant.
fn checkerboard() -> GrayImage {
    let mut pixels = Vec::with_capacity(64);
    for y in 0..8u32 {
        for x in 0..8u32 {
            pixels.push(if (x + y) % 2 == 0 { 0 } else { 255 });
        }
    }
    GrayImage::new(8, 8, pixels).expect("8x8 is a valid gray image")
}

fn visual(confidence: f64) -> VisualObservation {
    VisualObservation::new(checkerboard(), checkerboard(), confidence)
        .expect("identical reference and observed images are a valid visual observation")
}

fn visual_assertion_json() -> Value {
    json!({
        "kind": "visual_assert",
        "field": "phash",
        "op": "hamming_within",
        "max_hamming_distance": 0,
        "confidence_min": 0.9,
    })
}

/// The single parsed postcondition under test (avoid `parsed[0]`, which the workspace bans).
const fn only(postconditions: &[Postcondition]) -> &Postcondition {
    postconditions
        .first()
        .expect("the fixture declares exactly one postcondition")
}

#[test]
fn test_visual_assert_parses_into_a_typed_postcondition() {
    let parsed = parse_postconditions(&[visual_assertion_json()]).expect("the shape is valid");
    assert_eq!(parsed.len(), 1);
    assert!(
        matches!(parsed.first(), Some(Postcondition::VisualAssert { .. })),
        "visual_assert must become a typed postcondition, not a skipped entry"
    );
}

#[test]
fn test_visual_assert_rejects_unknown_fields() {
    // 直接在 JSON 里多给一个键：`Value` 的 IndexMut 会触发 workspace 的 `indexing_slicing` 禁用项。
    let bad = json!({
        "kind": "visual_assert",
        "field": "phash",
        "op": "hamming_within",
        "max_hamming_distance": 0,
        "confidence_min": 0.9,
        "unexpected": 1,
    });
    let error = parse_postconditions(&[bad]).expect_err("unknown fields must fail the parse");
    assert_eq!(
        error.error_code(),
        assistant_protocol::ErrorCode::ToolInvalidArgs
    );
}

#[test]
fn test_visual_assert_without_images_is_never_satisfied() {
    let parsed = parse_postconditions(&[visual_assertion_json()]).expect("the shape is valid");
    let outcome = evaluate_postcondition(only(&parsed), &observation());
    assert!(
        matches!(outcome, AssertionOutcome::NotEvaluable { .. }),
        "the legacy entry point has no images, so it must not claim success"
    );
    assert!(matches!(
        verify_postconditions(&parsed, &observation()),
        VerifyOutcome::Inconclusive { .. }
    ));
}

#[test]
fn test_visual_assert_is_satisfied_with_identical_images() {
    let parsed = parse_postconditions(&[visual_assertion_json()]).expect("the shape is valid");
    let visual = visual(0.95);
    let outcome = evaluate_postcondition_with_visual(only(&parsed), &observation(), Some(&visual));
    assert!(matches!(outcome, AssertionOutcome::Satisfied));
    assert!(matches!(
        verify_postconditions_with_visual(&parsed, &observation(), Some(&visual)),
        VerifyOutcome::Verified { .. }
    ));
}

#[test]
fn test_low_confidence_stays_unevaluable() {
    let parsed = parse_postconditions(&[visual_assertion_json()]).expect("the shape is valid");
    // `confidence_min` is 0.9, so 0.3 must be NeedsHuman -> NotEvaluable, never Satisfied.
    let visual = visual(0.3);
    assert!(matches!(
        evaluate_postcondition_with_visual(only(&parsed), &observation(), Some(&visual)),
        AssertionOutcome::NotEvaluable { .. }
    ));
    assert!(matches!(
        verify_postconditions_with_visual(&parsed, &observation(), Some(&visual)),
        VerifyOutcome::Inconclusive { .. }
    ));
}
