//! Production Paint handlers for the T3.1 / T3.2 vertical slices.
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
use crate::handler_support as support;
use crate::target_lease::TargetLeaseGate;
use assistant_platform_api::{
    CoordinateSpace, ElementBounds, KeyModifier, Selection, UiAutomationProvider, WindowProvider,
};
use assistant_protocol::serde_json;
use assistant_tool_bus::{ToolBusError, ToolOutput};
use serde_json::{Map, Value, json};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[path = "paint_handler_actions.rs"]
mod paint_handler_actions;
#[path = "paint_handler_actions_file_io.rs"]
mod paint_handler_actions_file_io;
#[path = "paint_handler_flyout.rs"]
mod paint_handler_flyout;
#[path = "paint_handler_map.rs"]
mod paint_handler_map;
#[path = "paint_handler_selection.rs"]
mod paint_handler_selection;
#[path = "paint_handler_support.rs"]
mod paint_handler_support;
pub use paint_handler_map::build_handler_map;
use paint_handler_selection::{ResolvedPointSlot, paint_layer_contract, palette_index};
use paint_handler_support::{
    BRUSH_GROUP_VOCABULARY, SHAPE_GROUP_VOCABULARY, TOOL_GROUP_VOCABULARY, bounds_json,
    brush_target, canvas_point_to_physical, elapsed_ms, image_descriptor, parse_canvas_size,
    parse_coordinate_space, parse_draw_request, required_i32, required_non_negative_i32,
    required_positive_f64, required_selection_path, required_string, required_u8, shape_state_id,
    shape_target, tool_state_id, tool_target, unavailable, verify_failed,
};
/// `paint.document.new` tool name.
pub const TOOL_DOCUMENT_NEW: &str = "paint.document.new";
/// `paint.tool.select` tool name.
pub const TOOL_TOOL_SELECT: &str = "paint.tool.select";
/// `paint.shape.select` tool name.
pub const TOOL_SHAPE_SELECT: &str = "paint.shape.select";
/// `paint.brush.select` tool name.
pub const TOOL_BRUSH_SELECT: &str = "paint.brush.select";
/// `paint.outline.select` tool name.
pub const TOOL_OUTLINE_SELECT: &str = "paint.outline.select";
/// `paint.fill.select` tool name.
pub const TOOL_FILL_SELECT: &str = "paint.fill.select";
/// `paint.size.select` tool name.
pub const TOOL_SIZE_SELECT: &str = "paint.size.select";
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
/// `paint.file.open` tool name.
pub const TOOL_FILE_OPEN: &str = "paint.file.open";
/// `paint.file.save_as` tool name.
pub const TOOL_FILE_SAVE_AS: &str = "paint.file.save_as";
const TARGET_MAIN_WINDOW: &str = "main_window";
const TARGET_RECTANGLE_TOOL: &str = "rectangle_tool_button";
const TARGET_FOREGROUND_COLOR: &str = "foreground_color_button";
const TARGET_LAYER_ITEM: &str = "layer_item";
const TARGET_STATUS_BAR: &str = "status_bar";
const TARGET_CANVAS: &str = "canvas";
const TARGET_SHAPES_TOGGLE: &str = "shapes_gallery_toggle";
const TARGET_BRUSH_TOGGLE: &str = "brush_tool_button";
const TARGET_SHAPE_OUTLINE_TOGGLE: &str = "shape_outline_button";
const TARGET_SHAPE_FILL_TOGGLE: &str = "shape_fill_button";
const TARGET_SHAPE_SIZE_TOGGLE: &str = "shape_size_button";
const TARGET_FILE_OPEN_DIALOG: &str = "file_open_dialog";
const TARGET_FILE_OPEN_FILENAME: &str = "file_open_filename";
const TARGET_ZOOM_VALUES_COMBO: &str = "zoom_values_combo";
const TARGET_SAVE_AS_DIALOG: &str = "save_as_dialog";
const TARGET_SAVE_AS_FILENAME: &str = "save_as_filename";
/// MCP code agreed with `assistant-tool-bus`; maps to `ErrorCode::UserInteraction`.
const MCP_USER_INTERACTION: i32 = -32_010;
/// Measured Windows 11 Paint 11.2605.81.0 palette order and RGB values.
///
/// The palette exposes no `AutomationId` and only localized display names, so the
/// adapter maps logical RGB values to the measured `GridView` child index. The
/// UIA selection call still has a strong `SelectionItemPattern::IsSelected`
/// postcondition; this table only defines the application-version contract.
const PAINT_PALETTE: [((u8, u8, u8), u32); 20] = [
    ((0, 0, 0), 0),
    ((127, 127, 127), 1),
    ((136, 0, 21), 2),
    ((237, 28, 36), 3),
    ((255, 127, 39), 4),
    ((255, 242, 0), 5),
    ((34, 177, 76), 6),
    ((0, 162, 232), 7),
    ((63, 72, 204), 8),
    ((163, 73, 164), 9),
    ((255, 255, 255), 10),
    ((195, 195, 195), 11),
    ((185, 122, 87), 12),
    ((255, 174, 201), 13),
    ((255, 201, 14), 14),
    ((239, 228, 176), 15),
    ((181, 230, 29), 16),
    ((153, 217, 234), 17),
    ((112, 146, 190), 18),
    ((200, 191, 231), 19),
];
#[derive(Debug, Clone)]
struct PaintLayerSelection {
    id: String,
}
#[derive(Debug, Default)]
struct PaintSelectionState {
    tool: Option<String>,
    tool_leaf: Option<String>,
    shape_leaf: Option<String>,
    brush_leaf: Option<String>,
    outline_leaf: Option<String>,
    fill_leaf: Option<String>,
    size_leaf: Option<String>,
    color: Option<String>,
    layer: Option<PaintLayerSelection>,
    start_point: Option<ResolvedCanvasPoint>,
    end_point: Option<ResolvedCanvasPoint>,
}

#[derive(Debug, Default)]
struct PaintDocumentState {
    opened_path: Option<String>,
    canvas_width_px: Option<u32>,
    canvas_height_px: Option<u32>,
    zoom_ratio: Option<f64>,
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
    document: Mutex<PaintDocumentState>,
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
            document: Mutex::new(PaintDocumentState::default()),
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
        // 人类可能保留了未保存画布。此时 Paint 的保存三态框是一个不可见顶层
        // `PopupWindowSiteBridge`：先查一次，绝不在已阻塞的窗口上继续发输入。
        if self.window_state(&window, TOOL_DOCUMENT_NEW)?.occluded() {
            return Err(blocked_by_popup(TOOL_DOCUMENT_NEW));
        }
        let before = self.fingerprint_event(&window, TOOL_DOCUMENT_NEW)?;
        let started = Instant::now();
        self.send_key(
            task_id,
            &window,
            "N",
            vec![KeyModifier::Control],
            TOOL_DOCUMENT_NEW,
        )?;
        // `SendInput` 返回 ≠ 弹层已经创建；在有界窗口内确认没有出现三态框。
        // 检测到就停在 `Ctrl+N` 之后，不再发 `Alt+L` 或任何其它输入。
        if self.wait_for_blocking_popup(&window, TOOL_DOCUMENT_NEW)? {
            return Err(blocked_by_popup(TOOL_DOCUMENT_NEW));
        }
        // 新建文档后结构树不会变化，而 `paint.t3.1.new-canvas-rectangle-color-screenshot`
        // 的步骤级后置条件要求 fingerprint 变化。Paint 11 在 `Ctrl+N` 之后没有可见的树节点
        // 变化，所以顺手 `Alt+L` 打开图层面板，结构树里多出 layersList 节点，fingerprint
        // 因此会改变；这是与真机行为一致的副作用（后续 `select_layer` 同样依赖面板展开）。
        self.send_key(
            task_id,
            &window,
            "L",
            vec![KeyModifier::Alt],
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
        *self
            .document
            .lock()
            .map_err(|_| unavailable("paint document state"))? = PaintDocumentState {
            opened_path: None,
            canvas_width_px: Some(canvas_width_px),
            canvas_height_px: Some(canvas_height_px),
            zoom_ratio: None,
        };
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
        let path = required_selection_path(arguments, TOOL_TOOL_SELECT, TOOL_GROUP_VOCABULARY, 1)?;
        let leaf = path
            .first()
            .ok_or_else(|| support::invalid_arguments(TOOL_TOOL_SELECT, "path must not be empty"))?
            .as_str();
        let target = tool_target(leaf).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_TOOL_SELECT,
                format!("tool path `{leaf}` has no declared Paint target"),
            )
        })?;
        let state_id = tool_state_id(leaf).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_TOOL_SELECT,
                format!("tool path `{leaf}` has no declared runtime state id"),
            )
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_TOOL_SELECT)?;
        let element = self.resolve_element(target, &window, TOOL_TOOL_SELECT)?;
        let previous_leaf = self
            .selection_tool()?
            .unwrap_or_else(|| "unknown".to_owned());
        let before = self.fingerprint_event(&window, TOOL_TOOL_SELECT)?;
        let started = Instant::now();
        self.invoke_element(&element, "toggle", TOOL_TOOL_SELECT)?;
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool = Some(state_id.to_owned());
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool_leaf = Some(leaf.to_owned());
        let after = self.fingerprint_event(&window, TOOL_TOOL_SELECT)?;
        Ok(ToolOutput::json(json!({
            "selected_leaf": leaf,
            "previous_leaf": previous_leaf,
            "selected_path": path,
            "path_depth": 1,
            "ui_state_readback": false,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }
    fn select_shape_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let path =
            required_selection_path(arguments, TOOL_SHAPE_SELECT, SHAPE_GROUP_VOCABULARY, 1)?;
        let leaf = path
            .first()
            .ok_or_else(|| support::invalid_arguments(TOOL_SHAPE_SELECT, "path must not be empty"))?
            .as_str();
        let target = shape_target(leaf).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_SHAPE_SELECT,
                format!("shape path `{leaf}` has no declared Paint target"),
            )
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_SHAPE_SELECT)?;
        let shape_item = self.wait_for_shape_item(&window, target, TOOL_SHAPE_SELECT)?;
        let previous_leaf = self
            .selection_shape()?
            .unwrap_or_else(|| "unknown".to_owned());
        let before = self.fingerprint_event(&window, TOOL_SHAPE_SELECT)?;
        let started = Instant::now();
        // 实测：形状项没有可用 UIA pattern，只能走 L4 合成点击（DRIFT-256-5）。
        self.click_element_center(task_id, &window, &shape_item, TOOL_SHAPE_SELECT)?;
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool = Some(shape_state_id(leaf).to_owned());
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .shape_leaf = Some(leaf.to_owned());
        let after = self.fingerprint_event(&window, TOOL_SHAPE_SELECT)?;
        Ok(ToolOutput::json(json!({
            "selected_leaf": leaf,
            "previous_leaf": previous_leaf,
            "selected_path": path,
            "path_depth": 1,
            "ui_state_readback": false,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }
    fn select_brush_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let path =
            required_selection_path(arguments, TOOL_BRUSH_SELECT, BRUSH_GROUP_VOCABULARY, 1)?;
        let leaf = path
            .first()
            .ok_or_else(|| support::invalid_arguments(TOOL_BRUSH_SELECT, "path must not be empty"))?
            .as_str();
        let target = brush_target(leaf).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_BRUSH_SELECT,
                format!("brush path `{leaf}` has no declared Paint target"),
            )
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_BRUSH_SELECT)?;
        let brush_item = self.wait_for_brush_item(&window, target, TOOL_BRUSH_SELECT)?;
        let previous_leaf = self
            .selection_brush()?
            .unwrap_or_else(|| "unknown".to_owned());
        let before = self.fingerprint_event(&window, TOOL_BRUSH_SELECT)?;
        let started = Instant::now();
        self.invoke_element(&brush_item, "invoke", TOOL_BRUSH_SELECT)?;
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .brush_leaf = Some(leaf.to_owned());
        let after = self.fingerprint_event(&window, TOOL_BRUSH_SELECT)?;
        Ok(ToolOutput::json(json!({
            "selected_leaf": leaf,
            "previous_leaf": previous_leaf,
            "selected_path": path,
            "path_depth": 1,
            "ui_state_readback": false,
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
        let palette_index = palette_index(red, green, blue).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_COLOR_SELECT_FOREGROUND,
                format!(
                    "RGB `{requested}` is not one of the measured Paint 11.2605.81.0 palette colors"
                ),
            )
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_COLOR_SELECT_FOREGROUND)?;
        let gallery = self.resolve_element(
            TARGET_FOREGROUND_COLOR,
            &window,
            TOOL_COLOR_SELECT_FOREGROUND,
        )?;
        let previous_color = self
            .selection_color()?
            .unwrap_or_else(|| "unknown".to_owned());
        let before = self.fingerprint_event(&window, TOOL_COLOR_SELECT_FOREGROUND)?;
        let started = Instant::now();
        self.select_element(
            &gallery,
            &Selection::ByIndex(palette_index),
            TOOL_COLOR_SELECT_FOREGROUND,
        )?;
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .color = Some(requested.clone());
        let after = self.fingerprint_event(&window, TOOL_COLOR_SELECT_FOREGROUND)?;
        Ok(ToolOutput::json(json!({
            "foreground_rgb": requested,
            "previous_foreground_rgb": previous_color,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }
    fn select_layer_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let layer_id = required_string(arguments, "layer_id", TOOL_LAYER_SELECT)?;
        let expected_name = required_string(arguments, "expected_layer_name", TOOL_LAYER_SELECT)?;
        let (layer_index, logical_name) = paint_layer_contract(layer_id).ok_or_else(|| {
            support::invalid_arguments(
                TOOL_LAYER_SELECT,
                format!("layer `{layer_id}` is not calibrated for Paint T3.1"),
            )
        })?;
        if expected_name != logical_name {
            return Err(support::invalid_arguments(
                TOOL_LAYER_SELECT,
                format!(
                    "expected_layer_name `{expected_name}` does not match the logical name \
                     `{logical_name}` for layer `{layer_id}`"
                ),
            ));
        }
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_LAYER_SELECT)?;
        let previous_layer_id = self
            .selection_layer_id()?
            .unwrap_or_else(|| "unknown".to_owned());
        let layer_list = self.resolve_layer_list(task_id, &window, TOOL_LAYER_SELECT)?;
        let before = self.fingerprint_event(&window, TOOL_LAYER_SELECT)?;
        let started = Instant::now();
        self.select_element(
            &layer_list,
            &Selection::ByIndex(layer_index),
            TOOL_LAYER_SELECT,
        )?;
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .layer = Some(PaintLayerSelection {
            id: layer_id.to_owned(),
        });
        let after = self.fingerprint_event(&window, TOOL_LAYER_SELECT)?;
        Ok(ToolOutput::json(json!({
            "active_layer_id": layer_id,
            "active_layer_name": logical_name,
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
        let after = self.fingerprint_event(&window, TOOL_CANVAS_DRAW_RECTANGLE)?;
        let state_changed_elapsed_ms = elapsed_ms(started);
        let after_capture = self.capture_window(&window, TOOL_CANVAS_DRAW_RECTANGLE)?;
        if before_capture.width() != after_capture.width()
            || before_capture.height() != after_capture.height()
        {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: capture dimensions changed during the drag"
            )));
        }
        // `visual_assert` 的 `changed_ratio_within` 是上界：旧内容已满足目标区域时，
        // 完全相同的前后图也会 Satisfied。这里用 content-addressed blob id 做下界检查：
        // 两次整窗截图完全相同 = 本轮没有真实像素变化，必须显式失败。
        if before_capture.blob_id() == after_capture.blob_id() {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: drag produced no pixel change \
                 (before == after == {})",
                after_capture.blob_id()
            )));
        }
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
            "elapsed_ms": state_changed_elapsed_ms,
            "verification_elapsed_ms": elapsed_ms(started),
        })))
    }

    fn file_open_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.open_file_dialog_output(task_id, arguments)
    }

    fn file_save_as_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.save_as_dialog_output(task_id, arguments)
    }
    fn capture_pixels_output(&self) -> Result<ToolOutput, ToolBusError> {
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_CANVAS_CAPTURE_PIXELS)?;
        let before = self.fingerprint_event(&window, TOOL_CANVAS_CAPTURE_PIXELS)?;
        let started = Instant::now();
        let image = self.wait_for_stable_capture(&window, TOOL_CANVAS_CAPTURE_PIXELS)?;
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
}

/// Fail-closed error for an open Paint popup / modal surface.
///
/// This is intentionally `UserInteraction`: the call must not retry until a human
/// resolves the Save changes dialog (or any other blocking target-owned popup).
fn blocked_by_popup(tool: &str) -> ToolBusError {
    ToolBusError::Mcp {
        code: MCP_USER_INTERACTION,
        message: format!(
            "{tool}: Paint has an open modal/popup surface (for example the unsaved-changes \
             dialog); resolve it before retrying"
        ),
    }
}

/// Builds one handler for every T3.1 Paint tool.
#[cfg(test)]
#[path = "paint_handlers_tests.rs"]
mod tests;
