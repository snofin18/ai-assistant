#!/usr/bin/env python3
"""Validate the declarative Microsoft Paint adapter pack."""

from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


PACK_ROOT = Path(__file__).resolve().parents[1]
ERRORS: list[str] = []


def check(condition: bool, message: str) -> None:
    """Record a failed contract check without raising."""
    if not condition:
        ERRORS.append(message)


def read_json(relative_path: str) -> dict[str, Any]:
    """Read one JSON object from the adapter pack."""
    path = PACK_ROOT / relative_path
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        ERRORS.append(f"{relative_path}: cannot parse JSON: {error}")
        return {}
    check(isinstance(value, dict), f"{relative_path}: root must be an object")
    return value if isinstance(value, dict) else {}


def main() -> int:
    """Run all adapter-pack checks and print a single verdict."""
    adapter = tomllib.loads((PACK_ROOT / "adapter.toml").read_text(encoding="utf-8"))
    app_id = adapter.get("adapter", {}).get("app_id")
    check(app_id == "com.microsoft.paint", "adapter.toml: app_id mismatch")

    selectors = read_json("selectors/targets.json")
    tools_document = read_json("tools/tools.json")
    rollback = read_json("rollback/recipes.json")
    interrupts = read_json("interrupts/interrupts.json")
    memory = read_json("memory/app_map.v1.json")
    app_map = read_json("app_map.json")

    for label, document in (
        ("selectors/targets.json", selectors),
        ("tools/tools.json", tools_document),
        ("rollback/recipes.json", rollback),
        ("interrupts/interrupts.json", interrupts),
        ("memory/app_map.v1.json", memory),
        ("app_map.json", app_map),
    ):
        if document:
            check(document.get("app_id") == app_id, f"{label}: app_id mismatch")

    check(selectors.get("probe_status", {}).get("status") == "required",
          "selectors/targets.json: whole-pack probe_status must be required")
    check(tools_document.get("probe_status") == "required",
          "tools/tools.json: probe_status must be required")

    targets = selectors.get("targets", [])
    check(isinstance(targets, list) and targets, "selectors/targets.json: targets missing")
    target_ids: set[str] = set()
    for target in targets:
        target_id = target.get("id")
        check(isinstance(target_id, str) and target_id, "selector target id missing")
        check(target_id not in target_ids, f"selector target id duplicated: {target_id}")
        target_ids.add(target_id)
        probe_status = target.get("probe_status")
        check(probe_status in {"required", "observed"},
              f"selector target {target_id}: unsupported probe_status {probe_status}")
        # ADR-0095 D3：整条链只有 fallback 候选时，必须是**显式声明**的入口链。
        # 光靠"全部候选都是兜底"不能当主选择器用，所以这里把声明变成机器门禁。
        target_candidates = target.get("candidates") or []
        if target_candidates and all(
            candidate.get("fallback_only") is True for candidate in target_candidates
        ):
            check(target.get("allow_fallback_entry") is True,
                  f"selector target {target_id}: every candidate is fallback_only, so the target "
                  f"must declare allow_fallback_entry: true (ADR-0095 D3)")
        candidates = target.get("candidates", [])
        check(candidates, f"selector target {target_id}: candidates missing")
        candidate_ids: set[str] = set()
        for candidate in candidates:
            candidate_id = candidate.get("id")
            kind = candidate.get("kind")
            score = candidate.get("score")
            check(candidate_id, f"selector target {target_id}: candidate id missing")
            check(candidate_id not in candidate_ids,
                  f"selector target {target_id}: duplicate candidate id {candidate_id}")
            candidate_ids.add(candidate_id)
            check(isinstance(score, (int, float)) and 0.0 <= float(score) <= 1.0,
                  f"selector target {target_id}: bad score on {candidate_id}")
            check(kind != "runtime_id",
                  f"selector target {target_id}: runtime_id must not be declarative")
            if kind in {
                "a11y_path",
                "title_regex",
                "name_regex",
                "exact_name",
                "exact_name_and_role",
                "visual_anchor",
            }:
                check(candidate.get("locale_dependent") is True,
                      f"selector target {target_id}: {kind} must be locale-dependent")
                if probe_status == "observed":
                    check(candidate.get("fallback_only") is True,
                          f"selector target {target_id}: {kind} must be fallback-only")
            elif probe_status == "observed":
                check(candidate.get("fallback_only") is not True,
                      f"selector target {target_id}: {kind} must not be fallback-only")
            if kind == "role_and_parent":
                parent_id = candidate.get("value", {}).get("role_and_parent", {}).get("parent_id")
                check(parent_id in candidate_ids,
                      f"selector target {target_id}: unknown parent id {parent_id}")

    # ADR-0091: the shape item and the colour palette container both carry a localized
    # accessible name, so they are pinned as name + non-localized role pairs. The role
    # half is what removes the "container and its own DropDownButton share the name"
    # ambiguity that plain `exact_name` could not (DRIFT-256-3).
    localized_fallbacks = {
        "rectangle_tool_button": (
            "rectangle-item-exact-name-and-role",
            "矩形",
            "Button",
        ),
        "foreground_color_button": (
            "color-gallery-exact-name-and-role-fallback",
            "颜色",
            "Group",
        ),
    }
    targets_by_id = {target.get("id"): target for target in targets}
    for target_id, (candidate_id, expected_name, expected_role) in localized_fallbacks.items():
        target = targets_by_id.get(target_id, {})
        candidates = {candidate.get("id"): candidate for candidate in target.get("candidates", [])}
        candidate = candidates.get(candidate_id, {})
        check(candidate.get("kind") == "exact_name_and_role",
              f"selector target {target_id}: {candidate_id} must use exact_name_and_role")
        value = candidate.get("value", {}).get("exact_name_and_role", {})
        check(value.get("name") == expected_name,
              f"selector target {target_id}: {candidate_id} must use exact name {expected_name!r}")
        check(value.get("role") == expected_role,
              f"selector target {target_id}: {candidate_id} must use non-localized role "
              f"{expected_role!r}")
        check(candidate.get("locale_dependent") is True,
              f"selector target {target_id}: {candidate_id} must be locale-dependent")
        check(candidate.get("fallback_only") is True,
              f"selector target {target_id}: {candidate_id} must be fallback-only")

    # TASK-309 real-machine probe (docs/memory/apps/paint.md §13.14 / §13.16): the four
    # shape-size menu items are the only Paint targets measured to carry a stable
    # AutomationId, so they must stay pinned to `automation_id` values. The first item is
    # named `大小` (the flyout's own title), so a name-based candidate would be wrong here.
    measured_size_options = {
        "size_1px_option": ("size-1px-option-automation-id", "Size1"),
        "size_3px_option": ("size-3px-option-automation-id", "Size3"),
        "size_5px_option": ("size-5px-option-automation-id", "Size5"),
        "size_8px_option": ("size-8px-option-automation-id", "Size8"),
    }
    for target_id, (candidate_id, expected_aid) in measured_size_options.items():
        target = targets_by_id.get(target_id, {})
        check(target.get("probe_status") == "observed",
              f"selector target {target_id}: must stay observed after the real-machine probe")
        candidates = {candidate.get("id"): candidate for candidate in target.get("candidates", [])}
        candidate = candidates.get(candidate_id, {})
        check(candidate.get("kind") == "automation_id",
              f"selector target {target_id}: {candidate_id} must use automation_id")
        check(candidate.get("value", {}).get("text") == expected_aid,
              f"selector target {target_id}: {candidate_id} must use AutomationId "
              f"{expected_aid!r}")
        check(candidate.get("locale_dependent") is False,
              f"selector target {target_id}: {candidate_id} must not be locale-dependent")
        check(candidate.get("fallback_only") is not True,
              f"selector target {target_id}: {candidate_id} must not be fallback-only")

    # ADR-0096: ribbon groups have group-local vocabularies. These targets were
    # observed in the ControlView probe and must stay pinned to measured names.
    measured_group_targets = {
        "text_tool_button": ("text-tool-exact-name-and-role-fallback", "文本", "Button"),
        "color_picker_tool_button": (
            "color-picker-exact-name-and-role-fallback",
            "颜色选取器",
            "Button",
        ),
        "magnifier_tool_button": (
            "magnifier-exact-name-and-role-fallback",
            "放大镜",
            "Button",
        ),
        "shape_line_button": ("shape-line-exact-name-and-role-fallback", "直线", "Button"),
        "shape_curve_button": ("shape-curve-exact-name-and-role-fallback", "曲线", "Button"),
        "shape_ellipse_button": ("shape-ellipse-exact-name-and-role-fallback", "椭圆", "Button"),
        "shape_rounded_rectangle_button": (
            "shape-rounded-rectangle-exact-name-and-role-fallback",
            "圆角矩形",
            "Button",
        ),
        "shape_polygon_button": ("shape-polygon-exact-name-and-role-fallback", "多边形", "Button"),
        "shape_triangle_button": (
            "shape-triangle-exact-name-and-role-fallback",
            "三角形",
            "Button",
        ),
        "shape_right_triangle_button": (
            "shape-right-triangle-exact-name-and-role-fallback",
            "直角三角形",
            "Button",
        ),
        "shape_diamond_button": ("shape-diamond-exact-name-and-role-fallback", "菱形", "Button"),
        "shape_pentagon_button": ("shape-pentagon-exact-name-and-role-fallback", "五边形", "Button"),
        "shape_hexagon_button": ("shape-hexagon-exact-name-and-role-fallback", "六边形", "Button"),
        "shape_right_arrow_button": (
            "shape-right-arrow-exact-name-and-role-fallback",
            "向右键",
            "Button",
        ),
        "shape_left_arrow_button": (
            "shape-left-arrow-exact-name-and-role-fallback",
            "向左键",
            "Button",
        ),
        "shape_up_arrow_button": (
            "shape-up-arrow-exact-name-and-role-fallback",
            "向上箭头",
            "Button",
        ),
        "shape_down_arrow_button": (
            "shape-down-arrow-exact-name-and-role-fallback",
            "向下箭头",
            "Button",
        ),
        "shape_four_point_star_button": (
            "shape-four-point-star-exact-name-and-role-fallback",
            "四角星",
            "Button",
        ),
        "shape_five_point_star_button": (
            "shape-five-point-star-exact-name-and-role-fallback",
            "五角星",
            "Button",
        ),
        "shape_six_point_star_button": (
            "shape-six-point-star-exact-name-and-role-fallback",
            "六角星",
            "Button",
        ),
        "shape_rounded_rectangle_callout_button": (
            "shape-rounded-rectangle-callout-exact-name-and-role-fallback",
            "圆角矩形标注",
            "Button",
        ),
        "shape_oval_callout_button": (
            "shape-oval-callout-exact-name-and-role-fallback",
            "椭圆形标注",
            "Button",
        ),
        "shape_cloud_callout_button": (
            "shape-cloud-callout-exact-name-and-role-fallback",
            "云形标注",
            "Button",
        ),
        "shape_heart_button": ("shape-heart-exact-name-and-role-fallback", "心形", "Button"),
        "shape_lightning_button": (
            "shape-lightning-exact-name-and-role-fallback",
            "闪电",
            "Button",
        ),
        "brush_style_brush": (
            "brush-style-brush-exact-name-and-role-fallback",
            "画笔",
            "MenuItem",
        ),
        "brush_style_calligraphy": (
            "brush-style-calligraphy-exact-name-and-role-fallback",
            "毛笔",
            "MenuItem",
        ),
        "brush_style_marker": (
            "brush-style-marker-exact-name-and-role-fallback",
            "书写笔",
            "MenuItem",
        ),
        "brush_style_airbrush": (
            "brush-style-airbrush-exact-name-and-role-fallback",
            "喷枪",
            "MenuItem",
        ),
        "brush_style_oil": (
            "brush-style-oil-exact-name-and-role-fallback",
            "油画笔",
            "MenuItem",
        ),
        "brush_style_crayon": (
            "brush-style-crayon-exact-name-and-role-fallback",
            "蜡笔",
            "MenuItem",
        ),
        "brush_style_highlighter": (
            "brush-style-highlighter-exact-name-and-role-fallback",
            "记号笔",
            "MenuItem",
        ),
        "brush_style_pencil": (
            "brush-style-pencil-exact-name-and-role-fallback",
            "普通铅笔",
            "MenuItem",
        ),
        "brush_style_watercolor": (
            "brush-style-watercolor-exact-name-and-role-fallback",
            "水彩画笔",
            "MenuItem",
        ),
        # ADR-0097: the shape-outline / shape-fill flyouts also only expose localized
        # names, so the two buttons and their 2 x 7 items are pinned the same way.
        "shape_outline_button": (
            "shape-outline-exact-name-and-role-fallback",
            "形状轮廓",
            "Button",
        ),
        "shape_fill_button": (
            "shape-fill-exact-name-and-role-fallback",
            "形状填充",
            "Button",
        ),
        "outline_style_none": (
            "outline-style-none-exact-name-and-role-fallback",
            "无边框",
            "MenuItem",
        ),
        "outline_style_solid": (
            "outline-style-solid-exact-name-and-role-fallback",
            "实心边框",
            "MenuItem",
        ),
        "outline_style_crayon": (
            "outline-style-crayon-exact-name-and-role-fallback",
            "蜡笔",
            "MenuItem",
        ),
        "outline_style_marker": (
            "outline-style-marker-exact-name-and-role-fallback",
            "记号笔",
            "MenuItem",
        ),
        "outline_style_oil": (
            "outline-style-oil-exact-name-and-role-fallback",
            "油画笔",
            "MenuItem",
        ),
        "outline_style_pencil": (
            "outline-style-pencil-exact-name-and-role-fallback",
            "普通铅笔",
            "MenuItem",
        ),
        "outline_style_watercolor": (
            "outline-style-watercolor-exact-name-and-role-fallback",
            "水彩画笔",
            "MenuItem",
        ),
        "fill_style_none": (
            "fill-style-none-exact-name-and-role-fallback",
            "无填充",
            "MenuItem",
        ),
        "fill_style_solid": (
            "fill-style-solid-exact-name-and-role-fallback",
            "纯色填充",
            "MenuItem",
        ),
        "fill_style_crayon": (
            "fill-style-crayon-exact-name-and-role-fallback",
            "蜡笔",
            "MenuItem",
        ),
        "fill_style_marker": (
            "fill-style-marker-exact-name-and-role-fallback",
            "记号笔",
            "MenuItem",
        ),
        "fill_style_oil": (
            "fill-style-oil-exact-name-and-role-fallback",
            "油画笔",
            "MenuItem",
        ),
        "fill_style_pencil": (
            "fill-style-pencil-exact-name-and-role-fallback",
            "普通铅笔",
            "MenuItem",
        ),
        "fill_style_watercolor": (
            "fill-style-watercolor-exact-name-and-role-fallback",
            "水彩画笔",
            "MenuItem",
        ),
    }
    for target_id, (candidate_id, expected_name, expected_role) in measured_group_targets.items():
        target = targets_by_id.get(target_id, {})
        check(target.get("probe_status") == "observed",
              f"selector target {target_id}: must stay observed after the ribbon probe")
        check(target.get("allow_fallback_entry") is True,
              f"selector target {target_id}: localized name-only target must allow fallback entry")
        candidates = {candidate.get("id"): candidate for candidate in target.get("candidates", [])}
        candidate = candidates.get(candidate_id, {})
        check(candidate.get("kind") == "exact_name_and_role",
              f"selector target {target_id}: {candidate_id} must use exact_name_and_role")
        value = candidate.get("value", {}).get("exact_name_and_role", {})
        check(value.get("name") == expected_name,
              f"selector target {target_id}: {candidate_id} must use exact name {expected_name!r}")
        check(value.get("role") == expected_role,
              f"selector target {target_id}: {candidate_id} must use role {expected_role!r}")
        check(candidate.get("fallback_only") is True,
              f"selector target {target_id}: {candidate_id} must be fallback-only")

    tools = tools_document.get("tools", [])
    check(isinstance(tools, list) and tools, "tools/tools.json: tools missing")
    tool_names: set[str] = set()
    tool_by_name: dict[str, dict[str, Any]] = {}
    for entry in tools:
        schema = entry.get("schema", {})
        name = schema.get("name")
        check(isinstance(name, str) and re.fullmatch(
            r"paint\.[a-z0-9_]+\.[a-z0-9_]+", name or ""),
            f"tool name is not paint.<domain>.<action>: {name}")
        check(name not in tool_names, f"tool name duplicated: {name}")
        tool_names.add(name)
        tool_by_name[name] = entry
        check(schema.get("effect") in {"read", "write", "navigate", "launch", "delete", "send", "configure"},
              f"tool {name}: invalid effect")
        check(schema.get("reversibility") in {
            "l0_undo_stack", "l1_snapshot", "l2_compensation",
            "l3_irreversible", "none_readonly"
        }, f"tool {name}: invalid reversibility")
        check(schema.get("risk_level") in {"low", "medium", "high", "critical"},
              f"tool {name}: invalid risk level")
        if schema.get("risk_level") == "high":
            check(schema.get("requires_approval") is True,
                  f"tool {name}: high risk requires approval")
        check(isinstance(schema.get("input"), dict), f"tool {name}: input schema missing")
        check(isinstance(schema.get("output"), dict), f"tool {name}: output schema missing")
        input_properties = set(schema.get("input", {}).get("properties", {}))
        output_properties = set(schema.get("output", {}).get("properties", {}))
        postconditions = entry.get("postconditions", [])
        check(postconditions, f"tool {name}: at least one postcondition required")
        for postcondition in postconditions:
            check("assert" not in postcondition, f"tool {name}: free-form assert is forbidden")
            check(postcondition.get("kind"), f"tool {name}: postcondition kind missing")
            check(postcondition.get("error_code"), f"tool {name}: postcondition error_code missing")
            postcondition_name = postcondition.get("name")
            if postcondition.get("kind") in {"value_equals", "value_in_range"} and postcondition_name:
                check(postcondition_name in output_properties,
                      f"tool {name}: postcondition references missing output field {postcondition_name}")
            for token_name in re.findall(r"\{\{([a-z0-9_]+)\}\}", str(postcondition.get("value", ""))):
                check(token_name in input_properties or token_name in output_properties,
                      f"tool {name}: postcondition token {token_name} is not declared")
        for input_name in input_properties:
            if input_name.startswith("expected_"):
                token = "{{" + input_name + "}}"
                check(any(token in str(postcondition.get("value", ""))
                          for postcondition in postconditions),
                      f"tool {name}: input {input_name} is not read back in a postcondition")

    recipes = rollback.get("recipes", [])
    check(recipes, "rollback/recipes.json: recipes missing")
    for recipe in recipes:
        check(recipe.get("tool") in tool_names,
              f"rollback recipe {recipe.get('id')}: unknown tool {recipe.get('tool')}")

    draw_entry = tool_by_name.get("paint.canvas.draw_rectangle", {})
    draw_postconditions = draw_entry.get("postconditions", [])
    check(any(item.get("kind") == "visual_assert" for item in draw_postconditions),
          "paint.canvas.draw_rectangle: visual_assert postcondition required")
    check(any(item.get("kind") == "state_changed" for item in draw_postconditions),
          "paint.canvas.draw_rectangle: state_changed postcondition required")
    draw_recipe = next(
        (recipe for recipe in recipes if recipe.get("id") == "draw-rectangle-l0-l1"),
        {},
    )
    levels = {item.get("level") for item in draw_recipe.get("anchor_strategy", [])}
    check({"L0", "L1"}.issubset(levels), "rollback draw-rectangle-l0-l1: L0 and L1 required")

    coordinate_tool = tool_by_name.get("paint.canvas.resolve_point", {})
    coordinate_properties = set(
        coordinate_tool.get("schema", {}).get("input", {}).get("properties", {})
    )
    required_coordinate_fields = {
        "canvas_x", "canvas_y", "zoom_ratio",
        "viewport_offset_x_px", "viewport_offset_y_px", "coordinate_space"
    }
    check(required_coordinate_fields.issubset(coordinate_properties),
          "paint.canvas.resolve_point: coordinate transform fields missing")

    unsaved = next(
        (item for item in interrupts.get("interrupts", []) if item.get("id") == "unsaved-canvas-dialog"),
        {},
    )
    check(unsaved.get("default_action") == "cancel",
          "unsaved-canvas-dialog: default_action must be cancel")
    forbidden_defaults = set(unsaved.get("forbidden_defaults", []))
    check({"save", "discard"}.issubset(forbidden_defaults),
          "unsaved-canvas-dialog: save and discard must be forbidden defaults")

    check(app_map.get("identity", {}).get("aumid") == adapter.get("adapter", {}).get("aumid"),
          "app_map.json: AUMID does not match adapter.toml")
    check(app_map.get("evidence_status", {}).get("selector_ids") == "provisional",
          "app_map.json: selector evidence must remain provisional")
    check(isinstance(memory.get("entries"), list) and memory.get("entries"),
          "memory/app_map.v1.json: entries missing")

    if ERRORS:
        for error in ERRORS:
            print(f"FAILED: {error}")
        print(f"verdict: FAILED ({len(ERRORS)} error(s))")
        return 1

    print(
        "PASSED: "
        f"app_id={app_id} "
        f"targets={len(targets)} "
        f"tools={len(tools)} "
        f"recipes={len(recipes)} "
        "probe_status=required"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
