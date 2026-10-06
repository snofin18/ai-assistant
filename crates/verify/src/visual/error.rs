//! Error type for the visual verification pure algorithms.
//!
//! Responsibility: make every malformed visual input fail loudly with a readable reason and a
//! protocol [`ErrorCode`]. The visual layer never repairs an input (no zero-fill, no truncation,
//! no default tolerance): a caller that declares a 10x10 image with a 50-byte buffer has a
//! contract bug, not a sparse image.
//!
//! Boundary: no IO, no logging, no retry. This module only names failures; parsing `visual_assert`
//! JSON reuses [`crate::error::VerifyError`] so that a malformed postcondition keeps mapping to
//! `ToolInvalidArgs` exactly like the other assertion kinds.

use assistant_protocol::ErrorCode;
use thiserror::Error;

/// Errors raised while building or hashing a grayscale image.
///
/// Every variant maps to a protocol [`ErrorCode`]; malformed caller input is `ToolInvalidArgs`,
/// while a broken internal invariant is `Fatal` because it means the crate's own arithmetic
/// disagreed with its validation (never a user-fixable contract problem).
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum VisualError {
    /// The declared image has a zero width or height.
    #[error("gray image dimensions must be non-zero, got {width}x{height}")]
    EmptyDimensions {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
    },

    /// The declared image is not empty but the pixel buffer is.
    #[error("gray image {width}x{height} requires {width}x{height} bytes, got an empty buffer")]
    EmptyBuffer {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
    },

    /// The pixel buffer length does not match `width * height`.
    #[error(
        "gray image buffer length mismatch: {width}x{height} needs {expected} bytes, got {actual}"
    )]
    BufferSizeMismatch {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
        /// Required buffer length.
        expected: usize,
        /// Observed buffer length.
        actual: usize,
    },

    /// The declared image exceeds the hard pixel bound.
    #[error("gray image {width}x{height} exceeds the {max_pixels}-pixel limit")]
    ImageTooLarge {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
        /// Hard upper bound on the number of pixels.
        max_pixels: usize,
    },

    /// `width * height` does not fit in an addressable buffer length.
    #[error("gray image dimensions {width}x{height} overflow the addressable buffer size")]
    DimensionsOverflow {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
    },

    /// A `u32` dimension cannot be represented as `usize` on this platform.
    #[error("gray image dimension {value} cannot be represented on this platform")]
    DimensionNotRepresentable {
        /// The rejected dimension value.
        value: u32,
    },

    /// A pixel coordinate was requested outside the image bounds.
    #[error("pixel ({x}, {y}) is outside the {width}x{height} image")]
    PixelOutOfBounds {
        /// Requested x coordinate.
        x: usize,
        /// Requested y coordinate.
        y: usize,
        /// Image width in pixels.
        width: usize,
        /// Image height in pixels.
        height: usize,
    },

    /// A confidence value was outside `[0.0, 1.0]`.
    #[error("visual confidence must be in [0, 1], got {confidence}")]
    ConfidenceOutOfRange {
        /// The rejected confidence.
        confidence: f64,
    },

    /// An internal arithmetic or validation invariant broke.
    ///
    /// This should be unreachable after `GrayImage::new`; if it is ever observed, the algorithm
    /// implementation disagrees with its own validation and must be treated as a defect.
    #[error("internal visual invariant broken while {operation}: {reason}")]
    Inconsistent {
        /// Which algorithm step detected the break.
        operation: &'static str,
        /// What exactly was inconsistent.
        reason: String,
    },
}

impl VisualError {
    /// The protocol error category this error belongs to.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::EmptyDimensions { .. }
            | Self::EmptyBuffer { .. }
            | Self::BufferSizeMismatch { .. }
            | Self::ImageTooLarge { .. }
            | Self::DimensionsOverflow { .. }
            | Self::DimensionNotRepresentable { .. }
            | Self::PixelOutOfBounds { .. }
            | Self::ConfidenceOutOfRange { .. } => ErrorCode::ToolInvalidArgs,
            Self::Inconsistent { .. } => ErrorCode::Fatal,
        }
    }
}
