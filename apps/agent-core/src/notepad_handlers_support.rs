//! Shared error and polling helpers for the Notepad handlers.
//!
//! These helpers only translate platform errors and bounded future results;
//! they do not resolve targets or execute tools.

use assistant_platform_api::{ErrorCode, PlatformError};
use assistant_tool_bus::ToolBusError;

pub(super) fn invalid_arguments(tool: &str, reason: impl Into<String>) -> ToolBusError {
    ToolBusError::InvalidArguments {
        tool: tool.to_owned(),
        reason: reason.into(),
    }
}

pub(super) fn map_platform_error(tool: &str, error: &PlatformError) -> ToolBusError {
    let reason = format!("{:?}: {}", error.code(), error.message());
    match error.code() {
        ErrorCode::ToolInvalidArgs => ToolBusError::InvalidArguments {
            tool: tool.to_owned(),
            reason,
        },
        ErrorCode::TargetNotFound => ToolBusError::UnknownTool {
            tool: format!("{tool}: {reason}"),
        },
        ErrorCode::CapabilityMissing => ToolBusError::Mcp {
            code: -32_601,
            message: format!("{tool}: {reason}"),
        },
        _ => ToolBusError::Mcp {
            code: -32_000,
            message: format!("{tool}: {reason}"),
        },
    }
}
