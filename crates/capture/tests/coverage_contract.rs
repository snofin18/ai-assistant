//! Contract tests for capture error classification and stable display text.

use std::error::Error;

use assistant_capture::{CaptureError, ScrollCleanupError};
use assistant_dlp::RedactError;
use assistant_platform_api::{ErrorCode, PlatformError};

fn assert_error_source(error: &CaptureError) {
    assert!(error.source().is_some());
}

#[test]
fn test_redaction_error_code_is_tool_invalid_args() {
    let error = CaptureError::Redaction(RedactError::NoRules);

    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        error.to_string(),
        "capture redaction failed: redaction was requested without any rules"
    );
    assert_error_source(&error);
}

#[test]
fn test_platform_error_code_and_source_are_preserved() {
    let error = CaptureError::Platform(PlatformError::new(
        ErrorCode::CapabilityMissing,
        "capture unavailable",
    ));

    assert_eq!(error.code(), ErrorCode::CapabilityMissing);
    assert_eq!(
        error.to_string(),
        "capture platform call failed: CapabilityMissing: capture unavailable (capability.id + adapter.id)"
    );
    assert_error_source(&error);
}

#[test]
fn test_scroll_zero_steps_error_has_stable_code_and_display() {
    let error = ScrollCleanupError::ZeroSteps;

    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        error.to_string(),
        "scroll cleanup requires at least one step"
    );
}

#[test]
fn test_scroll_limit_error_has_stable_code_and_display() {
    let error = ScrollCleanupError::StepLimitExceeded;

    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        error.to_string(),
        "scroll cleanup exceeded the hard step limit"
    );
}
