//! Production Paint handlers for the T3.1 vertical slice.
//!
//! Responsibilities:
//! - resolve the adapter's Paint targets inside the main window scope;
//! - perform the seven T3.1 UIA / pointer / capture actions;
//! - return read-back evidence and fingerprints for runtime verification.
//!
//! Boundaries:
//! - does not decide policy or approval;
//! - does not expose platform handles across the `ToolBus`;
//! - does not implement the other three Paint tools or real-GUI calibration.
//!
//! Invariants:
//! 1. every tool comes from the adapter declaration and has exactly one handler;
//! 2. a write returns only after its UIA read-back succeeds;
//! 3. pointer input is lease-protected and uses an explicit `CoordinateSpace`;
//! 4. a missing target, missing sink, invalid bounds, or unreadable state fails
//!    closed instead of fabricating a successful result.
//!
//! Related documents: ADR-0084, ADR-0085, ADR-0067, ADR-0076, and
//! `adapters/com.microsoft.paint/tools/tools.json`.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use assistant_platform_api::{
    CaptureOptions, CoordinateSpace, ElementBounds, Fingerprint, FingerprintScope, ImageRef,
    KeyChord, KeyModifier, KeyTarget, PointerAction, ResolvedElement, ResolvedWindow, Selection,
    UiAutomationProvider, WindowProvider,
};
use assistant_protocol::serde_json;
use assistant_storage::BlobId;
use assistant_tool_bus::{ToolBusError, ToolOutput};
use serde_json::{Map, Value, json};

use crate::handler_support as support;
use crate::target_lease::{TargetLeaseGate, window_lease_key};

#[path = "paint_handler_map.rs"]
mod paint_handler_map;
#[path = "paint_handler_support.rs"]
mod paint_handler_support;

pub use paint_handler_map::build_handler_map;

use paint_handler_support::{
    bounds_json, canvas_point_to_physical, elapsed_ms, image_descriptor, normalized_point,
    parse_canvas_size, parse_coordinate_space, parse_draw_request, required_i32,
    required_non_negative_i32, required_positive_f64, required_string, required_u8, tool_target,
    unavailable, verify_failed,
};

/// `paint.document.new` tool name.
pub const TOOL_DOCUMENT_NEW: &str = "paint.document.new";
/// `paint.tool.select` tool name.
pub const TOOL_TOOL_SELECT: &str = "paint.tool.select";
/// `paint.color.select_foreground` tool name.
pub const TOOL_COLOR_SELECT_FOREGROUND: &str = "paint.color.select_foreground";
/// `paint.layer.select` tool name.
pub const TOOL_LAYER_SELECT: &str = "paint.layer.select";
/// `paint.canvas.resolve_point` tool name.
pub const TOOL_CANVAS_RESOLVE_POINT: &str = "paint.canvas.resolve_point";
/// `paint.canvas.draw_rectangle` tool name.
pub const TOOL_CANVAS_DRAW_RECTANGLE: &str = "paint.canvas.draw_rectangle";
/// `paint.canvas.capture_pixels` tool name.
pub const TOOL_CANVAS_CAPTURE_PIXELS: &str = "paint.canvas.capture_pixels";

const TARGET_MAIN_WINDOW: &str = "main_window";
const TARGET_RECTANGLE_TOOL: &str = "rectangle_tool_button";
const TARGET_FOREGROUND_COLOR: &str = "foreground_color_button";
const TARGET_LAYERS_PANEL: &str = "layers_panel";
const TARGET_LAYER_ITEM: &str = "layer_item";
const TARGET_STATUS_BAR: &str = "status_bar";
const TARGET_CANVAS: &str = "canvas";

#[derive(Debug, Clone)]
struct PaintLayerSelection {
    id: String,
}

#[derive(Debug, Default)]
struct PaintSelectionState {
    tool: Option<String>,
    color: Option<String>,
    layer: Option<PaintLayerSelection>,
    start_point: Option<ResolvedCanvasPoint>,
    end_point: Option<ResolvedCanvasPoint>,
}

#[derive(Debug, Clone)]
struct ResolvedCanvasPoint {
    canvas: CanvasPoint,
    screen: PhysicalPointPx,
}

#[derive(Debug, Clone, Copy)]
struct CanvasPoint {
    horizontal: i32,
    vertical: i32,
}

#[derive(Debug, Clone, Copy)]
struct PhysicalPointPx {
    horizontal_px: i32,
    vertical_px: i32,
}

#[derive(Debug, Clone, Copy)]
struct ViewportOffset {
    horizontal_px: i32,
    vertical_px: i32,
}

#[derive(Debug, Clone, Copy)]
struct CanvasTransform {
    bounds: ElementBounds,
    zoom_ratio: f64,
    viewport: ViewportOffset,
}

#[derive(Debug, Clone, Copy)]
struct DragRequest {
    start: PhysicalPointPx,
    end: PhysicalPointPx,
}

struct DrawRequest {
    expected_tool: String,
    expected_color: String,
    expected_layer_id: String,
    pre_snapshot_blob_id: String,
    start: CanvasPoint,
    end: CanvasPoint,
    coordinate_space: CoordinateSpace,
}

/// Shared state and platform access for Paint tools.
pub struct PaintHandlerContext<P> {
    pub(crate) platform: Arc<P>,
    pub(crate) app_id: String,
    pub(crate) targets: Arc<crate::notepad_targets::NotepadTargetCatalog>,
    pub(crate) input_leases: TargetLeaseGate,
    document_generation: Mutex<u64>,
    selection: Mutex<PaintSelectionState>,
}

impl<P> PaintHandlerContext<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    /// Creates the shared Paint handler context with empty local selection state.
    pub(crate) fn new(
        platform: Arc<P>,
        app_id: String,
        targets: Arc<crate::notepad_targets::NotepadTargetCatalog>,
        input_leases: TargetLeaseGate,
    ) -> Self {
        Self {
            platform,
            app_id,
            targets,
            input_leases,
            document_generation: Mutex::new(0),
            selection: Mutex::new(PaintSelectionState::default()),
        }
    }

    fn document_new_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        if arguments
            .get("discard_unsaved_confirmed")
            .and_then(Value::as_bool)
            != Some(false)
        {
            return Err(support::invalid_arguments(
                TOOL_DOCUMENT_NEW,
                "discard_unsaved_confirmed must be false",
            ));
        }
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_DOCUMENT_NEW)?;
        let before = self.fingerprint_event(&window, TOOL_DOCUMENT_NEW)?;
        let started = Instant::now();
        self.send_key(
            task_id,
            &window,
            "N",
            vec![KeyModifier::Control],
            TOOL_DOCUMENT_NEW,
        )?;
        let status = self.resolve_element(TARGET_STATUS_BAR, &window, TOOL_DOCUMENT_NEW)?;
        let status_text = self.read_element_text(&status, TOOL_DOCUMENT_NEW)?;
        let (canvas_width_px, canvas_height_px) =
            parse_canvas_size(&status_text).ok_or_else(|| {
                support::invalid_arguments(
                    TOOL_DOCUMENT_NEW,
                    format!("status bar did not expose a canvas size: `{status_text}`"),
                )
            })?;
        self.reset_selection()?;
        let generation = self.next_document_generation()?;
        let after = self.fingerprint_event(&window, TOOL_DOCUMENT_NEW)?;
        Ok(ToolOutput::json(json!({
            "document_generation": generation,
            "canvas_width_px": canvas_width_px,
            "canvas_height_px": canvas_height_px,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    fn select_tool_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let tool_name = required_string(arguments, "tool", TOOL_TOOL_SELECT)?;
        let target = tool_target(tool_name).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_TOOL_SELECT,
                format!("tool `{tool_name}` has no declared Paint target"),
            )
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_TOOL_SELECT)?;
        let button = self.resolve_element(target, &window, TOOL_TOOL_SELECT)?;
        let before_state = self.read_element_text(&button, TOOL_TOOL_SELECT)?;
        let previous_tool = if before_state.trim().is_empty() {
            self.selection_tool()?
                .unwrap_or_else(|| "unknown".to_owned())
        } else {
            before_state
        };
        let before = self.fingerprint_event(&window, TOOL_TOOL_SELECT)?;
        let started = Instant::now();
        self.invoke_element(&button, "invoke", TOOL_TOOL_SELECT)?;
        let observed = self.read_element_text(&button, TOOL_TOOL_SELECT)?;
        if observed != tool_name {
            return Err(verify_failed(format!(
                "{TOOL_TOOL_SELECT}: read-back was `{observed}`, expected `{tool_name}`"
            )));
        }
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool = Some(observed.clone());
        let after = self.fingerprint_event(&window, TOOL_TOOL_SELECT)?;
        Ok(ToolOutput::json(json!({
            "selected_tool": observed,
            "previous_tool": previous_tool,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    fn select_foreground_color_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let red = required_u8(arguments, "red", TOOL_COLOR_SELECT_FOREGROUND)?;
        let green = required_u8(arguments, "green", TOOL_COLOR_SELECT_FOREGROUND)?;
        let blue = required_u8(arguments, "blue", TOOL_COLOR_SELECT_FOREGROUND)?;
        let requested = format!("{red},{green},{blue}");
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_COLOR_SELECT_FOREGROUND)?;
        let button = self.resolve_element(
            TARGET_FOREGROUND_COLOR,
            &window,
            TOOL_COLOR_SELECT_FOREGROUND,
        )?;
        let before_state = self.read_element_text(&button, TOOL_COLOR_SELECT_FOREGROUND)?;
        let previous_color = if before_state.trim().is_empty() {
            self.selection_color()?
                .unwrap_or_else(|| "unknown".to_owned())
        } else {
            before_state
        };
        let before = self.fingerprint_event(&window, TOOL_COLOR_SELECT_FOREGROUND)?;
        let started = Instant::now();
        self.set_element_value(&button, &requested, TOOL_COLOR_SELECT_FOREGROUND)?;
        let observed = self.read_element_text(&button, TOOL_COLOR_SELECT_FOREGROUND)?;
        if observed != requested {
            return Err(verify_failed(format!(
                "{TOOL_COLOR_SELECT_FOREGROUND}: read-back was `{observed}`, expected `{requested}`"
            )));
        }
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .color = Some(observed.clone());
        let after = self.fingerprint_event(&window, TOOL_COLOR_SELECT_FOREGROUND)?;
        Ok(ToolOutput::json(json!({
            "foreground_rgb": observed,
            "previous_foreground_rgb": previous_color,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    fn select_layer_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let layer_id = required_string(arguments, "layer_id", TOOL_LAYER_SELECT)?;
        let expected_name = required_string(arguments, "expected_layer_name", TOOL_LAYER_SELECT)?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_LAYER_SELECT)?;
        let panel = self.resolve_element(TARGET_LAYERS_PANEL, &window, TOOL_LAYER_SELECT)?;
        let previous_layer_id = self
            .read_element_text(&panel, TOOL_LAYER_SELECT)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or(self.selection_layer_id()?)
            .unwrap_or_else(|| "unknown".to_owned());
        let item = self.resolve_element(TARGET_LAYER_ITEM, &window, TOOL_LAYER_SELECT)?;
        let before = self.fingerprint_event(&window, TOOL_LAYER_SELECT)?;
        let started = Instant::now();
        self.select_element(
            &item,
            &Selection::ByStableValue(layer_id.to_owned()),
            TOOL_LAYER_SELECT,
        )?;
        let observed_name = self.read_element_text(&item, TOOL_LAYER_SELECT)?;
        if observed_name != expected_name {
            return Err(verify_failed(format!(
                "{TOOL_LAYER_SELECT}: layer name read-back was `{observed_name}`, expected `{expected_name}`"
            )));
        }
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .layer = Some(PaintLayerSelection {
            id: layer_id.to_owned(),
        });
        let after = self.fingerprint_event(&window, TOOL_LAYER_SELECT)?;
        Ok(ToolOutput::json(json!({
            "active_layer_id": layer_id,
            "active_layer_name": observed_name,
            "previous_layer_id": previous_layer_id,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    fn resolve_point_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_CANVAS_RESOLVE_POINT)?;
        let canvas = self.resolve_element(TARGET_CANVAS, &window, TOOL_CANVAS_RESOLVE_POINT)?;
        let bounds = self.element_bounds(&canvas, TOOL_CANVAS_RESOLVE_POINT)?;
        parse_coordinate_space(arguments, TOOL_CANVAS_RESOLVE_POINT)?;
        let canvas = CanvasPoint {
            horizontal: required_non_negative_i32(
                arguments,
                "canvas_x",
                TOOL_CANVAS_RESOLVE_POINT,
            )?,
            vertical: required_non_negative_i32(arguments, "canvas_y", TOOL_CANVAS_RESOLVE_POINT)?,
        };
        let zoom_ratio = required_positive_f64(arguments, "zoom_ratio", TOOL_CANVAS_RESOLVE_POINT)?;
        let viewport = ViewportOffset {
            horizontal_px: required_i32(
                arguments,
                "viewport_offset_x_px",
                TOOL_CANVAS_RESOLVE_POINT,
            )?,
            vertical_px: required_i32(
                arguments,
                "viewport_offset_y_px",
                TOOL_CANVAS_RESOLVE_POINT,
            )?,
        };
        let transform = CanvasTransform {
            bounds,
            zoom_ratio,
            viewport,
        };
        let screen = canvas_point_to_physical(transform, canvas, TOOL_CANVAS_RESOLVE_POINT)?;
        let before = self.fingerprint_event(&window, TOOL_CANVAS_RESOLVE_POINT)?;
        let after = self.fingerprint_event(&window, TOOL_CANVAS_RESOLVE_POINT)?;
        let mut state = self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?;
        let point = ResolvedCanvasPoint { canvas, screen };
        if state.start_point.is_none() {
            state.start_point = Some(point);
        } else {
            state.end_point = Some(point);
        }
        drop(state);
        Ok(ToolOutput::json(json!({
            "screen_x_px": screen.horizontal_px,
            "screen_y_px": screen.vertical_px,
            "canvas_bounds_px": bounds_json(bounds),
            "zoom_ratio": zoom_ratio,
            "viewport_offset_x_px": viewport.horizontal_px,
            "viewport_offset_y_px": viewport.vertical_px,
            "coordinate_space_kind": "physical_pixels",
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": 0,
        })))
    }

    fn draw_rectangle_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let request = parse_draw_request(arguments)?;
        let layer = self.verified_active_layer(&request)?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_CANVAS_DRAW_RECTANGLE)?;
        let start_screen = self.resolved_point(
            request.start,
            ResolvedPointSlot::Start,
            TOOL_CANVAS_DRAW_RECTANGLE,
        )?;
        let end_screen = self.resolved_point(
            request.end,
            ResolvedPointSlot::End,
            TOOL_CANVAS_DRAW_RECTANGLE,
        )?;

        let before_capture = self.capture_window(&window, TOOL_CANVAS_DRAW_RECTANGLE)?;
        if before_capture.blob_id() != request.pre_snapshot_blob_id {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: canvas changed after the pre snapshot \
                 (`{}` -> `{}`)",
                request.pre_snapshot_blob_id,
                before_capture.blob_id()
            )));
        }
        let before = self.fingerprint_event(&window, TOOL_CANVAS_DRAW_RECTANGLE)?;
        let started = Instant::now();
        self.drag(
            task_id,
            &window,
            &request.coordinate_space,
            DragRequest {
                start: start_screen,
                end: end_screen,
            },
            TOOL_CANVAS_DRAW_RECTANGLE,
        )?;
        let after_capture = self.capture_window(&window, TOOL_CANVAS_DRAW_RECTANGLE)?;
        if before_capture.width() != after_capture.width()
            || before_capture.height() != after_capture.height()
        {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: capture dimensions changed during the drag"
            )));
        }
        let after = self.fingerprint_event(&window, TOOL_CANVAS_DRAW_RECTANGLE)?;
        Ok(ToolOutput::json(json!({
            "selected_tool": request.expected_tool,
            "foreground_rgb": request.expected_color,
            "active_layer_id": layer.id,
            "pixel_snapshot_blob_id": after_capture.blob_id(),
            "undo_steps_recorded": 1,
            "visual_observation": {
                "reference": image_descriptor(&before_capture),
                "observed": image_descriptor(&after_capture),
                "confidence": 1.0,
            },
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    fn capture_pixels_output(&self) -> Result<ToolOutput, ToolBusError> {
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_CANVAS_CAPTURE_PIXELS)?;
        let before = self.fingerprint_event(&window, TOOL_CANVAS_CAPTURE_PIXELS)?;
        let started = Instant::now();
        let image = self.capture_window(&window, TOOL_CANVAS_CAPTURE_PIXELS)?;
        let after = self.fingerprint_event(&window, TOOL_CANVAS_CAPTURE_PIXELS)?;
        Ok(ToolOutput::json(json!({
            "blob_id": image.blob_id(),
            "width_px": image.width(),
            "height_px": image.height(),
            "content_address": image.blob_id(),
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    pub(crate) fn capture_initial_fingerprint_data(&self) -> Result<Value, ToolBusError> {
        let window = self.resolve_window(
            TARGET_MAIN_WINDOW,
            crate::runtime_tools::TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT,
        )?;
        let fingerprint = self.fingerprint_event(
            &window,
            crate::runtime_tools::TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT,
        )?;
        Ok(json!({
            "initial_fingerprint": fingerprint.as_str(),
            "fingerprint": fingerprint.as_str(),
            "previous_fingerprint": fingerprint.as_str(),
            "elapsed_ms": 0,
        }))
    }

    fn resolve_window(&self, target: &str, tool: &str) -> Result<ResolvedWindow, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let result = support::poll_immediate(
            self.platform.resolve_window(descriptor),
            tool,
            "resolve_window",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn resolve_element(
        &self,
        target: &str,
        scope: &ResolvedWindow,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let chain =
            assistant_platform_api::SelectorChain::new(descriptor.element_candidates().to_vec());
        let result = support::poll_immediate(
            self.platform.resolve_element(scope, &chain),
            tool,
            "resolve_element",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn element_bounds(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<ElementBounds, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.element_bounds(element),
            tool,
            "element_bounds",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn read_element_text(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<String, ToolBusError> {
        let result = support::poll_immediate(self.platform.read_text(element), tool, "read_text")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn set_element_value(
        &self,
        element: &ResolvedElement,
        value: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result =
            support::poll_immediate(self.platform.set_value(element, value), tool, "set_value")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn invoke_element(
        &self,
        element: &ResolvedElement,
        action: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result = support::poll_immediate(
            self.platform.invoke_action(element, action),
            tool,
            "invoke_action",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn select_element(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result =
            support::poll_immediate(self.platform.select(element, selection), tool, "select")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn fingerprint_event(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<Fingerprint, ToolBusError> {
        let result = support::poll_immediate(
            self.platform
                .fingerprint(window, &FingerprintScope::WholeWindow),
            tool,
            "fingerprint",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn send_key(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        key: &str,
        modifiers: Vec<KeyModifier>,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let chord = KeyChord::new(key.to_owned(), modifiers);
            let result = support::poll_immediate(
                self.platform
                    .key_action(&chord, &KeyTarget::Window(window.clone())),
                tool,
                "key_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    fn drag(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        coordinate_space: &CoordinateSpace,
        points: DragRequest,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let start = normalized_point(points.start, coordinate_space, tool)?;
        let drop_at = normalized_point(points.end, coordinate_space, tool)?;
        let action = PointerAction::DragTo {
            drop_at,
            drop_coordinate_space: coordinate_space.clone(),
        };
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let result = support::poll_immediate(
                self.platform
                    .pointer_action(coordinate_space, start, &action),
                tool,
                "pointer_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    fn capture_window(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ImageRef, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.capture(window, &CaptureOptions::new(true)),
            tool,
            "capture",
        )?;
        let image = result.map_err(|error| support::map_platform_error(tool, &error))?;
        BlobId::parse(image.blob_id()).map_err(|error| ToolBusError::EnvelopeAssembly {
            tool: tool.to_owned(),
            reason: format!("capture returned an invalid content address: {error}"),
        })?;
        Ok(image)
    }

    fn reset_selection(&self) -> Result<(), ToolBusError> {
        *self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))? = PaintSelectionState::default();
        Ok(())
    }

    fn selection_tool(&self) -> Result<Option<String>, ToolBusError> {
        Ok(self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool
            .clone())
    }

    fn selection_color(&self) -> Result<Option<String>, ToolBusError> {
        Ok(self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .color
            .clone())
    }

    fn selection_layer_id(&self) -> Result<Option<String>, ToolBusError> {
        Ok(self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .layer
            .as_ref()
            .map(|layer| layer.id.clone()))
    }

    fn next_document_generation(&self) -> Result<u64, ToolBusError> {
        let mut generation = self
            .document_generation
            .lock()
            .map_err(|_| unavailable("paint document generation state"))?;
        let next = generation
            .checked_add(1)
            .ok_or_else(|| ToolBusError::EnvelopeAssembly {
                tool: TOOL_DOCUMENT_NEW.to_owned(),
                reason: "document generation reached u64::MAX".to_owned(),
            })?;
        *generation = next;
        drop(generation);
        Ok(next)
    }

    fn resolved_point(
        &self,
        canvas: CanvasPoint,
        slot: ResolvedPointSlot,
        tool: &str,
    ) -> Result<PhysicalPointPx, ToolBusError> {
        let state = self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?;
        let point = match slot {
            ResolvedPointSlot::Start => state.start_point.as_ref(),
            ResolvedPointSlot::End => state.end_point.as_ref(),
        }
        .ok_or_else(|| {
            verify_failed(format!(
                "{tool}: canvas point ({}, {}) was not resolved by a prior step",
                canvas.horizontal, canvas.vertical
            ))
        })?;
        if point.canvas.horizontal != canvas.horizontal || point.canvas.vertical != canvas.vertical
        {
            return Err(verify_failed(format!(
                "{tool}: resolved {} point ({}, {}) does not match draw arguments ({}, {})",
                slot.label(),
                point.canvas.horizontal,
                point.canvas.vertical,
                canvas.horizontal,
                canvas.vertical
            )));
        }
        let resolved = point.screen;
        drop(state);
        Ok(resolved)
    }

    fn verified_active_layer(
        &self,
        request: &DrawRequest,
    ) -> Result<PaintLayerSelection, ToolBusError> {
        let state = self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?;
        if state.tool.as_deref() != Some(request.expected_tool.as_str()) {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active tool is {:?}, expected `{}`",
                state.tool, request.expected_tool
            )));
        }
        if state.color.as_deref() != Some(request.expected_color.as_str()) {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active color is {:?}, expected `{}`",
                state.color, request.expected_color
            )));
        }
        let layer = state.layer.clone().ok_or_else(|| {
            verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active layer is unavailable"
            ))
        })?;
        if layer.id != request.expected_layer_id {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active layer is not `{}`",
                request.expected_layer_id
            )));
        }
        drop(state);
        Ok(layer)
    }
}

#[derive(Debug, Clone, Copy)]
enum ResolvedPointSlot {
    Start,
    End,
}

impl ResolvedPointSlot {
    const fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// Builds one handler for every T3.1 Paint tool.
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::paint_handler_support::{parse_canvas_size, rounded_i32};

    #[test]
    fn test_parse_canvas_size_reads_first_two_integers() {
        assert_eq!(parse_canvas_size("418 x 74 px"), Some((418, 74)));
        assert_eq!(parse_canvas_size("canvas 1920 x 1080"), Some((1920, 1080)));
        assert_eq!(parse_canvas_size("no size"), None);
    }

    #[test]
    fn test_rounded_i32_rounds_half_away_from_zero_and_rejects_overflow() {
        assert_eq!(rounded_i32(1.5, "paint.test", "x").expect("round"), 2);
        assert_eq!(rounded_i32(-1.5, "paint.test", "x").expect("round"), -2);
        assert!(rounded_i32(f64::INFINITY, "paint.test", "x").is_err());
    }
}
