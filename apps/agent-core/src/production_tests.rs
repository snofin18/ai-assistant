//! Unit tests for production-root assembly invariants.
//!
//! These tests stay in the module's private test tree so the empty-registry
//! guard is covered without exposing a second assembly surface.

use assistant_protocol::ErrorCode;
use assistant_tool_bus::ToolRegistry;

use super::{ProductionError, validate_registry_not_empty};

#[test]
fn test_empty_registry_is_rejected_with_tool_invalid_args() {
    let result = validate_registry_not_empty(&ToolRegistry::new());
    assert!(
        result.is_err(),
        "empty production registry must be rejected"
    );
    if let Err(error) = result {
        assert!(matches!(
            error,
            ProductionError::InvalidConfiguration {
                field: "tool_registry",
                ..
            }
        ));
        assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    }
}
