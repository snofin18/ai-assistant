"""Static validation for the T1.1 task and evaluation declarations."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
TASK_DIR = ROOT / "adapters" / "com.microsoft.notepad" / "tasks"
EVAL_DIR = Path(__file__).resolve().parent


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise AssertionError(f"{path} must contain a JSON object")
    return value


def main() -> None:
    task = load_json(TASK_DIR / "t1.1.open-read-full-text.json")
    cases = load_json(EVAL_DIR / "cases.json")
    expected = load_json(EVAL_DIR / "expected.json")

    assert task["schema_version"] == 1
    assert cases["schema_version"] == 1
    assert expected["schema_version"] == 1
    assert task["task_id"] == cases["task_id"] == expected["task_id"]
    assert task["read_only"] is True
    assert task["open_mode"] == "platform_open_file"
    assert task["metrics"]["repeat_runs"] == 10

    case_ids = [case["id"] for case in cases["cases"]]
    assert len(case_ids) == 10, case_ids
    assert len(set(case_ids)) == len(case_ids), case_ids
    assert cases["repeat_runs"] == 10
    assert cases["primary_case"] in case_ids
    assert set(case_ids) == set(expected["cases"])

    large = expected["cases"]["large_1mb_file_channel"]
    assert large["text_source"] == "file_channel"
    assert large["truncated"] is True
    assert large["analysis_scope"] == "truncated_prefix"
    assert large["line_count_total"] == 105000
    assert large["paragraphs_truncated"] is True

    missing = expected["cases"]["missing_file_negative"]
    assert missing["outcome"] == "error"
    assert missing["error_code"] == "TargetNotFound"

    script = (EVAL_DIR / "generate-fixtures.py").read_text(encoding="utf-8")
    assert "large_1mb.txt" in script
    assert "35000" in script
    print(f"t1_1_static_validation_ok cases={len(case_ids)}")


if __name__ == "__main__":
    main()
