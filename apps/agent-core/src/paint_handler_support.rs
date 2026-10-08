//! Parsing and geometry helpers for the Paint handlers.

use std::time::Instant;

use assistant_platform_api::{
    CoordinateSpace, CoordinateSpaceKind, ElementBounds, ImageRef, NormalizedPoint,
};
use assistant_storage::BlobId;
use assistant_tool_bus::ToolBusError;
use serde_json::{Map, Value, json};

use super::{
    CanvasPoint, CanvasTransform, DrawRequest, PhysicalPointPx, TARGET_RECTANGLE_TOOL,
    TOOL_CANVAS_DRAW_RECTANGLE,
};
use crate::handler_support as support;

pub(super) fn tool_target(tool: &str) -> Option<&'static str> {
    match tool {
        "pencil" => Some("pencil_tool_button"),
        "brush" => Some("brush_tool_button"),
        "eraser" => Some("eraser_tool_button"),
        "fill" => Some("fill_tool_button"),
        "rectangle" => Some(TARGET_RECTANGLE_TOOL),
        _ => None,
    }
}

pub(super) fn parse_draw_request(
    arguments: &Map<String, Value>,
) -> Result<DrawRequest, ToolBusError> {
    let expected_tool = required_string(arguments, "expected_tool", TOOL_CANVAS_DRAW_RECTANGLE)?;
    if expected_tool != "rectangle" {
        return Err(support::invalid_arguments(
            TOOL_CANVAS_DRAW_RECTANGLE,
            "expected_tool must be `rectangle`",
        ));
    }
    let pre_snapshot_blob_id = required_string(
        arguments,
        "pre_snapshot_blob_id",
        TOOL_CANVAS_DRAW_RECTANGLE,
    )?;
    BlobId::parse(pre_snapshot_blob_id).map_err(|error| {
        support::invalid_arguments(
            TOOL_CANVAS_DRAW_RECTANGLE,
            format!("pre_snapshot_blob_id is invalid: {error}"),
        )
    })?;
    Ok(DrawRequest {
        expected_tool: expected_tool.to_owned(),
        expected_color: required_string(
            arguments,
            "expected_foreground_rgb",
            TOOL_CANVAS_DRAW_RECTANGLE,
        )?
        .to_owned(),
        expected_layer_id: required_string(
            arguments,
            "expected_layer_id",
            TOOL_CANVAS_DRAW_RECTANGLE,
        )?
        .to_owned(),
        pre_snapshot_blob_id: pre_snapshot_blob_id.to_owned(),
        start: required_canvas_point(arguments, "start", TOOL_CANVAS_DRAW_RECTANGLE)?,
        end: required_canvas_point(arguments, "end", TOOL_CANVAS_DRAW_RECTANGLE)?,
        coordinate_space: parse_coordinate_space(arguments, TOOL_CANVAS_DRAW_RECTANGLE)?,
    })
}

pub(super) fn parse_canvas_size(text: &str) -> Option<(u32, u32)> {
    let mut numbers = text
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok());
    let width = numbers.next()?;
    let height = numbers.next()?;
    (width > 0 && height > 0).then_some((width, height))
}

pub(super) fn parse_coordinate_space(
    arguments: &Map<String, Value>,
    tool: &str,
) -> Result<CoordinateSpace, ToolBusError> {
    let value = arguments
        .get("coordinate_space")
        .and_then(Value::as_object)
        .ok_or_else(|| support::invalid_arguments(tool, "coordinate_space must be an object"))?;
    let kind = value.get("kind").and_then(Value::as_str).ok_or_else(|| {
        support::invalid_arguments(tool, "coordinate_space.kind must be a string")
    })?;
    if kind != "physical_pixels" {
        return Err(support::invalid_arguments(
            tool,
            "coordinate_space.kind must be `physical_pixels`",
        ));
    }
    let monitor_id = value
        .get("monitor_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            support::invalid_arguments(tool, "coordinate_space.monitor_id must be a string")
        })?;
    let scale = value.get("scale").and_then(Value::as_f64).ok_or_else(|| {
        support::invalid_arguments(tool, "coordinate_space.scale must be a number")
    })?;
    CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, scale, monitor_id)
        .map_err(|error| support::map_platform_error(tool, &error))
}

pub(super) fn canvas_point_to_physical(
    transform: CanvasTransform,
    canvas: CanvasPoint,
    tool: &str,
) -> Result<PhysicalPointPx, ToolBusError> {
    let horizontal = (f64::from(canvas.horizontal) - f64::from(transform.viewport.horizontal_px))
        .mul_add(transform.zoom_ratio, f64::from(transform.bounds.left()));
    let vertical = (f64::from(canvas.vertical) - f64::from(transform.viewport.vertical_px))
        .mul_add(transform.zoom_ratio, f64::from(transform.bounds.top()));
    let horizontal = rounded_i32(horizontal, tool, "screen_x_px")?;
    let vertical = rounded_i32(vertical, tool, "screen_y_px")?;
    if horizontal < transform.bounds.left()
        || horizontal >= transform.bounds.right()
        || vertical < transform.bounds.top()
        || vertical >= transform.bounds.bottom()
    {
        return Err(support::invalid_arguments(
            tool,
            format!(
                "canvas point ({}, {}) maps outside bounds ({}, {})-({}, {})",
                canvas.horizontal,
                canvas.vertical,
                transform.bounds.left(),
                transform.bounds.top(),
                transform.bounds.right(),
                transform.bounds.bottom()
            ),
        ));
    }
    Ok(PhysicalPointPx {
        horizontal_px: horizontal,
        vertical_px: vertical,
    })
}

pub(super) fn rounded_i32(value: f64, tool: &str, field: &str) -> Result<i32, ToolBusError> {
    if !value.is_finite() {
        return Err(support::invalid_arguments(
            tool,
            format!("{field} is not finite"),
        ));
    }
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(support::invalid_arguments(
            tool,
            format!("{field} is outside the i32 range"),
        ));
    }
    format!("{rounded:.0}")
        .parse::<i32>()
        .map_err(|_| support::invalid_arguments(tool, format!("{field} is not an integer")))
}

pub(super) fn normalized_point(
    point: PhysicalPointPx,
    coordinate_space: &CoordinateSpace,
    tool: &str,
) -> Result<NormalizedPoint, ToolBusError> {
    let horizontal = f64::from(point.horizontal_px) / coordinate_space.scale();
    let vertical = f64::from(point.vertical_px) / coordinate_space.scale();
    NormalizedPoint::new(horizontal, vertical)
        .map_err(|error| support::map_platform_error(tool, &error))
}

pub(super) fn bounds_json(bounds: ElementBounds) -> Value {
    json!({
        "left": bounds.left(),
        "top": bounds.top(),
        "right": bounds.right(),
        "bottom": bounds.bottom(),
    })
}

pub(super) fn image_descriptor(image: &ImageRef) -> Value {
    json!({
        "blob_id": image.blob_id(),
        "width": image.width(),
        "height": image.height(),
    })
}

fn required_canvas_point(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<CanvasPoint, ToolBusError> {
    let object = arguments
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| support::invalid_arguments(tool, format!("{field} must be an object")))?;
    Ok(CanvasPoint {
        horizontal: required_non_negative_i32(object, "x", tool)?,
        vertical: required_non_negative_i32(object, "y", tool)?,
    })
}

pub(super) fn required_string<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<&'a str, ToolBusError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| support::invalid_arguments(tool, format!("{field} must be a string")))
}

pub(super) fn required_u8(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<u8, ToolBusError> {
    let value = arguments
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            support::invalid_arguments(tool, format!("{field} must be a non-negative integer"))
        })?;
    u8::try_from(value)
        .map_err(|_| support::invalid_arguments(tool, format!("{field} must be in 0..=255")))
}

pub(super) fn required_non_negative_i32(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<i32, ToolBusError> {
    let value = required_i32(arguments, field, tool)?;
    if value < 0 {
        return Err(support::invalid_arguments(
            tool,
            format!("{field} must be >= 0"),
        ));
    }
    Ok(value)
}

pub(super) fn required_i32(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<i32, ToolBusError> {
    let value = arguments
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| support::invalid_arguments(tool, format!("{field} must be an integer")))?;
    i32::try_from(value)
        .map_err(|_| support::invalid_arguments(tool, format!("{field} is outside the i32 range")))
}

pub(super) fn required_positive_f64(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<f64, ToolBusError> {
    let value = arguments
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| support::invalid_arguments(tool, format!("{field} must be a number")))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(support::invalid_arguments(
            tool,
            format!("{field} must be finite and > 0"),
        ));
    }
    Ok(value)
}

pub(super) const fn verify_failed(message: String) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_004,
        message,
    }
}

pub(super) fn unavailable(component: &str) -> ToolBusError {
    ToolBusError::EnvelopeAssembly {
        tool: "paint.handler".to_owned(),
        reason: format!("{component} is unavailable"),
    }
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
