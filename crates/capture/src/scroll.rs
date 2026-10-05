//! Bounded scroll-cleanup planning for multi-capture sequences.
//!
//! This module deliberately plans only. Platform scrolling and screenshot
//! stitching require real pixels and platform capabilities, so they belong to
//! later platform work.

use std::fmt;

use assistant_platform_api::ErrorCode;

/// Hard upper bound for one scroll-cleanup sequence (ADR-0063 / ADR-0073).
pub const MAX_SCROLL_STEPS: usize = 8;

/// Direction used when merging successive scroll captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollMergeDirection {
    /// Capture from the top of the content and move downward.
    TopToBottom,
    /// Capture from the bottom of the content and move upward.
    BottomToTop,
}

/// Validated, bounded scroll-cleanup plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollCleanupPlan {
    direction: ScrollMergeDirection,
    steps: usize,
    overlap_pixels: u32,
}

impl ScrollCleanupPlan {
    /// Construct a bounded scroll-cleanup plan.
    ///
    /// # Errors
    /// Zero steps or more than [`MAX_SCROLL_STEPS`] steps return
    /// [`ScrollCleanupError`].
    pub const fn new(
        direction: ScrollMergeDirection,
        steps: usize,
        overlap_pixels: u32,
    ) -> Result<Self, ScrollCleanupError> {
        if steps == 0 {
            return Err(ScrollCleanupError::ZeroSteps);
        }
        if steps > MAX_SCROLL_STEPS {
            return Err(ScrollCleanupError::StepLimitExceeded);
        }
        Ok(Self {
            direction,
            steps,
            overlap_pixels,
        })
    }

    /// Merge direction for the sequence.
    #[must_use]
    pub const fn direction(self) -> ScrollMergeDirection {
        self.direction
    }

    /// Number of scroll steps.
    #[must_use]
    pub const fn steps(self) -> usize {
        self.steps
    }

    /// Pixel overlap expected between adjacent captures.
    #[must_use]
    pub const fn overlap_pixels(self) -> u32 {
        self.overlap_pixels
    }
}

/// Invalid scroll-cleanup plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollCleanupError {
    /// The plan requested no scroll steps.
    ZeroSteps,
    /// The plan exceeded [`MAX_SCROLL_STEPS`].
    StepLimitExceeded,
}

impl ScrollCleanupError {
    /// Stable error category for callers.
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        ErrorCode::ToolInvalidArgs
    }
}

impl fmt::Display for ScrollCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroSteps => "scroll cleanup requires at least one step",
            Self::StepLimitExceeded => "scroll cleanup exceeded the hard step limit",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ScrollCleanupError {}
