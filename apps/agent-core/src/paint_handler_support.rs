//! Parsing and geometry helpers for the Paint handlers.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
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

pub(super) const TOOL_GROUP_VOCABULARY: &[&str] =
    &["铅笔", "填充", "文本", "橡皮擦", "颜色选取器", "放大镜"];

pub(super) const SHAPE_GROUP_VOCABULARY: &[&str] = &[
    "直线",
    "曲线",
    "椭圆",
    "矩形",
    "圆角矩形",
    "多边形",
    "三角形",
    "直角三角形",
    "菱形",
    "五边形",
    "六边形",
    "向右键",
    "向左键",
    "向上箭头",
    "向下箭头",
    "四角星",
    "五角星",
    "六角星",
    "圆角矩形标注",
    "椭圆形标注",
    "云形标注",
    "心形",
    "闪电",
];

pub(super) const BRUSH_GROUP_VOCABULARY: &[&str] = &[
    "画笔",
    "毛笔",
    "书写笔",
    "喷枪",
    "油画笔",
    "蜡笔",
    "记号笔",
    "普通铅笔",
    "水彩画笔",
];

pub(super) const OUTLINE_GROUP_VOCABULARY: &[&str] = &[
    "无边框",
    "实心边框",
    "蜡笔",
    "记号笔",
    "油画笔",
    "普通铅笔",
    "水彩画笔",
];

pub(super) const FILL_GROUP_VOCABULARY: &[&str] = &[
    "无填充",
    "纯色填充",
    "蜡笔",
    "记号笔",
    "油画笔",
    "普通铅笔",
    "水彩画笔",
];

/// Shape stroke width (`大小`) vocabulary.
///
/// ADR-0098: unlike the brush / outline / fill groups this vocabulary is the
/// measured **pixel width**, not the menu label. `Size1`'s UIA name is `大小`,
/// which the flyout button and the brush size slider share, so the display name
/// cannot identify an item; the pixel width plus the pinned `AutomationId` can.
pub(super) const SIZE_GROUP_VOCABULARY: &[&str] = &["1 像素", "3 像素", "5 像素", "8 像素"];

pub(super) fn tool_target(leaf: &str) -> Option<&'static str> {
    match leaf {
        "铅笔" => Some("pencil_tool_button"),
        "填充" => Some("fill_tool_button"),
        "文本" => Some("text_tool_button"),
        "橡皮擦" => Some("eraser_tool_button"),
        "颜色选取器" => Some("color_picker_tool_button"),
        "放大镜" => Some("magnifier_tool_button"),
        _ => None,
    }
}

pub(super) fn tool_state_id(leaf: &str) -> Option<&'static str> {
    match leaf {
        "铅笔" => Some("pencil"),
        "填充" => Some("fill"),
        "文本" => Some("text"),
        "橡皮擦" => Some("eraser"),
        "颜色选取器" => Some("color_picker"),
        "放大镜" => Some("magnifier"),
        _ => None,
    }
}

pub(super) fn shape_target(leaf: &str) -> Option<&'static str> {
    match leaf {
        "直线" => Some("shape_line_button"),
        "曲线" => Some("shape_curve_button"),
        "椭圆" => Some("shape_ellipse_button"),
        "矩形" => Some(TARGET_RECTANGLE_TOOL),
        "圆角矩形" => Some("shape_rounded_rectangle_button"),
        "多边形" => Some("shape_polygon_button"),
        "三角形" => Some("shape_triangle_button"),
        "直角三角形" => Some("shape_right_triangle_button"),
        "菱形" => Some("shape_diamond_button"),
        "五边形" => Some("shape_pentagon_button"),
        "六边形" => Some("shape_hexagon_button"),
        "向右键" => Some("shape_right_arrow_button"),
        "向左键" => Some("shape_left_arrow_button"),
        "向上箭头" => Some("shape_up_arrow_button"),
        "向下箭头" => Some("shape_down_arrow_button"),
        "四角星" => Some("shape_four_point_star_button"),
        "五角星" => Some("shape_five_point_star_button"),
        "六角星" => Some("shape_six_point_star_button"),
        "圆角矩形标注" => Some("shape_rounded_rectangle_callout_button"),
        "椭圆形标注" => Some("shape_oval_callout_button"),
        "云形标注" => Some("shape_cloud_callout_button"),
        "心形" => Some("shape_heart_button"),
        "闪电" => Some("shape_lightning_button"),
        _ => None,
    }
}

pub(super) fn shape_state_id(leaf: &str) -> &str {
    match leaf {
        "矩形" => "rectangle",
        other => other,
    }
}

pub(super) fn brush_target(leaf: &str) -> Option<&'static str> {
    match leaf {
        "画笔" => Some("brush_style_brush"),
        "毛笔" => Some("brush_style_calligraphy"),
        "书写笔" => Some("brush_style_marker"),
        "喷枪" => Some("brush_style_airbrush"),
        "油画笔" => Some("brush_style_oil"),
        "蜡笔" => Some("brush_style_crayon"),
        "记号笔" => Some("brush_style_highlighter"),
        "普通铅笔" => Some("brush_style_pencil"),
        "水彩画笔" => Some("brush_style_watercolor"),
        _ => None,
    }
}

pub(super) fn outline_target(leaf: &str) -> Option<&'static str> {
    match leaf {
        "无边框" => Some("outline_style_none"),
        "实心边框" => Some("outline_style_solid"),
        "蜡笔" => Some("outline_style_crayon"),
        "记号笔" => Some("outline_style_marker"),
        "油画笔" => Some("outline_style_oil"),
        "普通铅笔" => Some("outline_style_pencil"),
        "水彩画笔" => Some("outline_style_watercolor"),
        _ => None,
    }
}

pub(super) fn fill_target(leaf: &str) -> Option<&'static str> {
    match leaf {
        "无填充" => Some("fill_style_none"),
        "纯色填充" => Some("fill_style_solid"),
        "蜡笔" => Some("fill_style_crayon"),
        "记号笔" => Some("fill_style_marker"),
        "油画笔" => Some("fill_style_oil"),
        "普通铅笔" => Some("fill_style_pencil"),
        "水彩画笔" => Some("fill_style_watercolor"),
        _ => None,
    }
}

/// Maps a shape stroke width leaf to its adapter-pack target id.
///
/// ADR-0098 D3: every entry here is anchored by the measured `AutomationId`
/// (`Size1` / `Size3` / `Size5` / `Size8`), never by the localized menu label.
pub(super) fn size_target(leaf: &str) -> Option<&'static str> {
    match leaf {
        "1 像素" => Some("size_1px_option"),
        "3 像素" => Some("size_3px_option"),
        "5 像素" => Some("size_5px_option"),
        "8 像素" => Some("size_8px_option"),
        _ => None,
    }
}

pub(super) fn required_selection_path(
    arguments: &Map<String, Value>,
    tool: &str,
    vocabulary: &[&str],
    max_depth: usize,
) -> Result<Vec<String>, ToolBusError> {
    let values = arguments
        .get("path")
        .and_then(Value::as_array)
        .ok_or_else(|| support::invalid_arguments(tool, "path must be an array"))?;
    if values.is_empty() {
        return Err(support::invalid_arguments(
            tool,
            "path must contain at least one element",
        ));
    }
    if values.len() > max_depth {
        return Err(support::invalid_arguments(
            tool,
            format!("path must contain at most {max_depth} elements"),
        ));
    }
    let mut path = Vec::with_capacity(values.len());
    for value in values {
        let element = value.as_str().ok_or_else(|| {
            support::invalid_arguments(tool, "every path element must be a string")
        })?;
        if !vocabulary.contains(&element) {
            return Err(support::invalid_arguments(
                tool,
                format!("path element `{element}` is not in this group's vocabulary"),
            ));
        }
        path.push(element.to_owned());
    }
    Ok(path)
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

/// Validates and returns an absolute `.png` path for the file-dialog handlers.
///
/// `require_existing_file` distinguishes the open contract (source must already
/// be a regular file) from save-as (the target must not exist yet, while its
/// parent directory must). The handler repeats the no-overwrite check because
/// this function intentionally owns only shape validation.
pub(super) fn required_png_path(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
    require_existing_file: bool,
) -> Result<PathBuf, ToolBusError> {
    let raw = required_string(arguments, field, tool)?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(support::invalid_arguments(
            tool,
            format!("{field} must be an absolute path"),
        ));
    }
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
    {
        return Err(support::invalid_arguments(
            tool,
            format!("{field} must end in `.png`"),
        ));
    }
    if require_existing_file {
        if !path.is_file() {
            return Err(support::invalid_arguments(
                tool,
                format!("{field} must name an existing regular file"),
            ));
        }
    } else {
        let parent = path.parent().ok_or_else(|| {
            support::invalid_arguments(tool, format!("{field} must have a parent directory"))
        })?;
        if !parent.is_dir() {
            return Err(support::invalid_arguments(
                tool,
                format!("{field} parent directory does not exist"),
            ));
        }
    }
    Ok(path)
}

/// Reads the PNG signature and `IHDR` width / height without a decoder
/// dependency.
///
/// The task schema permits only PNG files, and both open / save-as need a
/// postcondition that proves the image is structurally present. Reading the
/// fixed 24-byte prefix is enough to reject non-PNG data and capture the
/// dimensions without loading the pixel payload into memory.
pub(super) fn png_dimensions(path: &Path, tool: &str) -> Result<(u32, u32), ToolBusError> {
    const SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    let mut file = File::open(path).map_err(|error| {
        support::invalid_arguments(tool, format!("cannot open `{}`: {error}", path.display()))
    })?;
    let mut header = [0_u8; 24];
    file.read_exact(&mut header).map_err(|error| {
        support::invalid_arguments(tool, format!("cannot read `{}`: {error}", path.display()))
    })?;
    if header.get(..8) != Some(SIGNATURE.as_slice())
        || header.get(8..12) != Some(13_u32.to_be_bytes().as_slice())
        || header.get(12..16) != Some(b"IHDR".as_slice())
    {
        return Err(support::invalid_arguments(
            tool,
            format!("`{}` is not a PNG with an IHDR header", path.display()),
        ));
    }
    let width = u32::from_be_bytes(
        header
            .get(16..20)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| support::invalid_arguments(tool, "PNG width is truncated"))?,
    );
    let height = u32::from_be_bytes(
        header
            .get(20..24)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| support::invalid_arguments(tool, "PNG height is truncated"))?,
    );
    if width == 0 || height == 0 {
        return Err(support::invalid_arguments(
            tool,
            "PNG width and height must be positive",
        ));
    }
    Ok((width, height))
}

pub(super) const fn verify_failed(message: String) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_004,
        message,
    }
}

/// Fail-closed error for a user-driven dialog state (cancel, existing target,
/// or other action the user must resolve).
pub(super) fn user_interaction(tool: &str) -> ToolBusError {
    user_interaction_with(tool, "user interaction is required before retrying")
}

/// Same as [`user_interaction`], with a path-specific reason for diagnosis.
pub(super) fn user_interaction_with(tool: &str, reason: &str) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_010,
        message: format!("{tool}: {reason}"),
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
