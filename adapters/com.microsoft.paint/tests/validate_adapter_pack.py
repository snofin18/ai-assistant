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
            if kind in {"a11y_path", "title_regex", "name_regex", "exact_name", "visual_anchor"}:
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

    exact_name_targets = {
        "rectangle_tool_button": ("shape-gallery-exact-name-fallback", "形状"),
        "foreground_color_button": ("color-gallery-exact-name-fallback", "颜色"),
    }
    targets_by_id = {target.get("id"): target for target in targets}
    for target_id, (candidate_id, expected_name) in exact_name_targets.items():
        target = targets_by_id.get(target_id, {})
        candidates = {candidate.get("id"): candidate for candidate in target.get("candidates", [])}
        exact_name = candidates.get(candidate_id, {})
        check(exact_name.get("kind") == "exact_name",
              f"selector target {target_id}: {candidate_id} must use exact_name")
        check(exact_name.get("value", {}).get("text") == expected_name,
              f"selector target {target_id}: {candidate_id} must use exact text {expected_name!r}")
        check(exact_name.get("locale_dependent") is True,
              f"selector target {target_id}: {candidate_id} must be locale-dependent")
        check(exact_name.get("fallback_only") is True,
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
