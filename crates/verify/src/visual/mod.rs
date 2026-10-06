//! Zero-dependency visual verification: grayscale buffers, pHash / dHash, tolerance assertions,
//! and the `confidence_min` rule (ADR-0074).
//!
//! Responsibilities:
//! - validate the only accepted image input shape ([`GrayImage`]: width / height / row-major
//!   grayscale bytes);
//! - compute the frozen 64-bit pHash and dHash口径 and their Hamming distance;
//! - parse the structured, fail-closed `visual_assert` shape
//!   (`field` + `op` + named tolerance parameters + `confidence_min`);
//! - evaluate a visual assertion into a three-valued [`VisualVerdict`], where low confidence and
//!   incomparable images become [`VisualVerdict::NeedsHuman`] instead of a fake success.
//!
//! Boundaries (what this module deliberately does not do):
//! - **No image codec, color conversion, OCR, template matching, or region selection.** Turning a
//!   platform `ImageRef` into grayscale, and choosing a capture region, belong to the platform /
//!   capture layers.
//! - **No platform access.** No UIA, no Win32, no filesystem, no network, no clock, no randomness.
//! - **No new dependencies.** pHash / dHash / box downsampling are implemented here with `std`;
//!   no `image` / `img_hash` crate is used (ADR-0074 D12).
//! - **No image inside the observation.** `visual_assert` is wired into the engine by ADR-0077:
//!   [`parse_postconditions`](crate::parse_postconditions) accepts the kind and
//!   `Postcondition::VisualAssert` carries the shape below. The **images**, however, are passed
//!   next to the observation (`evaluate_postcondition_with_visual` /
//!   `verify_postconditions_with_visual`) rather than inside `Observation`, which stays
//!   serializable and pixel-free. This module keeps its own [`parse_visual_assert`] /
//!   [`evaluate_visual_assert`] entry points because the engine reuses them.
//!
//! Invariants:
//! 1. A malformed image (empty dimensions, empty buffer, size mismatch, over the pixel cap) is an
//!    explicit error, never zero-filled or truncated.
//! 2. A low-confidence result is [`VisualVerdict::NeedsHuman`]; it is never
//!    [`VisualVerdict::Satisfied`], and [`VisualVerdict::to_assertion_outcome`] maps it to
//!    `NotEvaluable` so the existing reduction yields `Inconclusive` / `VerifyFailed` and cannot
//!    mint a `VerificationReceipt`.
//! 3. The perceptual hashes are pure functions of the validated buffer; equal inputs give
//!    bit-identical hashes, with no clock, randomness, or platform branch.
//! 4. `max_hamming_distance` never exceeds [`MAX_HAMMING_DISTANCE`]; the parse step enforces the
//!    <5% random-pair false-match bound documented in ADR-0074 D7.
//!
//! Typical use:
//! ```
//! use assistant_verify::visual::{
//!     GrayImage, VisualObservation, evaluate_visual_assert, parse_visual_assert,
//! };
//! use assistant_protocol::serde_json::json;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let assertion = parse_visual_assert(
//!     0,
//!     &json!({
//!         "kind": "visual_assert",
//!         "field": "phash",
//!         "op": "hamming_within",
//!         "max_hamming_distance": 10,
//!         "confidence_min": 0.9,
//!     }),
//! )?;
//!
//! let reference = GrayImage::new(2, 2, vec![0, 255, 255, 0])?;
//! let observed = GrayImage::new(2, 2, vec![0, 254, 255, 1])?;
//! let observation = VisualObservation::new(reference, observed, 0.95)?;
//! let verdict = evaluate_visual_assert(&assertion, &observation)?;
//!
//! assert!(verdict.is_satisfied() || verdict.is_falsified());
//! # Ok(())
//! # }
//! ```
//!
//! Related documents: ADR-0074, ADR-0047, ADR-0063, ADR-0071, architecture v2 section 7.4,
//! `docs/spec/testing.md`, `tasks/TASK-042-visual-verify-phash-dhash-confidence.md`.

mod assert;
mod error;
mod hash;
mod image;

#[cfg(test)]
mod tests;

pub use assert::{
    VISUAL_ASSERT_KIND, VisualAssert, VisualField, VisualMetric, VisualObservation, VisualOp,
    VisualTolerance, VisualVerdict, evaluate_visual_assert, parse_visual_assert,
};
pub use error::VisualError;
pub use hash::{
    DHASH_HEIGHT, DHASH_WIDTH, MAX_HAMMING_DISTANCE, PERCEPTUAL_HASH_BITS,
    PHASH_LOW_FREQUENCY_SIDE, PHASH_SIDE, PerceptualHash, compute_dhash, compute_phash,
    hamming_distance,
};
pub use image::{GrayImage, MAX_IMAGE_PIXELS};
