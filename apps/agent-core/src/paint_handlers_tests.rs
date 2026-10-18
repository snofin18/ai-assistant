// Paint handler unit tests.
//
// Kept in a separate file so `paint_handlers.rs` stays under the repository
// line guides (gov section 5.4); the test module is mounted from that file.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::paint_handler_support::{
    BRUSH_GROUP_VOCABULARY, FILL_GROUP_VOCABULARY, OUTLINE_GROUP_VOCABULARY,
    SHAPE_GROUP_VOCABULARY, TOOL_GROUP_VOCABULARY, brush_target, fill_target, outline_target,
    parse_canvas_size, required_selection_path, rounded_i32, shape_target, tool_target,
};
use super::{paint_layer_contract, palette_index};
use assistant_protocol::serde_json::json;

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

#[test]
fn test_selection_paths_are_group_local_and_bounded() {
    let tool_arguments = json!({"path": ["铅笔"]});
    let tool_arguments = tool_arguments.as_object().expect("object");
    assert_eq!(
        required_selection_path(
            tool_arguments,
            "paint.tool.select",
            TOOL_GROUP_VOCABULARY,
            1,
        )
        .expect("tool path"),
        vec!["铅笔"]
    );
    assert!(tool_target("铅笔").is_some());

    let shape_arguments = json!({"path": ["矩形"]});
    assert_eq!(
        required_selection_path(
            shape_arguments.as_object().expect("object"),
            "paint.shape.select",
            SHAPE_GROUP_VOCABULARY,
            1,
        )
        .expect("shape path"),
        vec!["矩形"]
    );
    assert_eq!(shape_target("矩形"), Some(super::TARGET_RECTANGLE_TOOL));

    let brush_arguments = json!({"path": ["毛笔"]});
    assert_eq!(
        required_selection_path(
            brush_arguments.as_object().expect("object"),
            "paint.brush.select",
            BRUSH_GROUP_VOCABULARY,
            1,
        )
        .expect("brush path"),
        vec!["毛笔"]
    );
    assert!(brush_target("毛笔").is_some());
}

#[test]
fn test_selection_path_rejects_cross_group_and_over_depth_values() {
    let cross_group = json!({"path": ["长方形"]});
    assert!(
        required_selection_path(
            cross_group.as_object().expect("object"),
            "paint.shape.select",
            SHAPE_GROUP_VOCABULARY,
            1,
        )
        .is_err()
    );
    let too_deep = json!({"path": ["形状", "矩形"]});
    assert!(
        required_selection_path(
            too_deep.as_object().expect("object"),
            "paint.shape.select",
            SHAPE_GROUP_VOCABULARY,
            1,
        )
        .is_err()
    );
    let empty = json!({"path": []});
    assert!(
        required_selection_path(
            empty.as_object().expect("object"),
            "paint.tool.select",
            TOOL_GROUP_VOCABULARY,
            1,
        )
        .is_err()
    );
}

/// ADR-0097 D3: the five names shared by the brush / outline / fill groups must
/// each resolve to their own target, never to a neighbouring group's item.
#[test]
fn test_shape_group_flyout_vocabularies_stay_group_local_for_shared_names() {
    let shared = "蜡笔";
    assert_eq!(brush_target(shared), Some("brush_style_crayon"));
    assert_eq!(outline_target(shared), Some("outline_style_crayon"));
    assert_eq!(fill_target(shared), Some("fill_style_crayon"));

    let outline_arguments = json!({"path": ["无边框"]});
    assert_eq!(
        required_selection_path(
            outline_arguments.as_object().expect("object"),
            "paint.outline.select",
            OUTLINE_GROUP_VOCABULARY,
            1,
        )
        .expect("outline path"),
        vec!["无边框"]
    );
    let fill_arguments = json!({"path": ["纯色填充"]});
    assert_eq!(
        required_selection_path(
            fill_arguments.as_object().expect("object"),
            "paint.fill.select",
            FILL_GROUP_VOCABULARY,
            1,
        )
        .expect("fill path"),
        vec!["纯色填充"]
    );
}

/// The outline and fill vocabularies do not overlap: `无填充` is not an outline
/// item and `无边框` is not a fill item.
#[test]
fn test_shape_group_flyout_paths_reject_the_other_groups_terms() {
    let fill_term_in_outline = json!({"path": ["无填充"]});
    assert!(
        required_selection_path(
            fill_term_in_outline.as_object().expect("object"),
            "paint.outline.select",
            OUTLINE_GROUP_VOCABULARY,
            1,
        )
        .is_err()
    );
    let outline_term_in_fill = json!({"path": ["无边框"]});
    assert!(
        required_selection_path(
            outline_term_in_fill.as_object().expect("object"),
            "paint.fill.select",
            FILL_GROUP_VOCABULARY,
            1,
        )
        .is_err()
    );
    assert_eq!(outline_target("无填充"), None);
    assert_eq!(fill_target("无边框"), None);
}
