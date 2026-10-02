//! Pointer calibration checks for the first display combination.
//!
//! Responsibilities:
//! - Validate a small, bounded sample set produced by a first-use calibration pass.
//! - Compare expected and observed physical points per display.
//! - Fail closed when any sample exceeds the caller-provided pixel tolerance.
//!
//! Boundaries:
//! - This module does not move the pointer, click a target, or operate a GUI.
//! - It does not enumerate displays; the caller supplies the display ordinals.
//! - It does not persist calibration results.
//!
//! Invariants:
//! - At least [`MINIMUM_CALIBRATION_SAMPLES`] samples are required.
//! - No more than [`MAXIMUM_CALIBRATION_SAMPLES`] samples are accepted.
//! - A returned report always means every sample is within tolerance.
//!
//! Typical use: collect known points, then call [`calibrate_pointer_samples`].
//! Related: architecture v2 section 6.9, TASK-040, ADR-0063.

use assistant_platform_api::{ErrorCode, PhysicalPoint, PlatformError, PlatformResult};
use std::collections::BTreeSet;
/// Minimum number of samples required for a first-use calibration.
pub const MINIMUM_CALIBRATION_SAMPLES: usize = 3;
/// Hard upper bound for samples retained by a single calibration check.
pub const MAXIMUM_CALIBRATION_SAMPLES: usize = 64;

/// One expected/observed pair collected from a display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationSample {
    /// Display ordinal supplied by the caller.
    display_ordinal: u16,
    /// Point requested by the coordinator in physical pixels.
    expected: PhysicalPoint,
    /// Point observed after the calibration click or cursor move.
    observed: PhysicalPoint,
}

impl CalibrationSample {
    /// Construct one calibration sample.
    #[must_use]
    pub const fn new(
        display_ordinal: u16,
        expected: PhysicalPoint,
        observed: PhysicalPoint,
    ) -> Self {
        Self {
            display_ordinal,
            expected,
            observed,
        }
    }

    /// Display ordinal supplied by the caller.
    #[must_use]
    pub const fn display_ordinal(self) -> u16 {
        self.display_ordinal
    }

    /// Point requested by the coordinator.
    #[must_use]
    pub const fn expected(self) -> PhysicalPoint {
        self.expected
    }

    /// Point observed by the calibration pass.
    #[must_use]
    pub const fn observed(self) -> PhysicalPoint {
        self.observed
    }
}

/// Bounded calibration summary returned only when every sample passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerCalibrationReport {
    sample_count: usize,
    display_count: usize,
    maximum_error_pixels: u32,
    total_error_pixels: u64,
    tolerance_pixels: u32,
}

impl PointerCalibrationReport {
    /// Number of accepted samples.
    #[must_use]
    pub const fn sample_count(self) -> usize {
        self.sample_count
    }

    /// Number of distinct display ordinals represented by the samples.
    #[must_use]
    pub const fn display_count(self) -> usize {
        self.display_count
    }

    /// Largest per-axis pixel error among the samples.
    #[must_use]
    pub const fn maximum_error_pixels(self) -> u32 {
        self.maximum_error_pixels
    }

    /// Sum of per-axis pixel errors across every sample.
    #[must_use]
    pub const fn total_error_pixels(self) -> u64 {
        self.total_error_pixels
    }

    /// Tolerance used for this calibration check.
    #[must_use]
    pub const fn tolerance_pixels(self) -> u32 {
        self.tolerance_pixels
    }
}

/// Validate first-use pointer samples against a pixel tolerance.
///
/// The per-sample error is the larger absolute X/Y delta. This is deliberately
/// stricter than Euclidean distance for axis-aligned UI hit testing.
///
/// # Errors
/// - Fewer than [`MINIMUM_CALIBRATION_SAMPLES`] samples -> `ToolInvalidArgs`
/// - More than [`MAXIMUM_CALIBRATION_SAMPLES`] samples -> `ToolInvalidArgs`
/// - Zero tolerance -> `ToolInvalidArgs`
/// - Any sample exceeds tolerance -> `VerifyFailed`
pub fn calibrate_pointer_samples(
    samples: &[CalibrationSample],
    tolerance_pixels: u32,
) -> PlatformResult<PointerCalibrationReport> {
    if tolerance_pixels == 0 {
        return Err(invalid_args(
            "pointer calibration: tolerance must be greater than zero",
        ));
    }
    if samples.len() < MINIMUM_CALIBRATION_SAMPLES {
        return Err(invalid_args(format!(
            "pointer calibration: expected at least {MINIMUM_CALIBRATION_SAMPLES} samples, got {}",
            samples.len()
        )));
    }
    if samples.len() > MAXIMUM_CALIBRATION_SAMPLES {
        return Err(invalid_args(format!(
            "pointer calibration: at most {MAXIMUM_CALIBRATION_SAMPLES} samples are accepted, got {}",
            samples.len()
        )));
    }

    let mut displays = BTreeSet::new();
    let mut maximum_error_pixels = 0_u32;
    let mut total_error_pixels = 0_u64;
    let mut failed = None;

    for sample in samples {
        displays.insert(sample.display_ordinal());
        let error = sample_error_pixels(*sample);
        maximum_error_pixels = maximum_error_pixels.max(error);
        total_error_pixels += u64::from(error);
        if error > tolerance_pixels {
            failed = Some(error);
        }
    }

    if let Some(error) = failed {
        return Err(PlatformError::new(
            ErrorCode::VerifyFailed,
            format!(
                "pointer calibration failed: maximum error is {error} px, tolerance is \
                 {tolerance_pixels} px, samples={}, displays={}",
                samples.len(),
                displays.len()
            ),
        ));
    }

    Ok(PointerCalibrationReport {
        sample_count: samples.len(),
        display_count: displays.len(),
        maximum_error_pixels,
        total_error_pixels,
        tolerance_pixels,
    })
}

/// Per-axis error for one sample.
fn sample_error_pixels(sample: CalibrationSample) -> u32 {
    let delta_x = (i64::from(sample.observed().x_px()) - i64::from(sample.expected().x_px())).abs();
    let delta_y = (i64::from(sample.observed().y_px()) - i64::from(sample.expected().y_px())).abs();
    let maximum = delta_x.max(delta_y);
    u32::try_from(maximum).unwrap_or(u32::MAX)
}

/// Construct `ToolInvalidArgs`.
fn invalid_args(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::ToolInvalidArgs, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_platform_api::{CoordinateSpace, CoordinateSpaceKind, NormalizedPoint};

    /// Test fixture: physical point through the only allowed conversion API.
    fn physical(x: i32, y: i32) -> PhysicalPoint {
        let space = match CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, 1.0, "test") {
            Ok(space) => space,
            Err(failure) => unreachable!("test coordinate space must be valid: {failure}"),
        };
        let point = match NormalizedPoint::new(f64::from(x), f64::from(y)) {
            Ok(point) => point,
            Err(failure) => unreachable!("test point must be valid: {failure}"),
        };
        match point.to_physical(&space) {
            Ok(point) => point,
            Err(failure) => unreachable!("test physical point must be valid: {failure}"),
        }
    }

    #[test]
    fn test_calibrate_pointer_samples_accepts_an_in_tolerance_display_pair() {
        let samples = [
            CalibrationSample::new(0, physical(10, 20), physical(11, 21)),
            CalibrationSample::new(0, physical(30, 40), physical(30, 39)),
            CalibrationSample::new(1, physical(50, 60), physical(50, 60)),
        ];
        let report = match calibrate_pointer_samples(&samples, 2) {
            Ok(report) => report,
            Err(failure) => unreachable!("in-tolerance samples must pass: {failure}"),
        };
        assert_eq!(report.sample_count(), 3);
        assert_eq!(report.display_count(), 2);
        assert_eq!(report.maximum_error_pixels(), 1);
        assert_eq!(report.total_error_pixels(), 2);
        assert_eq!(report.tolerance_pixels(), 2);
    }

    #[test]
    fn test_calibrate_pointer_samples_rejects_one_out_of_tolerance_point() {
        let samples = [
            CalibrationSample::new(0, physical(10, 20), physical(10, 20)),
            CalibrationSample::new(0, physical(30, 40), physical(30, 40)),
            CalibrationSample::new(1, physical(50, 60), physical(53, 60)),
        ];
        let failure = match calibrate_pointer_samples(&samples, 2) {
            Ok(report) => unreachable!("out-of-tolerance samples must fail: {report:?}"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::VerifyFailed);
        assert!(failure.message().contains("3 px"));
        assert!(failure.message().contains("2 px"));
    }

    #[test]
    fn test_calibrate_pointer_samples_rejects_invalid_limits_and_sample_counts() {
        let samples = [CalibrationSample::new(0, physical(1, 1), physical(1, 1))];
        let failure = match calibrate_pointer_samples(&samples, 2) {
            Ok(report) => unreachable!("too few samples must fail: {report:?}"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);

        let enough = [
            CalibrationSample::new(0, physical(1, 1), physical(1, 1)),
            CalibrationSample::new(0, physical(2, 2), physical(2, 2)),
            CalibrationSample::new(0, physical(3, 3), physical(3, 3)),
        ];
        let failure = match calibrate_pointer_samples(&enough, 0) {
            Ok(report) => unreachable!("zero tolerance must fail: {report:?}"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
    }

    #[test]
    fn test_calibrate_pointer_samples_enforces_the_hard_sample_limit() {
        let sample = CalibrationSample::new(0, physical(1, 1), physical(1, 1));
        let samples = vec![sample; MAXIMUM_CALIBRATION_SAMPLES + 1];
        let failure = match calibrate_pointer_samples(&samples, 1) {
            Ok(report) => unreachable!("oversized calibration must fail: {report:?}"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
    }
}
