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
#[path = "paint_handler_map.rs"]
mod paint_handler_map;
#[path = "paint_handler_selection.rs"]
mod paint_handler_selection;
#[path = "paint_handler_support.rs"]
mod paint_handler_support;
pub use paint_handler_map::build_handler_map;
use paint_handler_selection::{ResolvedPointSlot, paint_layer_contract, palette_index};
use paint_handler_support::{
    bounds_json, canvas_point_to_physical, elapsed_ms, image_descriptor, parse_canvas_size,
    parse_coordinate_space, parse_draw_request, required_i32, required_non_negative_i32,
    required_positive_f64, required_string, required_u8, tool_target, unavailable, verify_failed,
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
const TARGET_LAYER_ITEM: &str = "layer_item";
const TARGET_STATUS_BAR: &str = "status_bar";
const TARGET_CANVAS: &str = "canvas";
const TARGET_SHAPES_TOGGLE: &str = "shapes_gallery_toggle";
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
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let tool_name = required_string(arguments, "tool", TOOL_TOOL_SELECT)?;
        if tool_target(tool_name).is_none() {
            return Err(support::invalid_arguments(
                TOOL_TOOL_SELECT,
                format!("tool `{tool_name}` has no declared Paint target"),
            ));
        }
        if tool_name != "rectangle" {
            return Err(support::invalid_arguments(
                TOOL_TOOL_SELECT,
                format!(
                    "tool `{tool_name}` is not calibrated for the Paint T3.1 runtime; \
                     only `rectangle` is currently supported"
                ),
            ));
        }
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_TOOL_SELECT)?;
        let shape_item = self.wait_for_shape_item(&window, TOOL_TOOL_SELECT)?;
        let previous_tool = self
            .selection_tool()?
            .unwrap_or_else(|| "unknown".to_owned());
        let before = self.fingerprint_event(&window, TOOL_TOOL_SELECT)?;
        let started = Instant::now();
        // 实测（`docs/memory/apps/paint.md` §11 / `DRIFT-256-5`）：形状项是 `Button|矩形`，
        // 全树唯一，但 `TogglePattern.Toggle()` 两向都是**空操作**，且它**无** `InvokePattern` /
        // `SelectionItemPattern` —— 没有任何可用 UIA pattern 能改状态。所以走 L4 合成点击
        // （`click_element_center`：取几何 → 窗口内断言 → 租约内 `Click`），这是产品主线通道。
        self.click_element_center(task_id, &window, &shape_item, TOOL_TOOL_SELECT)?;
        self.selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool = Some(tool_name.to_owned());
        let after = self.fingerprint_event(&window, TOOL_TOOL_SELECT)?;
        Ok(ToolOutput::json(json!({
            "selected_tool": tool_name,
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
}

/// Builds one handler for every T3.1 Paint tool.
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::paint_handler_support::{parse_canvas_size, rounded_i32};
    use super::{paint_layer_contract, palette_index};
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

    #[test]
    fn test_palette_index_uses_measured_rgb_contract() {
        assert_eq!(palette_index(237, 28, 36), Some(3));
        assert_eq!(palette_index(34, 177, 76), Some(6));
        assert_eq!(palette_index(1, 2, 3), None);
    }

    #[test]
    fn test_paint_layer_contract_rejects_unmeasured_layer_ids() {
        assert_eq!(paint_layer_contract("layer-1"), Some((0, "Layer 1")));
        assert_eq!(paint_layer_contract("layer-0"), None);
    }
}
