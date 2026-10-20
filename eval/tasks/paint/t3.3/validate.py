#!/usr/bin/env python3
"""Static validation for the Paint T3.3 draw/undo contract."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    expected = _load(HERE / "expected.json")
    cases = _load(HERE / "cases.json")
    summary = _load(HERE / "ten-run-summary.json")
    task = _load(
        ROOT
        / "adapters/com.microsoft.paint/tasks/t3.3.open-png-draw-rollback.json"
    )
    tools = _load(ROOT / "adapters/com.microsoft.paint/tools/tools.json")["tools"]

    if task["task_id"] != expected["task_id"]:
        raise SystemExit("task package and expected task_id differ")
    if cases["task_id"] != expected["task_id"]:
        raise SystemExit("cases and expected task_id differ")
    if summary["task_id"] != expected["task_id"]:
        raise SystemExit("ten-run summary and expected task_id differ")

    declared_tools = {
        entry["schema"]["name"]
        for entry in tools
        if isinstance(entry, dict) and "schema" in entry
    }
    steps = task["steps"]
    task_tools = {
        step["tool"]
        for step in steps
        if step.get("kind") == "tool" and "tool" in step
    }
    missing = sorted(task_tools - declared_tools)
    if missing:
        raise SystemExit(f"task references undeclared tools: {missing}")

    required_tools = {
        "paint.file.open",
        "paint.canvas.capture_pixels",
        "paint.canvas.draw_rectangle",
        "paint.canvas.rollback_last_write",
    }
    missing_required = sorted(required_tools - task_tools)
    if missing_required:
        raise SystemExit(f"T3.3 is missing required tools: {missing_required}")

    step_by_id = {step["id"]: step for step in steps}
    open_step = step_by_id["open_png"]
    capture_step = step_by_id["capture_pre_snapshot"]
    draw_step = step_by_id["draw_rectangle"]
    rollback_step = step_by_id["rollback_last_write"]

    if open_step.get("args", {}).get("discard_unsaved_confirmed") is not False:
        raise SystemExit("paint.file.open must explicitly refuse unsaved discard")
    if capture_step.get("tool") != "paint.canvas.capture_pixels":
        raise SystemExit("pre snapshot must use paint.canvas.capture_pixels")
    capture_index = steps.index(capture_step)
    if capture_index >= steps.index(draw_step):
        raise SystemExit("pre snapshot must be captured before drawing")
    if draw_step.get("args", {}).get("pre_snapshot_blob_id") != "$pre_snapshot_blob_id":
        raise SystemExit("draw_rectangle must use the pre-write snapshot")
    rollback_args = rollback_step.get("args", {})
    if rollback_args.get("pre_snapshot_blob_id") != "$pre_snapshot_blob_id":
        raise SystemExit("rollback must compare against the real pre-write snapshot")
    if (
        rollback_args.get("expected_undo_steps")
        != "$input.expected_undo_steps"
    ):
        raise SystemExit("rollback must use the declared undo-step input")

    fixture = expected["fixture"]
    if (fixture["width_px"], fixture["height_px"]) != (2048, 1536):
        raise SystemExit("fixture dimensions drifted")
    if fixture["bytes"] <= 0 or len(fixture["sha256"]) != 64:
        raise SystemExit("fixture metadata is incomplete")

    draw = expected["draw"]
    if draw["outline_rgb"] != [237, 28, 36] or draw["outline_px"] != 3:
        raise SystemExit("T3.3 draw contract drifted")
    if draw["fill"] != "none":
        raise SystemExit("T3.3 rectangle must remain unfilled")
    if draw["start"] != {"x": 64, "y": 48}:
        raise SystemExit("T3.3 start coordinate drifted")
    if draw["end"] != {"x": 1984, "y": 1488}:
        raise SystemExit("T3.3 end coordinate drifted")

    rollback = expected["rollback"]
    if rollback["expected_undo_steps"] != 1:
        raise SystemExit("T3.3 standard draw must declare one undo step")
    if rollback["visual_metric"] != "mean_abs_diff_within":
        raise SystemExit("T3.3 rollback must verify pixels")
    if rollback["max_mean_abs_diff"] > 2:
        raise SystemExit("T3.3 pixel tolerance must not exceed 2")
    if rollback["confidence_min"] < 0.9:
        raise SystemExit("T3.3 confidence floor must not be relaxed")
    if expected["real_run_status"] != "passed_10_of_10":
        raise SystemExit("T3.3 real-machine status must reflect the ten runs")
    if expected["undo_granularity"] != "one_step_observed_10_of_10":
        raise SystemExit("T3.3 undo granularity must reflect the measured runs")

    if summary["requested_runs"] != 10 or summary["passed_runs"] != 10:
        raise SystemExit("T3.3 ten-run summary is not 10/10")
    if summary["undo_step_assertion_passed_runs"] != 10:
        raise SystemExit("T3.3 undo-step assertion did not pass every run")
    if summary["pixel_rollback_assertion_passed_runs"] != 10:
        raise SystemExit("T3.3 pixel rollback assertion did not pass every run")
    if summary["silent_failure_rate"] != 0:
        raise SystemExit("T3.3 silent failure rate must remain zero")

    case_ids = {case["id"] for case in cases["cases"]}
    for required in (
        "standard_draw_undo_returns_to_pre_snapshot",
        "foreground_lost_before_undo",
        "noop_undo",
        "undo_does_not_restore_pre_snapshot",
    ):
        if required not in case_ids:
            raise SystemExit(f"missing T3.3 case `{required}`")

    print(
        "paint_t3_3_static_validation_ok "
        f"fixture_sha256={fixture['sha256']} tools={len(task_tools)} "
        f"cases={len(cases['cases'])}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
