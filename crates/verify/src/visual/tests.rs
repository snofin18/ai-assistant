//! Unit tests for the visual verification pure logic.
//!
//! Every fixture is built from a fixed formula; no external file, screenshot, clock, or randomness
//! is read. The positive cases prove that equal images and mild perturbations stay inside the
//! tolerance, the negative cases prove that clearly different images do not, and the confidence
//! cases prove that a low-confidence match becomes `NeedsHuman` instead of a fake success.

use assistant_protocol::serde_json::{Value as JsonValue, json};

use crate::assertion::AssertionOutcome;
use crate::visual::assert::{
    VisualField, VisualTolerance, evaluate_visual_assert, parse_visual_assert,
};
use crate::visual::error::VisualError;
use crate::visual::hash::{MAX_HAMMING_DISTANCE, compute_dhash, compute_phash, hamming_distance};
use crate::visual::image::{GrayImage, MAX_IMAGE_PIXELS};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Builds a grayscale image from a fixed per-pixel formula.
fn make_image(
    width: u32,
    height: u32,
    value: impl Fn(u32, u32) -> u8,
) -> Result<GrayImage, VisualError> {
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            pixels.push(value(x, y));
        }
    }
    GrayImage::new(width, height, pixels)
}

/// A checkerboard with the given cell size and the two gray levels.
fn checkerboard(cell: u32, dark: u8, light: u8) -> Result<GrayImage, VisualError> {
    make_image(64, 64, |x, y| {
        if ((x / cell) + (y / cell)).is_multiple_of(2) {
            dark
        } else {
            light
        }
    })
}

/// A deterministic pseudo-noise overlay derived from the pixel coordinates and a fixed seed.
fn deterministic_noise(x: u32, y: u32, seed: u32) -> u8 {
    let mixed = x
        .wrapping_mul(73)
        .wrapping_add(y.wrapping_mul(151))
        .wrapping_add(seed.wrapping_mul(977))
        .rotate_left(7);
    u8::try_from(mixed % 7).unwrap_or(3)
}

/// A texture with energy spread across several frequencies, so the pHash median is meaningful.
fn textured_pattern() -> Result<GrayImage, VisualError> {
    textured_pattern_with_offset(0)
}

/// The same texture shifted by a constant brightness offset without leaving the byte range.
fn textured_pattern_with_offset(offset: u32) -> Result<GrayImage, VisualError> {
    make_image(64, 64, |x, y| {
        let checker: u32 = if ((x / 8) + (y / 8)) % 2 == 0 { 0 } else { 40 };
        let ramp = x.wrapping_mul(3).wrapping_add(y.wrapping_mul(5));
        u8::try_from(40 + offset + ((ramp + checker) % 121)).unwrap_or(0)
    })
}

/// Parses a `visual_assert` value, reporting a test failure as an error.
fn parse(value: &JsonValue) -> Result<VisualAssertForTest, Box<dyn std::error::Error>> {
    let assertion = parse_visual_assert(0, value)?;
    Ok(VisualAssertForTest { assertion })
}

/// Tiny wrapper so tests can name the parsed assertion without importing the type in every test.
struct VisualAssertForTest {
    assertion: crate::visual::assert::VisualAssert,
}

#[test]
fn test_gray_image_rejects_empty_dimensions() -> TestResult {
    match GrayImage::new(0, 4, Vec::new()) {
        Err(VisualError::EmptyDimensions { .. }) => Ok(()),
        other => Err(format!("expected EmptyDimensions, got {other:?}").into()),
    }
}

#[test]
fn test_gray_image_rejects_empty_buffer() -> TestResult {
    match GrayImage::new(2, 2, Vec::new()) {
        Err(VisualError::EmptyBuffer { .. }) => Ok(()),
        other => Err(format!("expected EmptyBuffer, got {other:?}").into()),
    }
}

#[test]
fn test_gray_image_rejects_buffer_size_mismatch() -> TestResult {
    match GrayImage::new(2, 2, vec![0, 0, 0]) {
        Err(VisualError::BufferSizeMismatch {
            expected: 4,
            actual: 3,
            ..
        }) => Ok(()),
        other => Err(format!("expected BufferSizeMismatch, got {other:?}").into()),
    }
}

#[test]
fn test_gray_image_rejects_oversized_dimensions() -> TestResult {
    let too_wide = u32::try_from(MAX_IMAGE_PIXELS + 1).map_or(u32::MAX, u32::from);
    match GrayImage::new(too_wide, 1, vec![0]) {
        Err(VisualError::ImageTooLarge { .. }) => Ok(()),
        other => Err(format!("expected ImageTooLarge, got {other:?}").into()),
    }
}

#[test]
fn test_pixel_at_reports_out_of_bounds() -> TestResult {
    let image = make_image(2, 2, |x, y| u8::try_from(x + y).unwrap_or(0))?;
    match image.pixel_at(2, 0) {
        Err(VisualError::PixelOutOfBounds { x: 2, y: 0, .. }) => Ok(()),
        other => Err(format!("expected PixelOutOfBounds, got {other:?}").into()),
    }
}

#[test]
fn test_perceptual_hashes_are_deterministic() -> TestResult {
    let image = checkerboard(4, 20, 230)?;
    let perceptual_hash_a = compute_phash(&image)?;
    let perceptual_hash_b = compute_phash(&image)?;
    let difference_hash_a = compute_dhash(&image)?;
    let difference_hash_b = compute_dhash(&image)?;
    assert_eq!(perceptual_hash_a, perceptual_hash_b);
    assert_eq!(difference_hash_a, difference_hash_b);
    Ok(())
}

#[test]
fn test_identical_images_have_zero_hamming_distance() -> TestResult {
    let image = checkerboard(4, 20, 230)?;
    assert_eq!(
        hamming_distance(compute_phash(&image)?, compute_phash(&image)?),
        0
    );
    assert_eq!(
        hamming_distance(compute_dhash(&image)?, compute_dhash(&image)?),
        0
    );
    Ok(())
}

#[test]
fn test_brightness_shift_stays_within_tolerance() -> TestResult {
    let reference = textured_pattern()?;
    // A constant offset changes the DC term only; the non-DC DCT coefficients and the adjacent
    // differences are invariant, so both hashes should stay very close.
    let shifted = textured_pattern_with_offset(40)?;

    let phash_distance = hamming_distance(compute_phash(&reference)?, compute_phash(&shifted)?);
    let dhash_distance = hamming_distance(compute_dhash(&reference)?, compute_dhash(&shifted)?);
    assert!(phash_distance <= 4, "phash distance was {phash_distance}");
    assert!(dhash_distance <= 4, "dhash distance was {dhash_distance}");
    Ok(())
}

#[test]
fn test_noise_perturbation_stays_within_phash_tolerance() -> TestResult {
    let reference = textured_pattern()?;
    let noisy = make_image(64, 64, |x, y| {
        let checker: u32 = if ((x / 8) + (y / 8)) % 2 == 0 { 0 } else { 40 };
        let ramp = x.wrapping_mul(3).wrapping_add(y.wrapping_mul(5));
        let base = u8::try_from(40 + ((ramp + checker) % 121)).unwrap_or(0);
        let offset = deterministic_noise(x, y, 11);
        base.saturating_sub(offset)
    })?;
    let distance = hamming_distance(compute_phash(&reference)?, compute_phash(&noisy)?);
    assert!(distance <= 10, "noisy phash distance was {distance}");
    Ok(())
}

#[test]
fn test_clearly_different_images_exceed_default_threshold() -> TestResult {
    // A textured reference has energy across several frequencies, so its pHash bits are stable;
    // a low-detail reference (solid color, very fine pattern) can collapse in the downsampling
    // step and must be paired with a pixel tolerance instead (see the known limitation).
    let reference = textured_pattern()?;
    let different = checkerboard(8, 20, 230)?;
    let phash_distance = hamming_distance(compute_phash(&reference)?, compute_phash(&different)?);
    let dhash_distance = hamming_distance(compute_dhash(&reference)?, compute_dhash(&different)?);
    assert!(phash_distance > 10, "phash distance was {phash_distance}");
    assert!(dhash_distance > 10, "dhash distance was {dhash_distance}");
    Ok(())
}

#[test]
fn test_low_detail_observation_is_caught_by_pixel_tolerance() -> TestResult {
    // pHash / dHash are weak when one side collapses to a near-uniform image (a solid canvas is
    // exactly the "did nothing get drawn" case). The pixel tolerance is the reliable guard there,
    // so the visual contract must let a task choose it instead of pretending the hash is enough.
    let reference = textured_pattern()?;
    let nothing_drawn = make_image(64, 64, |_, _| 255)?;
    let observation = VisualObservationForTest::new(reference, nothing_drawn, 1.0)?;
    let assertion = parse(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 10,
        "confidence_min": 0.5,
    }))?;
    let verdict = evaluate_visual_assert(&assertion.assertion, &observation.observation)?;
    assert!(verdict.is_falsified(), "verdict was {verdict:?}");
    Ok(())
}

#[test]
fn test_low_confidence_becomes_needs_human_not_success() -> TestResult {
    let reference = checkerboard(4, 20, 230)?;
    let observation = VisualObservationForTest::new(reference.clone(), reference, 0.3)?;
    let assertion = parse(&json!({
        "kind": "visual_assert",
        "field": "phash",
        "op": "hamming_within",
        "max_hamming_distance": 10,
        "confidence_min": 0.9,
    }))?;
    let verdict = evaluate_visual_assert(&assertion.assertion, &observation.observation)?;
    assert!(verdict.requires_human(), "verdict was {verdict:?}");
    assert!(!verdict.is_satisfied());
    match verdict.to_assertion_outcome() {
        AssertionOutcome::NotEvaluable { reason } => {
            assert!(reason.contains("confidence"), "reason was {reason}");
        }
        other => return Err(format!("expected NotEvaluable, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn test_dimension_mismatch_becomes_needs_human() -> TestResult {
    let reference = make_image(4, 4, |_, _| 100)?;
    let actual = make_image(2, 2, |_, _| 100)?;
    let observation = VisualObservationForTest::new(reference, actual, 1.0)?;
    let assertion = parse(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 5,
        "confidence_min": 0.5,
    }))?;
    let verdict = evaluate_visual_assert(&assertion.assertion, &observation.observation)?;
    assert!(verdict.requires_human(), "verdict was {verdict:?}");
    Ok(())
}

/// Wrapper around a real observation; keeps the test helper readable.
struct VisualObservationForTest {
    observation: crate::visual::assert::VisualObservation,
}

impl VisualObservationForTest {
    fn new(expected: GrayImage, actual: GrayImage, confidence: f64) -> Result<Self, VisualError> {
        Ok(Self {
            observation: crate::visual::assert::VisualObservation::new(
                expected, actual, confidence,
            )?,
        })
    }
}

#[test]
fn test_pixel_tolerance_satisfied_for_small_difference() -> TestResult {
    let reference = make_image(8, 8, |_, _| 100)?;
    let actual = make_image(8, 8, |x, _| if x == 0 { 104 } else { 100 })?;
    let observation = VisualObservationForTest::new(reference, actual, 1.0)?;
    let assertion = parse(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 1,
        "confidence_min": 0.5,
    }))?;
    let verdict = evaluate_visual_assert(&assertion.assertion, &observation.observation)?;
    assert!(verdict.is_satisfied(), "verdict was {verdict:?}");
    Ok(())
}

#[test]
fn test_pixel_tolerance_falsified_for_large_difference() -> TestResult {
    let reference = make_image(8, 8, |_, _| 0)?;
    let actual = make_image(8, 8, |_, _| 200)?;
    let observation = VisualObservationForTest::new(reference, actual, 1.0)?;
    let assertion = parse(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "changed_ratio_within",
        "pixel_delta_threshold": 10,
        "max_changed_ratio": 0.05,
        "confidence_min": 0.5,
    }))?;
    let verdict = evaluate_visual_assert(&assertion.assertion, &observation.observation)?;
    assert!(verdict.is_falsified(), "verdict was {verdict:?}");
    Ok(())
}

#[test]
fn test_parse_visual_assert_accepts_each_valid_shape() -> TestResult {
    let pixel = parse(&json!({
        "kind": "visual_assert",
        "field": "pixels",
        "op": "mean_abs_diff_within",
        "max_mean_abs_diff": 12,
        "confidence_min": 0.9,
    }))?;
    assert_eq!(pixel.assertion.field, VisualField::Pixels);
    assert!(matches!(
        pixel.assertion.tolerance,
        VisualTolerance::MeanAbsDiffWithin {
            max_mean_abs_diff: 12
        }
    ));

    let hash = parse(&json!({
        "kind": "visual_assert",
        "field": "dhash",
        "op": "hamming_within",
        "max_hamming_distance": 24,
        "confidence_min": 1.0,
    }))?;
    assert_eq!(hash.assertion.field, VisualField::Dhash);
    Ok(())
}

#[test]
fn test_parse_visual_assert_rejects_unknown_field_and_extra_keys() -> TestResult {
    for value in [
        json!({
            "kind": "visual_assert",
            "field": "ocr",
            "op": "hamming_within",
            "max_hamming_distance": 10,
            "confidence_min": 0.9,
        }),
        json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": 10,
            "confidence_min": 0.9,
            "assert": "similar_to(reference, 0.9)",
        }),
    ] {
        let error = parse_visual_assert(0, &value)
            .err()
            .ok_or("expected the visual assertion to be rejected")?;
        assert!(!error.to_string().is_empty());
    }
    Ok(())
}

#[test]
fn test_parse_visual_assert_rejects_zero_confidence_min() -> TestResult {
    let error = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": 10,
            "confidence_min": 0.0,
        }),
    )
    .err()
    .ok_or("confidence_min = 0 must be rejected")?;
    assert!(error.to_string().contains("confidence_min"));
    Ok(())
}

#[test]
fn test_parse_visual_assert_rejects_hamming_above_ceiling() -> TestResult {
    let too_large = u64::from(MAX_HAMMING_DISTANCE) + 1;
    let error = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "max_hamming_distance": too_large,
            "confidence_min": 0.9,
        }),
    )
    .err()
    .ok_or("a hamming distance above the ceiling must be rejected")?;
    assert!(error.to_string().contains("false-match"));
    Ok(())
}

#[test]
fn test_parse_visual_assert_rejects_field_op_mismatch() -> TestResult {
    let error = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "hamming_within",
            "max_hamming_distance": 10,
            "confidence_min": 0.9,
        }),
    )
    .err()
    .ok_or("pixels + hamming_within must be rejected")?;
    assert!(error.to_string().contains("cannot be combined"));
    Ok(())
}

#[test]
fn test_parse_visual_assert_rejects_missing_tolerance_parameter() -> TestResult {
    let error = parse_visual_assert(
        0,
        &json!({
            "kind": "visual_assert",
            "field": "phash",
            "op": "hamming_within",
            "confidence_min": 0.9,
        }),
    )
    .err()
    .ok_or("a missing max_hamming_distance must be rejected")?;
    assert!(error.to_string().contains("max_hamming_distance"));
    Ok(())
}
