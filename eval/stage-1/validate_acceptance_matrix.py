"""Validate the Stage 1 nine-task repeated-run acceptance matrix."""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MATRIX = Path(__file__).with_name("acceptance-matrix.json")

EXPECTED_TASKS = {
    "t1.1": ("notepad", 0.9, "notepad_runs_summary"),
    "t1.2": ("notepad", 0.9, "notepad_runs_summary"),
    "t1.3": ("notepad", 0.9, "notepad_runs_summary"),
    "t3.1": ("paint", 0.75, "round_summary"),
    "t3.2": ("paint", 0.75, "round_summary"),
    "t3.3": ("paint", 0.75, "round_summary"),
    "t5.1": ("edge", 0.75, "round_summary"),
    "t5.2": ("edge", 0.75, "single_human_approval"),
    "t5.3": ("edge", 1.0, "round_summary"),
}


class MatrixError(ValueError):
    """Raised when matrix structure or evidence cannot be trusted."""


def _require_object(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise MatrixError(f"{field} must be an object")
    return value


def _require_list(value: Any, field: str) -> list[Any]:
    if not isinstance(value, list):
        raise MatrixError(f"{field} must be an array")
    return value


def _require_int(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise MatrixError(f"{field} must be a non-negative integer")
    return value


def _require_number(value: Any, field: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise MatrixError(f"{field} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < 0.0 or number > 1.0:
        raise MatrixError(f"{field} must be within 0..1")
    return number


def _require_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise MatrixError(f"{field} must be a non-empty string")
    return value


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise MatrixError(f"evidence file missing: {_display_path(path)}") from error
    except (OSError, json.JSONDecodeError) as error:
        raise MatrixError(
            f"evidence file unreadable: {_display_path(path)}: {error}"
        ) from error
    return _require_object(value, _display_path(path))


def load_matrix(path: Path = DEFAULT_MATRIX) -> dict[str, Any]:
    matrix = _load_json(path)
    if matrix.get("schema_version") != 1:
        raise MatrixError("matrix.schema_version must be 1")
    if matrix.get("stage") != 1:
        raise MatrixError("matrix.stage must be 1")
    if matrix.get("required_runs") != 10:
        raise MatrixError("matrix.required_runs must be 10")

    tasks = [_require_object(item, "matrix.tasks[]") for item in _require_list(matrix.get("tasks"), "matrix.tasks")]
    seen: set[str] = set()
    app_counts = {"notepad": 0, "paint": 0, "edge": 0}
    for task in tasks:
        task_id = _require_string(task.get("task_id"), "task.task_id")
        if task_id in seen:
            raise MatrixError(f"duplicate task_id: {task_id}")
        seen.add(task_id)
        if task_id not in EXPECTED_TASKS:
            raise MatrixError(f"unexpected task_id: {task_id}")
        expected_app, expected_rate, expected_adapter = EXPECTED_TASKS[task_id]
        app = _require_string(task.get("app"), f"{task_id}.app")
        adapter = _require_string(task.get("adapter"), f"{task_id}.adapter")
        rate = _require_number(task.get("minimum_success_rate"), f"{task_id}.minimum_success_rate")
        evidence = _require_string(task.get("evidence"), f"{task_id}.evidence")
        if app != expected_app or adapter != expected_adapter or rate != expected_rate:
            raise MatrixError(f"{task_id} app / adapter / minimum_success_rate drifted")
        evidence_path = Path(evidence)
        if evidence_path.is_absolute() or ".." in evidence_path.parts:
            raise MatrixError(f"{task_id}.evidence must be a repository-relative path")
        app_counts[app] += 1

    if seen != set(EXPECTED_TASKS):
        raise MatrixError(f"matrix task set mismatch: missing={sorted(set(EXPECTED_TASKS) - seen)}")
    if app_counts != {"notepad": 3, "paint": 3, "edge": 3}:
        raise MatrixError(f"matrix app counts drifted: {app_counts}")
    return matrix


def _round_result(task_id: str, evidence: str, data: dict[str, Any], required_runs: int) -> dict[str, Any]:
    requested = _require_int(
        data.get("requested_runs", data.get("required_rounds")),
        f"{task_id}.requested_runs",
    )
    passed = _require_int(
        data.get("passed_runs", data.get("successful_rounds")),
        f"{task_id}.passed_runs",
    )
    if passed > requested:
        raise MatrixError(f"{task_id} passed_runs exceeds requested_runs")
    rate = _require_number(data.get("success_rate"), f"{task_id}.success_rate")
    computed_rate = passed / requested if requested else 0.0
    if not math.isclose(rate, computed_rate, rel_tol=0.0, abs_tol=1e-9):
        raise MatrixError(f"{task_id} success_rate does not match passed/requested")

    rounds = data.get("rounds")
    if rounds is not None:
        records = _require_list(rounds, f"{task_id}.rounds")
        if len(records) != requested:
            raise MatrixError(f"{task_id} rounds length does not match requested_runs")
        passed_flags = [
            record.get("passed")
            for record in records
            if isinstance(record, dict) and isinstance(record.get("passed"), bool)
        ]
        if passed_flags and sum(passed_flags) != passed:
            raise MatrixError(f"{task_id} round passed flags disagree with passed_runs")

    result = data.get("result")
    if result is not None and result != "passed" and passed == requested:
        raise MatrixError(f"{task_id} result contradicts a full pass summary")
    reasons = []
    if requested < required_runs:
        reasons.append(f"insufficient_runs:{requested}/{required_runs}")
    if result is not None and result != "passed":
        reasons.append(f"result:{result}")
    return {
        "observed_runs": requested,
        "passed_runs": passed,
        "success_rate": rate,
        "reasons": reasons,
    }


def _notepad_result(task_id: str, data: dict[str, Any], required_runs: int) -> dict[str, Any]:
    summary_items = _require_list(data.get("summary"), "notepad.summary")
    item = next(
        (
            summary
            for summary in summary_items
            if isinstance(summary, dict) and summary.get("task") == task_id
        ),
        None,
    )
    if item is None:
        raise MatrixError(f"{task_id} missing from Notepad summary")
    requested = _require_int(item.get("runs"), f"{task_id}.runs")
    passed = _require_int(item.get("passed"), f"{task_id}.passed")
    failed = _require_int(item.get("failed"), f"{task_id}.failed")
    if requested != passed + failed:
        raise MatrixError(f"{task_id} runs != passed + failed")
    if item.get("minimum_success_rate_met") is not True:
        raise MatrixError(f"{task_id} source summary did not meet its threshold")

    records = [
        run
        for run in _require_list(data.get("runs"), "notepad.runs")
        if isinstance(run, dict) and run.get("task") == task_id
    ]
    if len(records) != requested:
        raise MatrixError(f"{task_id} detail records do not match summary runs")
    if any(run.get("passed") is not True or run.get("exit_code") != 0 for run in records):
        raise MatrixError(f"{task_id} contains failed detail records")
    reasons = [] if requested >= required_runs else [f"insufficient_runs:{requested}/{required_runs}"]
    return {
        "observed_runs": requested,
        "passed_runs": passed,
        "success_rate": passed / requested if requested else 0.0,
        "reasons": reasons,
    }


def _single_human_result(task_id: str, data: dict[str, Any], required_runs: int) -> dict[str, Any]:
    if data.get("result") != "passed":
        raise MatrixError(f"{task_id} single human summary is not passed")
    if data.get("submitted") is not True or data.get("submit_step_count") != 1:
        raise MatrixError(f"{task_id} single human summary is not exactly one submit")
    return {
        "observed_runs": 1,
        "passed_runs": 1,
        "success_rate": 1.0,
        "reasons": [f"insufficient_runs:1/{required_runs}"],
    }


def evaluate_matrix(matrix: dict[str, Any]) -> dict[str, Any]:
    required_runs = _require_int(matrix.get("required_runs"), "matrix.required_runs")
    results = []
    for task in matrix["tasks"]:
        task_id = task["task_id"]
        evidence = task["evidence"]
        data = _load_json(ROOT / evidence)
        adapter = task["adapter"]
        if adapter == "round_summary":
            observed = _round_result(task_id, evidence, data, required_runs)
        elif adapter == "notepad_runs_summary":
            observed = _notepad_result(task_id, data, required_runs)
        elif adapter == "single_human_approval":
            observed = _single_human_result(task_id, data, required_runs)
        else:
            raise MatrixError(f"unknown adapter: {adapter}")

        minimum_rate = float(task["minimum_success_rate"])
        minimum_passed = math.ceil(required_runs * minimum_rate)
        reasons = list(observed["reasons"])
        if observed["passed_runs"] < minimum_passed:
            reasons.append(
                f"insufficient_passes:{observed['passed_runs']}/{minimum_passed}"
            )
        if observed["success_rate"] < minimum_rate:
            reasons.append(
                f"success_rate_below_threshold:{observed['success_rate']}/{minimum_rate}"
            )
        results.append(
            {
                "task_id": task_id,
                "app": task["app"],
                "evidence": evidence,
                "observed_runs": observed["observed_runs"],
                "passed_runs": observed["passed_runs"],
                "success_rate": observed["success_rate"],
                "minimum_success_rate": minimum_rate,
                "minimum_passed_runs": minimum_passed,
                "complete": not reasons,
                "reasons": reasons,
            }
        )

    missing = [result["task_id"] for result in results if not result["complete"]]
    return {
        "schema_version": 1,
        "stage": 1,
        "required_runs": required_runs,
        "stage1_complete": not missing,
        "completed_tasks": len(results) - len(missing),
        "total_tasks": len(results),
        "missing_tasks": missing,
        "tasks": results,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args(argv)
    try:
        matrix = load_matrix(args.matrix)
        report = evaluate_matrix(matrix)
    except MatrixError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print(json.dumps(report, indent=2, sort_keys=True))
    if args.require_complete and not report["stage1_complete"]:
        print(
            "Stage 1 acceptance is incomplete: " + ", ".join(report["missing_tasks"]),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
