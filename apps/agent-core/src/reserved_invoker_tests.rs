//! Unit tests for the reserved runtime invoker's stable contract.

use super::RESERVED_EXECUTOR_MISSING;
use crate::runtime_tools::{TOOL_PREPARE_ANCHORS, TOOL_REQUEST_APPROVAL};

#[test]
fn test_reserved_reason_is_stable_and_names_no_model_visibility() {
    assert!(RESERVED_EXECUTOR_MISSING.contains("not implemented"));
    assert!(TOOL_REQUEST_APPROVAL.starts_with("assistant.runtime."));
    assert!(TOOL_PREPARE_ANCHORS.starts_with("assistant.runtime."));
}
