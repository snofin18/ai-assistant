//! Paint adapter registry and reserved-operation bridge.
//!
//! Responsibilities:
//! - load `tools.json` and register exactly the seven T3.1 Paint handlers;
//! - provide the reserved initial-fingerprint operation used by the task package;
//! - fail explicitly for Notepad-only host operations that Paint does not use.
//!
//! Boundaries:
//! - does not implement policy, orchestration, or the remaining Paint tools;
//! - does not expose platform handles or let the model call reserved operations.
//!
//! Invariants:
//! 1. the mounted Paint set is exactly `PAINT_EXPECTED_TOOL_NAMES`;
//! 2. future declared Paint tools remain in the adapter catalog but are not mounted;
//! 3. unsupported host operations return an explicit error rather than a placeholder.
//!
//! Related documents: ADR-0084, ADR-0085, and `adapters/com.microsoft.paint/tools/tools.json`.

use std::path::Path;
use std::sync::Arc;

use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_protocol::{ErrorCode, ToolSchema};
use assistant_tool_bus::ToolRegistry;

use crate::notepad_registry::{
    DeclaredToolSelection, NotepadRegistryError, load_and_register_declared_tools,
};
use crate::notepad_targets::NotepadTargetCatalog;
use crate::paint_handlers::{
    PaintHandlerContext, TOOL_CANVAS_CAPTURE_PIXELS, TOOL_CANVAS_DRAW_RECTANGLE,
    TOOL_CANVAS_RESOLVE_POINT, TOOL_COLOR_SELECT_FOREGROUND, TOOL_DOCUMENT_NEW, TOOL_LAYER_SELECT,
    TOOL_TOOL_SELECT, build_handler_map,
};
use crate::runtime_host_ops::{ReservedHostOperationError, ReservedHostOperations};
use crate::target_lease::TargetLeaseGate;

/// Required Paint targets for the T3.1 vertical slice.
pub const PAINT_REQUIRED_TARGETS: &[&str] = &[
    "main_window",
    "rectangle_tool_button",
    "foreground_color_button",
    "layers_panel",
    "layer_item",
    "status_bar",
    "canvas",
];

/// Exact model-visible Paint tool set for T3.1.
pub const PAINT_EXPECTED_TOOL_NAMES: &[&str] = &[
    TOOL_DOCUMENT_NEW,
    TOOL_TOOL_SELECT,
    TOOL_COLOR_SELECT_FOREGROUND,
    TOOL_LAYER_SELECT,
    TOOL_CANVAS_RESOLVE_POINT,
    TOOL_CANVAS_DRAW_RECTANGLE,
    TOOL_CANVAS_CAPTURE_PIXELS,
];

/// Registry plus the catalog/operations supplied to the production host.
pub struct PaintRegistryBuild {
    pub(crate) registry: ToolRegistry,
    pub(crate) tool_schemas: Vec<ToolSchema>,
    pub(crate) host_operations: Arc<dyn ReservedHostOperations>,
}

/// Builds the Paint registry and reserved-operation bridge.
pub fn build_paint_registry<P>(
    platform: Arc<P>,
    targets: Arc<NotepadTargetCatalog>,
    tools_path: &Path,
    input_leases: TargetLeaseGate,
) -> Result<PaintRegistryBuild, NotepadRegistryError>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    let context = Arc::new(PaintHandlerContext::new(
        platform,
        targets.app_id().to_owned(),
        targets,
        input_leases,
    ));
    let handlers = build_handler_map(&context);
    let (registry, tool_schemas) = load_and_register_declared_tools(
        tools_path,
        context.app_id.as_str(),
        PAINT_EXPECTED_TOOL_NAMES,
        DeclaredToolSelection::Subset,
        &handlers,
    )?;
    let host_operations = Arc::new(PaintReservedHostOperations { context });
    Ok(PaintRegistryBuild {
        registry,
        tool_schemas,
        host_operations,
    })
}

struct PaintReservedHostOperations<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ReservedHostOperations for PaintReservedHostOperations<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn inspect_target_path(&self, _target_path: &str) -> Result<serde_json::Value, String> {
        Err("paint host operation `inspect_target_path` is not part of T3.1".to_owned())
    }

    fn set_editor_value(&self, _text: &str) -> Result<serde_json::Value, String> {
        Err("paint host operation `set_editor_value` is not part of T3.1".to_owned())
    }

    fn read_utf8_prefix(
        &self,
        _target_path: Option<&str>,
        _max_text_bytes: u64,
    ) -> Result<serde_json::Value, ReservedHostOperationError> {
        Err(ReservedHostOperationError::new(
            ErrorCode::CapabilityMissing,
            "paint host operation `read_utf8_prefix` is not part of T3.1".to_owned(),
        ))
    }

    fn capture_initial_fingerprint(&self) -> Result<serde_json::Value, String> {
        self.context
            .capture_initial_fingerprint_data()
            .map_err(|error| error.to_string())
    }

    fn capture_rollback_anchor(
        &self,
        _task_id: &str,
        _step_id: &str,
        _sequence: u32,
        _target_path: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        Err("paint rollback anchors are not part of T3.1".to_owned())
    }

    fn execute_rollback(
        &self,
        _task_id: &str,
        _step_id: &str,
        _restore_file: bool,
    ) -> Result<serde_json::Value, String> {
        Err("paint rollback is not part of T3.1".to_owned())
    }

    fn observe_rollback_state(&self, _task_id: &str) -> Result<serde_json::Value, String> {
        Err("paint rollback observation is not part of T3.1".to_owned())
    }

    fn release_task(&self, _task_id: &str) -> Result<(), String> {
        // STUB(TASK-106): the T3.1 slice keeps no per-task Paint state to release.
        Ok(())
    }
}
