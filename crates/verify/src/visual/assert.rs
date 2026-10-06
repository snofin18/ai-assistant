//! The structured `visual_assert` contract and its three-valued evaluation.
//!
//! Responsibility: turn the frozen ADR-0074 JSON shape into [`VisualAssert`], compare a caller
//! supplied [`VisualObservation`] (reference image, observed image, and a confidence) using one of
//! the pixel or perceptual-hash tolerances, and report a [`VisualVerdict`].
//!
//! Boundary: no image capture, no codec, no OCR, no region selection, no policy. The caller owns
//! how the two images were obtained; this module only decides, and it never repairs a malformed
//! assertion or a malformed image.
//!
//! Why the shape is flat and closed: ADR-0047 permanently rejects a free-form `assert` string
//! because it would be an expression language the model cannot enumerate. `visual_assert`
//! therefore uses `field` + `op` + named tolerance parameters + `confidence_min`, with every
//! unknown field and every field/operator mismatch rejected at parse time.
//!
//! The low-confidence rule (ADR-0074 D9): when the observation's confidence is below
//! `confidence_min`, the verdict is [`VisualVerdict::NeedsHuman`], **never**
//! [`VisualVerdict::Satisfied`]. [`VisualVerdict::to_assertion_outcome`] maps that state to
//! `AssertionOutcome::NotEvaluable`, which the existing reduction turns into
//! `VerifyOutcome::Inconclusive` (`ErrorCode::VerifyFailed`) and which can never mint a
//! `VerificationReceipt`.

use assistant_protocol::serde_json::{Map, Value as JsonValue};
use serde::{Deserialize, Serialize};

use crate::assertion::AssertionOutcome;
use crate::error::{VerifyResult, malformed};
use crate::json_field::{
    json_type_name, not_an_object, only_keys, required_f64, required_text, required_u64,
};
use crate::visual::error::VisualError;
use crate::visual::hash::{
    MAX_HAMMING_DISTANCE, PerceptualHash, compute_dhash, compute_phash, hamming_distance,
};
use crate::visual::image::GrayImage;

/// The only accepted `kind` value for this contract.
pub const VISUAL_ASSERT_KIND: &str = "visual_assert";

/// Which visual signal the assertion inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VisualField {
    /// Per-pixel comparison of the reference and observed grayscale buffers.
    Pixels,
    /// The 64-bit pHash from ADR-0074 D5.
    Phash,
    /// The 64-bit dHash from ADR-0074 D6.
    Dhash,
}

impl VisualField {
    /// The JSON / report label of this field.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pixels => "pixels",
            Self::Phash => "phash",
            Self::Dhash => "dhash",
        }
    }
}

/// Which comparison operator the assertion applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VisualOp {
    /// Mean absolute pixel difference is at most `max_mean_abs_diff`.
    MeanAbsDiffWithin,
    /// Fraction of changed pixels is at most `max_changed_ratio`.
    ChangedRatioWithin,
    /// Hamming distance between two perceptual hashes is at most `max_hamming_distance`.
    HammingWithin,
}

impl VisualOp {
    /// The JSON / report label of this operator.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MeanAbsDiffWithin => "mean_abs_diff_within",
            Self::ChangedRatioWithin => "changed_ratio_within",
            Self::HammingWithin => "hamming_within",
        }
    }
}

/// The validated tolerance of one visual assertion.
///
/// The enum makes illegal parameter combinations unrepresentable inside Rust: a hamming tolerance
/// cannot accidentally carry a pixel-difference bound, and a pixel tolerance cannot carry a hash
/// distance.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum VisualTolerance {
    /// `pixels` + `mean_abs_diff_within`.
    MeanAbsDiffWithin {
        /// Inclusive upper bound on the mean absolute pixel difference (`0..=255`).
        max_mean_abs_diff: u8,
    },
    /// `pixels` + `changed_ratio_within`.
    ChangedRatioWithin {
        /// Pixels whose absolute difference exceeds this threshold count as changed (`0..=255`).
        pixel_delta_threshold: u8,
        /// Inclusive upper bound on `changed / total` (`0.0..=1.0`).
        max_changed_ratio: f64,
    },
    /// `phash` / `dhash` + `hamming_within`.
    HammingWithin {
        /// Inclusive upper bound on the 64-bit Hamming distance (`0..=24`).
        max_hamming_distance: u32,
    },
}

impl VisualTolerance {
    /// The operator this tolerance implements.
    #[must_use]
    pub const fn op(self) -> VisualOp {
        match self {
            Self::MeanAbsDiffWithin { .. } => VisualOp::MeanAbsDiffWithin,
            Self::ChangedRatioWithin { .. } => VisualOp::ChangedRatioWithin,
            Self::HammingWithin { .. } => VisualOp::HammingWithin,
        }
    }

    /// The JSON / report label of this tolerance.
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.op().label()
    }
}

/// A parsed and validated `visual_assert` postcondition.
///
/// `#[non_exhaustive]` keeps external crates from constructing an unvalidated instance; they must
/// go through [`parse_visual_assert`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct VisualAssert {
    /// Which visual signal to compare.
    pub field: VisualField,
    /// The validated tolerance for that signal.
    pub tolerance: VisualTolerance,
    /// Minimum observation confidence for a match to count (`(0.0, 1.0]`).
    pub confidence_min: f64,
}

/// Parses the frozen ADR-0074 shape into a [`VisualAssert`], fail-closed.
///
/// The accepted shape is flat:
/// `{"kind":"visual_assert","field":..., "op":..., <named tolerance parameters>,
/// "confidence_min":...}`.
///
/// # Errors
///
/// Returns [`crate::error::VerifyError::PostconditionNotAnObject`] when `value` is not an object,
/// and [`crate::error::VerifyError::MalformedPostcondition`] for a missing / mistyped field, an
/// unknown extra field, an out-of-range tolerance, `confidence_min` outside `(0, 1]`, or an
/// illegal `field` / `op` combination. Both map to `ErrorCode::ToolInvalidArgs`.
pub fn parse_visual_assert(index: usize, value: &JsonValue) -> VerifyResult<VisualAssert> {
    let JsonValue::Object(object) = value else {
        return Err(not_an_object(index, value));
    };
    match object.get("kind") {
        Some(JsonValue::String(kind)) if kind == VISUAL_ASSERT_KIND => {}
        Some(JsonValue::String(kind)) => {
            return Err(malformed(
                index,
                format!("`kind` must be `{VISUAL_ASSERT_KIND}`, got `{kind}`"),
            ));
        }
        Some(other) => {
            return Err(malformed(
                index,
                format!("`kind` must be a string, got {}", json_type_name(other)),
            ));
        }
        None => return Err(malformed(index, "`kind` is required")),
    }

    let field = parse_field(index, object)?;
    let op = parse_op(index, object)?;
    let confidence_min = parse_confidence_min(index, object)?;
    let tolerance = match (field, op) {
        (VisualField::Pixels, VisualOp::MeanAbsDiffWithin) => {
            parse_mean_abs_diff_tolerance(index, object)?
        }
        (VisualField::Pixels, VisualOp::ChangedRatioWithin) => {
            parse_changed_ratio_tolerance(index, object)?
        }
        (VisualField::Phash | VisualField::Dhash, VisualOp::HammingWithin) => {
            parse_hamming_tolerance(index, object)?
        }
        _ => {
            return Err(malformed(
                index,
                format!(
                    "`field` {} cannot be combined with `op` {}; valid pairs are \
                     pixels/mean_abs_diff_within, pixels/changed_ratio_within, \
                     phash|dhash/hamming_within",
                    field.label(),
                    op.label()
                ),
            ));
        }
    };

    Ok(VisualAssert {
        field,
        tolerance,
        confidence_min,
    })
}

/// Parses the `field` key.
fn parse_field(index: usize, object: &Map<String, JsonValue>) -> VerifyResult<VisualField> {
    match required_text(index, object, "field")?.as_str() {
        "pixels" => Ok(VisualField::Pixels),
        "phash" => Ok(VisualField::Phash),
        "dhash" => Ok(VisualField::Dhash),
        other => Err(malformed(
            index,
            format!("`field` must be one of pixels/phash/dhash, got `{other}`"),
        )),
    }
}

/// Parses the `op` key.
fn parse_op(index: usize, object: &Map<String, JsonValue>) -> VerifyResult<VisualOp> {
    match required_text(index, object, "op")?.as_str() {
        "mean_abs_diff_within" => Ok(VisualOp::MeanAbsDiffWithin),
        "changed_ratio_within" => Ok(VisualOp::ChangedRatioWithin),
        "hamming_within" => Ok(VisualOp::HammingWithin),
        other => Err(malformed(
            index,
            format!(
                "`op` must be one of mean_abs_diff_within/changed_ratio_within/hamming_within, \
                 got `{other}`"
            ),
        )),
    }
}

/// Parses the mandatory `confidence_min`, rejecting the "accept anything" value `0.0`.
fn parse_confidence_min(index: usize, object: &Map<String, JsonValue>) -> VerifyResult<f64> {
    let value = required_f64(index, object, "confidence_min")?;
    if value > 0.0 && value <= 1.0 {
        Ok(value)
    } else {
        Err(malformed(
            index,
            format!("`confidence_min` must be in (0, 1], got {value}"),
        ))
    }
}

/// Parses the pixel mean-absolute-difference tolerance.
fn parse_mean_abs_diff_tolerance(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<VisualTolerance> {
    only_keys(
        index,
        object,
        &["kind", "field", "op", "max_mean_abs_diff", "confidence_min"],
    )?;
    let raw = required_u64(index, object, "max_mean_abs_diff")?;
    let max_mean_abs_diff = u8::try_from(raw).map_err(|_| {
        malformed(
            index,
            format!("`max_mean_abs_diff` must be in 0..=255, got {raw}"),
        )
    })?;
    Ok(VisualTolerance::MeanAbsDiffWithin { max_mean_abs_diff })
}

/// Parses the changed-pixel-ratio tolerance.
fn parse_changed_ratio_tolerance(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<VisualTolerance> {
    only_keys(
        index,
        object,
        &[
            "kind",
            "field",
            "op",
            "pixel_delta_threshold",
            "max_changed_ratio",
            "confidence_min",
        ],
    )?;
    let raw = required_u64(index, object, "pixel_delta_threshold")?;
    let pixel_delta_threshold = u8::try_from(raw).map_err(|_| {
        malformed(
            index,
            format!("`pixel_delta_threshold` must be in 0..=255, got {raw}"),
        )
    })?;
    let max_changed_ratio = required_f64(index, object, "max_changed_ratio")?;
    if !(0.0..=1.0).contains(&max_changed_ratio) {
        return Err(malformed(
            index,
            format!("`max_changed_ratio` must be in [0, 1], got {max_changed_ratio}"),
        ));
    }
    Ok(VisualTolerance::ChangedRatioWithin {
        pixel_delta_threshold,
        max_changed_ratio,
    })
}

/// Parses the perceptual-hash Hamming tolerance.
fn parse_hamming_tolerance(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<VisualTolerance> {
    only_keys(
        index,
        object,
        &[
            "kind",
            "field",
            "op",
            "max_hamming_distance",
            "confidence_min",
        ],
    )?;
    let raw = required_u64(index, object, "max_hamming_distance")?;
    if raw > u64::from(MAX_HAMMING_DISTANCE) {
        return Err(malformed(
            index,
            format!(
                "`max_hamming_distance` must be <= {MAX_HAMMING_DISTANCE} so the random-pair \
                 false-match rate stays below 5%, got {raw}"
            ),
        ));
    }
    let max_hamming_distance = u32::try_from(raw).map_err(|_| {
        malformed(
            index,
            format!("`max_hamming_distance` must fit in u32, got {raw}"),
        )
    })?;
    Ok(VisualTolerance::HammingWithin {
        max_hamming_distance,
    })
}

/// The reference image, observed image, and confidence one visual assertion evaluates.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualObservation {
    /// The reference image the step is expected to produce.
    pub expected: GrayImage,
    /// The image actually observed after the step.
    pub actual: GrayImage,
    /// How trustworthy the observation is (`0.0..=1.0`), supplied by the caller.
    pub confidence: f64,
}

impl VisualObservation {
    /// Builds an observation, rejecting a confidence outside `[0.0, 1.0]`.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::ConfidenceOutOfRange`] when `confidence` is NaN or outside
    /// `[0.0, 1.0]`, mapping to `ErrorCode::ToolInvalidArgs`.
    pub fn new(
        expected: GrayImage,
        actual: GrayImage,
        confidence: f64,
    ) -> Result<Self, VisualError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(VisualError::ConfidenceOutOfRange { confidence });
        }
        Ok(Self {
            expected,
            actual,
            confidence,
        })
    }
}

/// The structured metric behind a visual verdict.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum VisualMetric {
    /// Mean absolute pixel difference, with its inclusive upper bound.
    MeanAbsDiff {
        /// Observed mean absolute difference.
        value: f64,
        /// Inclusive upper bound from the assertion.
        max_allowed: u8,
    },
    /// Fraction of pixels whose difference exceeded the change threshold.
    ChangedPixelRatio {
        /// Observed `changed / total`.
        ratio: f64,
        /// Inclusive upper bound from the assertion.
        max_allowed: f64,
        /// Number of pixels counted as changed.
        changed_pixels: u32,
        /// Total number of compared pixels.
        total_pixels: u32,
    },
    /// Hamming distance between two 64-bit perceptual hashes.
    HammingDistance {
        /// Observed distance.
        distance: u32,
        /// Inclusive upper bound from the assertion.
        max_allowed: u32,
    },
}

/// The three-valued result of one visual assertion.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum VisualVerdict {
    /// The images matched inside the tolerance and the confidence was high enough.
    Satisfied {
        /// The measured metric.
        metric: VisualMetric,
        /// Observed confidence.
        confidence: f64,
    },
    /// The observation disproved the assertion.
    Falsified {
        /// The measured metric.
        metric: VisualMetric,
        /// Observed confidence.
        confidence: f64,
        /// What the assertion required, rendered for the model and the user.
        expected: String,
        /// What the observation actually showed, rendered for the model and the user.
        actual: String,
    },
    /// The result cannot be trusted as a success: low confidence or incomparable images.
    NeedsHuman {
        /// Why the result is not usable as success.
        reason: String,
        /// Observed confidence.
        confidence: f64,
        /// Minimum confidence the assertion required.
        confidence_min: f64,
    },
}

impl VisualVerdict {
    /// Whether the assertion was satisfied.
    #[must_use]
    pub const fn is_satisfied(&self) -> bool {
        matches!(self, Self::Satisfied { .. })
    }

    /// Whether the assertion was falsified.
    #[must_use]
    pub const fn is_falsified(&self) -> bool {
        matches!(self, Self::Falsified { .. })
    }

    /// Whether the result must not be treated as a success.
    #[must_use]
    pub const fn requires_human(&self) -> bool {
        matches!(self, Self::NeedsHuman { .. })
    }

    /// Projects this verdict onto the existing three-valued postcondition outcome.
    ///
    /// `NeedsHuman` becomes `NotEvaluable`, so the existing reduction turns it into
    /// `VerifyOutcome::Inconclusive` with `ErrorCode::VerifyFailed`; only `Satisfied` can ever
    /// contribute to a `VerificationReceipt`.
    #[must_use]
    pub fn to_assertion_outcome(&self) -> AssertionOutcome {
        match self {
            Self::Satisfied { .. } => AssertionOutcome::Satisfied,
            Self::Falsified {
                expected, actual, ..
            } => AssertionOutcome::Falsified {
                expected: expected.clone(),
                actual: actual.clone(),
            },
            Self::NeedsHuman { reason, .. } => AssertionOutcome::NotEvaluable {
                reason: reason.clone(),
            },
        }
    }
}

/// Evaluates one parsed visual assertion against one observation.
///
/// # Errors
///
/// Returns a [`VisualError`] only when the inputs are internally unusable (for example the two
/// pixel buffers have different lengths, or a hash lookup breaks its own invariant). A low
/// confidence or a dimension mismatch is **not** an error: it is a
/// [`VisualVerdict::NeedsHuman`] result, because the observation is well-formed but cannot decide
/// the assertion.
pub fn evaluate_visual_assert(
    assertion: &VisualAssert,
    observation: &VisualObservation,
) -> Result<VisualVerdict, VisualError> {
    if observation.confidence < assertion.confidence_min {
        return Ok(VisualVerdict::NeedsHuman {
            reason: format!(
                "visual confidence {} is below confidence_min {}; a low-confidence result cannot \
                 be treated as success",
                observation.confidence, assertion.confidence_min
            ),
            confidence: observation.confidence,
            confidence_min: assertion.confidence_min,
        });
    }

    if observation.expected.width() != observation.actual.width()
        || observation.expected.height() != observation.actual.height()
    {
        return Ok(VisualVerdict::NeedsHuman {
            reason: format!(
                "reference image is {}x{} but the observed image is {}x{}; they cannot be compared",
                observation.expected.width(),
                observation.expected.height(),
                observation.actual.width(),
                observation.actual.height()
            ),
            confidence: observation.confidence,
            confidence_min: assertion.confidence_min,
        });
    }

    match (assertion.field, assertion.tolerance) {
        (VisualField::Pixels, VisualTolerance::MeanAbsDiffWithin { max_mean_abs_diff }) => {
            evaluate_mean_abs_diff(observation, max_mean_abs_diff)
        }
        (
            VisualField::Pixels,
            VisualTolerance::ChangedRatioWithin {
                pixel_delta_threshold,
                max_changed_ratio,
            },
        ) => evaluate_changed_ratio(observation, pixel_delta_threshold, max_changed_ratio),
        (
            VisualField::Phash | VisualField::Dhash,
            VisualTolerance::HammingWithin {
                max_hamming_distance,
            },
        ) => evaluate_hamming(assertion.field, observation, max_hamming_distance),
        _ => Err(VisualError::Inconsistent {
            operation: "evaluate visual assert",
            reason: format!(
                "field {} does not match tolerance {}",
                assertion.field.label(),
                assertion.tolerance.label()
            ),
        }),
    }
}

/// Evaluates the `mean_abs_diff_within` tolerance.
fn evaluate_mean_abs_diff(
    observation: &VisualObservation,
    max_mean_abs_diff: u8,
) -> Result<VisualVerdict, VisualError> {
    let (sum, total) = absolute_difference_sum(&observation.expected, &observation.actual)?;
    let mean = f64::from(sum) / f64::from(total);
    let metric = VisualMetric::MeanAbsDiff {
        value: mean,
        max_allowed: max_mean_abs_diff,
    };
    if mean <= f64::from(max_mean_abs_diff) {
        Ok(VisualVerdict::Satisfied {
            metric,
            confidence: observation.confidence,
        })
    } else {
        Ok(VisualVerdict::Falsified {
            metric,
            confidence: observation.confidence,
            expected: format!("mean_abs_diff <= {max_mean_abs_diff}"),
            actual: format!("mean_abs_diff = {mean:.4}"),
        })
    }
}

/// Evaluates the `changed_ratio_within` tolerance.
fn evaluate_changed_ratio(
    observation: &VisualObservation,
    pixel_delta_threshold: u8,
    max_changed_ratio: f64,
) -> Result<VisualVerdict, VisualError> {
    let expected = observation.expected.pixels();
    let actual = observation.actual.pixels();
    ensure_same_buffer_length(expected, actual)?;
    let mut changed: u32 = 0;
    for (left, right) in expected.iter().zip(actual.iter()) {
        if left.abs_diff(*right) > pixel_delta_threshold {
            changed = changed
                .checked_add(1)
                .ok_or_else(|| VisualError::Inconsistent {
                    operation: "count changed pixels",
                    reason: "changed-pixel counter overflowed u32".to_owned(),
                })?;
        }
    }
    let total = u32::try_from(expected.len()).map_err(|_| VisualError::Inconsistent {
        operation: "count changed pixels",
        reason: format!("pixel count {} does not fit in u32", expected.len()),
    })?;
    let ratio = f64::from(changed) / f64::from(total);
    let metric = VisualMetric::ChangedPixelRatio {
        ratio,
        max_allowed: max_changed_ratio,
        changed_pixels: changed,
        total_pixels: total,
    };
    if ratio <= max_changed_ratio {
        Ok(VisualVerdict::Satisfied {
            metric,
            confidence: observation.confidence,
        })
    } else {
        Ok(VisualVerdict::Falsified {
            metric,
            confidence: observation.confidence,
            expected: format!("changed_ratio <= {max_changed_ratio}"),
            actual: format!(
                "changed_ratio = {ratio:.6} ({changed}/{total} pixels above delta \
                 {pixel_delta_threshold})"
            ),
        })
    }
}

/// Evaluates the `hamming_within` tolerance.
fn evaluate_hamming(
    field: VisualField,
    observation: &VisualObservation,
    max_hamming_distance: u32,
) -> Result<VisualVerdict, VisualError> {
    let expected_hash = compute_hash(field, &observation.expected)?;
    let actual_hash = compute_hash(field, &observation.actual)?;
    let distance = hamming_distance(expected_hash, actual_hash);
    let metric = VisualMetric::HammingDistance {
        distance,
        max_allowed: max_hamming_distance,
    };
    if distance <= max_hamming_distance {
        Ok(VisualVerdict::Satisfied {
            metric,
            confidence: observation.confidence,
        })
    } else {
        Ok(VisualVerdict::Falsified {
            metric,
            confidence: observation.confidence,
            expected: format!(
                "{} hamming_distance <= {max_hamming_distance}",
                field.label()
            ),
            actual: format!(
                "{} hamming_distance = {distance} (expected {}, actual {})",
                field.label(),
                expected_hash.to_hex(),
                actual_hash.to_hex()
            ),
        })
    }
}

/// Computes the hash named by `field`.
fn compute_hash(field: VisualField, image: &GrayImage) -> Result<PerceptualHash, VisualError> {
    match field {
        VisualField::Phash => compute_phash(image),
        VisualField::Dhash => compute_dhash(image),
        VisualField::Pixels => Err(VisualError::Inconsistent {
            operation: "compute perceptual hash",
            reason: "`pixels` has no perceptual hash".to_owned(),
        }),
    }
}

/// Sums absolute pixel differences and returns `(sum, total)`.
fn absolute_difference_sum(
    expected: &GrayImage,
    actual: &GrayImage,
) -> Result<(u32, u32), VisualError> {
    let expected_pixels = expected.pixels();
    let actual_pixels = actual.pixels();
    ensure_same_buffer_length(expected_pixels, actual_pixels)?;
    let mut sum: u32 = 0;
    for (left, right) in expected_pixels.iter().zip(actual_pixels.iter()) {
        sum = sum
            .checked_add(u32::from(left.abs_diff(*right)))
            .ok_or_else(|| VisualError::Inconsistent {
                operation: "sum absolute pixel differences",
                reason: "absolute-difference sum overflowed u32".to_owned(),
            })?;
    }
    let total = u32::try_from(expected_pixels.len()).map_err(|_| VisualError::Inconsistent {
        operation: "sum absolute pixel differences",
        reason: format!("pixel count {} does not fit in u32", expected_pixels.len()),
    })?;
    Ok((sum, total))
}

/// Rejects a buffer-length mismatch as an internal inconsistency (dimensions were already checked).
fn ensure_same_buffer_length(expected: &[u8], actual: &[u8]) -> Result<(), VisualError> {
    if expected.len() == actual.len() {
        Ok(())
    } else {
        Err(VisualError::Inconsistent {
            operation: "compare image buffers",
            reason: format!(
                "reference buffer has {} bytes but the observed buffer has {}",
                expected.len(),
                actual.len()
            ),
        })
    }
}
