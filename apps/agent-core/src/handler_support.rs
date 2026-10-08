//! Shared error and polling helpers for binary-layer tool handlers.
//!
//! These helpers translate platform errors and bounded future results; they do
//! not resolve targets or execute tools.

use std::future::Future;
use std::task::{Context, Poll, Waker};

use assistant_platform_api::{ErrorCode, PlatformError};
use assistant_tool_bus::ToolBusError;

/// Creates a handler argument error.
pub fn invalid_arguments(tool: &str, reason: impl Into<String>) -> ToolBusError {
    ToolBusError::InvalidArguments {
        tool: tool.to_owned(),
        reason: reason.into(),
    }
}

/// Maps a platform error into the tool-bus error vocabulary.
pub fn map_platform_error(tool: &str, error: &PlatformError) -> ToolBusError {
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

/// Executes only immediately-ready provider futures.
///
/// The platform providers expose synchronous work as immediately-ready futures. A pending future
/// means that assumption no longer holds, so the handler fails closed instead of blocking or
/// spawning.
pub fn poll_immediate<F: Future>(
    future: F,
    tool: &str,
    operation: &str,
) -> Result<F::Output, ToolBusError> {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(ToolBusError::Transport {
            reason: format!("{tool}: {operation} future unexpectedly returned Pending"),
        }),
    }
}
