//! Capture orchestration errors.
//!
//! The pipeline keeps platform failures intact and classifies local validation
//! failures as invalid tool arguments so callers can dispatch them without
//! parsing strings.

use std::fmt;

use assistant_dlp::RedactError;
use assistant_platform_api::{ErrorCode, PlatformError};

/// Error returned while preparing or executing a capture.
#[derive(Debug)]
#[non_exhaustive]
pub enum CaptureError {
    /// A redaction rule failed validation before or after capture.
    Redaction(RedactError),
    /// The injected platform provider failed.
    Platform(PlatformError),
}

impl CaptureError {
    /// Stable error category for callers.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Redaction(_) => ErrorCode::ToolInvalidArgs,
            Self::Platform(error) => error.code(),
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Redaction(error) => write!(formatter, "capture redaction failed: {error}"),
            Self::Platform(error) => write!(formatter, "capture platform call failed: {error}"),
        }
    }
}

impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Redaction(error) => Some(error),
            Self::Platform(error) => Some(error),
        }
    }
}
