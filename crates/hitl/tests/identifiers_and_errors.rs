//! Identifier and error-code contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_hitl::{ApprovalRequestId, SubjectId};
use assistant_protocol::ErrorCode;

#[test]
fn test_identifier_accepts_stable_unicode_text_and_rejects_empty() {
    let identifier = ApprovalRequestId::new("approval_1").expect("valid id");
    assert_eq!(identifier.as_str(), "approval_1");
    assert_eq!(identifier.to_string(), "approval_1");

    let error = ApprovalRequestId::new("").expect_err("empty id must fail");
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_identifier_rejects_control_characters_and_oversize_values() {
    assert!(SubjectId::new("user\n1").is_err());
    assert!(SubjectId::new("x".repeat(257)).is_err());
}
