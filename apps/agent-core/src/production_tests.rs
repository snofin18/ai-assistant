//! Unit tests for production-root assembly invariants.
//!
//! These tests stay in the module's private test tree so the empty-registry
//! guard is covered without exposing a second assembly surface.

use std::path::PathBuf;
use std::time::Duration;

use assistant_protocol::ErrorCode;
use assistant_tool_bus::ToolRegistry;

use super::{ProductionConfig, ProductionError, UiServerConfig, validate_registry_not_empty};
use crate::target_lease::TargetLeaseRegistry;

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

#[test]
fn test_production_config_clones_share_one_lease_registry() {
    let registry = TargetLeaseRegistry::new();
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-test",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer(PathBuf::from("assistant-test-peer.exe"));
    let config = ProductionConfig::new(
        PathBuf::from("data-root"),
        PathBuf::from("adapter-root"),
        PathBuf::from("task-package.json"),
        ui_config,
    )
    .with_lease_registry(registry);
    let cloned = config.clone();

    assert!(
        config
            .lease_registry
            .shares_manager_with(&cloned.lease_registry),
        "cloned task configs must use one process-local lease manager"
    );
}
