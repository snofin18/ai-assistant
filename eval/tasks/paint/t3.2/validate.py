#!/usr/bin/env python3
"""Static validation for the Paint T3.2 task package and fixture contract."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    expected = _load(HERE / "expected.json")
    cases = _load(HERE / "cases.json")
    task = _load(
        ROOT
        / "adapters/com.microsoft.paint/tasks/t3.2.open-png-mark-region-save-as.json"
    )
    tools = _load(ROOT / "adapters/com.microsoft.paint/tools/tools.json")["tools"]

    subprocess.run(
        [sys.executable, str(HERE / "prepare_fixture.py"), "--check"],
        cwd=ROOT,
        check=True,
    )

    declared_tools = {
        entry["schema"]["name"]
        for entry in tools
        if isinstance(entry, dict) and "schema" in entry
    }
    task_tools = {
        step["tool"]
        for step in task["steps"]
        if step.get("kind") == "tool" and "tool" in step
    }
    missing = sorted(task_tools - declared_tools)
    if missing:
        raise SystemExit(f"task references undeclared tools: {missing}")
    if "paint.file.open" not in task_tools or "paint.file.save_as" not in task_tools:
        raise SystemExit("T3.2 must use paint.file.open and paint.file.save_as")

    open_step = next(
        step for step in task["steps"] if step.get("tool") == "paint.file.open"
    )
    save_step = next(
        step for step in task["steps"] if step.get("tool") == "paint.file.save_as"
    )
    if open_step.get("args", {}).get("discard_unsaved_confirmed") is not False:
        raise SystemExit("paint.file.open must explicitly refuse unsaved discard")
    if save_step.get("args", {}).get("overwrite_existing") is not False:
        raise SystemExit("paint.file.save_as must reject overwrite_existing=true")

    fixture = expected["fixture"]
    if (fixture["width_px"], fixture["height_px"]) != (2048, 1536):
        raise SystemExit("fixture dimensions drifted")
    if fixture["bytes"] <= 0 or len(fixture["sha256"]) != 64:
        raise SystemExit("fixture metadata is incomplete")

    mark = expected["mark"]
    if mark["outline_px"] != 3 or mark["outline_rgb"] != [237, 28, 36]:
        raise SystemExit("T3.2 mark must stay 3 px red")
    if mark["fill"] != "none":
        raise SystemExit("T3.2 mark must stay unfilled")
    if mark["start"] != {"x": 64, "y": 48}:
        raise SystemExit("T3.2 start coordinate drifted")
    if mark["end"] != {"x": 1984, "y": 1488}:
        raise SystemExit("T3.2 end coordinate drifted")

    case_ids = {case["id"] for case in cases["cases"]}
    for required in (
        "standard_open_mark_save_as",
        "target_already_exists",
        "target_created_in_dialog_race",
    ):
        if required not in case_ids:
            raise SystemExit(f"missing T3.2 case `{required}`")

    print(
        "paint_t3_2_static_validation_ok "
        f"fixture_sha256={fixture['sha256']} bytes={fixture['bytes']} "
        f"tools={len(task_tools)} cases={len(cases['cases'])}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
