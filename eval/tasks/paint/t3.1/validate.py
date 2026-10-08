"""Static validation for the Paint T3.1 task and evaluation declarations."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
TASK_DIR = ROOT / "adapters" / "com.microsoft.paint" / "tasks"
EVAL_DIR = Path(__file__).resolve().parent


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise AssertionError(f"{path} must contain a JSON object")
    return value


def main() -> None:
    task = load_json(TASK_DIR / "t3.1.new-canvas-rectangle-color-screenshot.json")
    cases = load_json(EVAL_DIR / "cases.json")
    expected = load_json(EVAL_DIR / "expected.json")
    probe = load_json(EVAL_DIR / "probe-evidence.json")

    assert task["schema_version"] == 1
    assert cases["schema_version"] == 1
    assert expected["schema_version"] == 1
    assert probe["schema_version"] == 1
    assert task["task_id"] == cases["task_id"] == expected["task_id"]
    assert task["app_id"] == probe["app_id"] == "com.microsoft.paint"
    assert task["read_only"] is False
    assert task["metrics"]["repeat_runs"] == 10
    assert task["metrics"]["minimum_success_rate"] == 0.75
    assert task["metrics"]["coordinate_error_max_px"] == 2
    assert task["metrics"]["silent_failure_rate"] == 0
    assert task["inputs"]["tolerance_px"]["maximum"] == 2

    tools = [step.get("tool") for step in task["steps"] if step["kind"] == "tool"]
    assert tools.count("paint.canvas.resolve_point") == 2, tools
    assert "paint.canvas.draw_rectangle" in tools, tools
    assert any(
        postcondition["id"] == "coordinate_error_within_tolerance"
        for postcondition in task["postconditions"]
    )
    assert any(
        postcondition["id"] == "rectangle_visual_assert"
        for postcondition in task["postconditions"]
    )

    case_ids = [case["id"] for case in cases["cases"]]
    assert len(case_ids) == 10, case_ids
    assert len(set(case_ids)) == len(case_ids), case_ids
    assert cases["repeat_runs"] == 10
    assert cases["primary_case"] in case_ids
    assert set(case_ids) == set(expected["cases"])
    assert expected["real_run_status"] == "not_run"

    for case in cases["cases"]:
        assert case["coordinate_space"]["kind"] == "physical_pixels"
        assert case["rectangle_start"]["x"] < case["rectangle_end"]["x"]
        assert case["rectangle_start"]["y"] < case["rectangle_end"]["y"]
        expected_case = expected["cases"][case["id"]]
        assert expected_case["expected_tool"] == "rectangle"
        assert re.fullmatch(r"\d{1,3},\d{1,3},\d{1,3}", expected_case["foreground_rgb"])
        assert expected_case["coordinate_error_max_px"] <= 2

    canvas = next(control for control in probe["controls"] if control["target"] == "canvas")
    assert canvas["automation_id"] == "image"
    assert canvas["observed_zoom_ratio"] == 0.5
    assert probe["observed_coordinate_transform"]["canvas_origin_screen_px"] == {"x": 696, "y": 525}

    # 2026-10-08 measured selector-resolution facts (real Paint window via the
    # repository's own WindowsPlatform). These lock in *why* the provisional
    # pack blocks TASK-044's real ten-run acceptance.
    resolution = probe["selector_resolution_probe"]
    results = resolution["results"]
    declared_main = next(
        entry
        for entry in results
        if entry["target_id"] == "main_window" and entry["candidate"].startswith("declared")
    )
    assert declared_main["outcome"] == "TargetNotFound", declared_main
    measured_main = next(
        entry
        for entry in results
        if entry["target_id"] == "main_window" and entry["candidate"].startswith("measured")
    )
    assert measured_main["outcome"] == "resolved", measured_main
    declared_canvas = next(
        entry
        for entry in results
        if entry["target_id"] == "canvas" and entry["candidate"].startswith("declared")
    )
    assert declared_canvas["outcome"] == "TargetAmbiguous", declared_canvas
    measured_canvas = next(
        entry
        for entry in results
        if entry["target_id"] == "canvas" and entry["candidate"].startswith("measured")
    )
    assert measured_canvas["outcome"] == "resolved", measured_canvas
    assert "209x37" in measured_canvas["detail"], measured_canvas
    assert "outside TASK-044's write scope" in resolution["conclusion"], resolution

    rectangle = next(
        control for control in probe["controls"] if control["target"] == "rectangle_shape"
    )
    assert rectangle["automation_id"] == ""
    assert rectangle["fallback_only"] is True
    assert rectangle["locale_dependent"] is True

    for control in probe["controls"]:
        if control["locale_dependent"]:
            assert control["fallback_only"] is True, control

    print(
        "paint_t3_1_static_validation_ok "
        f"cases={len(case_ids)} tolerance_px={expected['coordinate_error_max_px']} "
        "probe=canvas_image_zoom_0.5"
    )


if __name__ == "__main__":
    main()
