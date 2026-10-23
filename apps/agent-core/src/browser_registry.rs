//! Browser CDP tool registration for Edge and Chrome.
//!
//! Responsibilities:
//! - register only the three browser tools that have production handlers;
//! - reuse the shared adapter declaration loader and fail-closed checks;
//! - expose the registry and Planner schemas to the binary assembly layer.
//!
//! Boundaries:
//! - does not launch a browser or establish a CDP connection;
//! - does not register declared-but-unimplemented browser tools;
//! - does not define a second protocol or tool schema representation.
//!
//! Invariants:
//! 1. Edge and Chrome mount exactly `navigate`, `dom_read`, and `wait_network_idle`;
//! 2. form fill, download, screenshot, and shell tools remain unmounted;
//! 3. a missing declaration or handler fails before `ToolBus` startup.

use std::path::Path;
use std::sync::Arc;

use assistant_platform_api::ResolvedCdpTarget;
use assistant_platform_windows::cdp::runtime::RuntimeCdpProvider;
use assistant_protocol::ToolSchema;
use assistant_tool_bus::ToolRegistry;
use thiserror::Error;

use crate::browser_handlers::{
    BrowserCdpRuntime, BrowserHandlerContext, BrowserToolNames, build_handler_map,
};
use crate::notepad_registry::{
    DeclaredToolSelection, NotepadRegistryError, load_and_register_declared_tools,
};

/// Browser tool family accepted by the production registry builder.
pub use crate::browser_handlers::BrowserToolFamily as BrowserFamily;

/// Registry plus the Planner catalog for one browser adapter.
pub struct BrowserRegistryBuild {
    registry: ToolRegistry,
    tool_schemas: Vec<ToolSchema>,
}

impl BrowserRegistryBuild {
    /// Splits the build into the registry consumed by `ToolBus` and its Planner catalog.
    #[must_use]
    pub fn into_parts(self) -> (ToolRegistry, Vec<ToolSchema>) {
        (self.registry, self.tool_schemas)
    }

    /// Registry containing exactly the implemented browser tools.
    #[must_use]
    pub const fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Planner catalog containing exactly the implemented browser tools.
    #[must_use]
    pub fn tool_schemas(&self) -> &[ToolSchema] {
        &self.tool_schemas
    }
}

/// Browser registry failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BrowserRegistryError {
    /// Adapter declaration loading or handler registration failed.
    #[error("browser tool registration failed: {reason}")]
    Registration {
        /// Failure detail.
        reason: String,
    },
}

/// Builds an Edge or Chrome registry around one already connected CDP target.
///
/// No network or browser operation occurs during assembly.
///
/// # Errors
///
/// Returns [`BrowserRegistryError::Registration`] when the adapter declaration is
/// malformed, lacks one of the three implemented tools, or cannot be mounted.
pub fn build_browser_registry(
    provider: Arc<RuntimeCdpProvider>,
    target: ResolvedCdpTarget,
    family: BrowserFamily,
    tools_path: &Path,
) -> Result<BrowserRegistryBuild, BrowserRegistryError> {
    build_browser_registry_with_runtime(provider, target, family, tools_path)
}

fn build_browser_registry_with_runtime<R>(
    runtime: Arc<R>,
    target: ResolvedCdpTarget,
    family: BrowserFamily,
    tools_path: &Path,
) -> Result<BrowserRegistryBuild, BrowserRegistryError>
where
    R: BrowserCdpRuntime + 'static,
{
    let tool_names = family.tool_names();
    let expected_names = tool_names.as_vec();
    let expected_refs = expected_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let context = Arc::new(BrowserHandlerContext {
        runtime,
        target,
        app_id: family.app_id().to_owned(),
    });
    let handlers = build_handler_map(&context, &tool_names);
    load_registry(
        tools_path,
        family.app_id(),
        &expected_refs,
        &tool_names,
        &handlers,
    )
}

fn load_registry(
    tools_path: &Path,
    app_id: &str,
    expected_names: &[&str],
    tool_names: &BrowserToolNames,
    handlers: &std::collections::BTreeMap<String, Arc<dyn assistant_tool_bus::ToolHandler>>,
) -> Result<BrowserRegistryBuild, BrowserRegistryError> {
    if tool_names.as_vec() != expected_names {
        return Err(BrowserRegistryError::Registration {
            reason: "browser tool name order drifted from registry expectations".to_owned(),
        });
    }
    let (registry, tool_schemas) = load_and_register_declared_tools(
        tools_path,
        app_id,
        expected_names,
        DeclaredToolSelection::Subset,
        handlers,
    )
    .map_err(|error| map_registry_error(&error))?;
    Ok(BrowserRegistryBuild {
        registry,
        tool_schemas,
    })
}

fn map_registry_error(error: &NotepadRegistryError) -> BrowserRegistryError {
    BrowserRegistryError::Registration {
        reason: error.to_string(),
    }
}

#[cfg(test)]
#[path = "browser_registry_tests.rs"]
mod tests;
