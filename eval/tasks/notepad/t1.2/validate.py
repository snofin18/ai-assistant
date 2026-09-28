"""Static validation for the T1.2 Notepad task and evaluation declarations."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
TASK_DIR = ROOT / "adapters" / "com.microsoft.notepad" / "tasks"
TOOLS_PATH = ROOT / "adapters" / "com.microsoft.notepad" / "tools" / "tools.json"
EVAL_DIR = Path(__file__).resolve().parent


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise AssertionError(f"{path} must contain a JSON object")
    return value


def assert_error_codes_are_known(task: dict) -> None:
    known = {
        "ModelInvalidOutput",
        "ModelNetworkFailure",
        "ToolInvalidArgs",
        "PolicyDenied",
        "TargetNotFound",
        "TargetAmbiguous",
        "TargetUnresponsive",
        "CapabilityMissing",
        "VerifyFailed",
        "PlatformPermission",
        "Transient",
        "UserInteraction",
        "Fatal",
    }
    mapped = {entry["error_code"] for entry in task["failure_mapping"]}
    if not mapped <= known:
        raise AssertionError(f"unknown failure mappings: {sorted(mapped - known)}")
    postconditions = {entry["error_code"] for entry in task["postconditions"]}
    if not postconditions <= known:
        raise AssertionError(
            f"unknown postcondition mappings: {sorted(postconditions - known)}"
        )


def main() -> None:
    task = load_json(TASK_DIR / "t1.2.replace-save-approval-undo.json")
    cases = load_json(EVAL_DIR / "cases.json")
    expected = load_json(EVAL_DIR / "expected.json")
    tools = load_json(TOOLS_PATH)

    assert task["schema_version"] == cases["schema_version"] == 1
    assert expected["schema_version"] == 1
    assert task["task_id"] == cases["task_id"] == expected["task_id"]
    assert task["read_only"] is False
    assert task["open_mode"] == "platform_open_file"
    assert task["metrics"]["repeat_runs"] == 10
    assert task["metrics"]["silent_failure_rate"] == 0
    assert task["metrics"]["approval_bypass_rate"] == 0

    case_ids = [case["id"] for case in cases["cases"]]
    assert len(case_ids) == 11, case_ids
    assert len(set(case_ids)) == len(case_ids), case_ids
    assert cases["repeat_runs"] == 10
    assert cases["primary_case"] in case_ids
    assert set(case_ids) == set(expected["cases"])

    registered_tools = {tool["schema"]["name"] for tool in tools["tools"]}
    referenced_tools = {
        step["tool"] for step in task["steps"] if step.get("kind") == "tool"
    }
    assert referenced_tools <= registered_tools, referenced_tools - registered_tools
    assert referenced_tools == {
        "notepad.file.read_text",
        "notepad.file.replace_text",
        "notepad.file.save",
    }

    approval = task["approval"]
    assert approval["replace_text"]["show_diff"] is True
    assert approval["replace_text"]["scope_options"] == ["once"]
    assert approval["save"]["show_diff"] is True
    assert approval["save"]["scope_options"] == ["once"]
    assert approval["save"]["point_of_no_return"] is False

    rollback = task["rollback"]
    assert rollback["required_anchor_levels"] == ["L0", "L1"]
    assert rollback["pre_replace_anchor_required"] is True
    assert rollback["pre_save_disk_snapshot_required"] is True
    undo_ids = {entry["id"] for entry in rollback["undo_verification"]}
    assert undo_ids == {
        "undo_l0_before_save",
        "undo_l1_after_save",
        "undo_l1_when_l0_unavailable",
    }

    step_ids = {step["id"] for step in task["steps"]}
    assert {
        "resolve_editor",
        "prepare_rollback_anchors",
        "prepare_replace_diff",
        "approve_replace",
        "replace_text",
        "prepare_save_diff",
        "approve_save",
        "save_file",
        "verify_postconditions",
    } <= step_ids
    resolve_step = next(step for step in task["steps"] if step["id"] == "resolve_editor")
    assert resolve_step["args"]["on_ambiguous"] == "$input.target_resolution_policy"

    postcondition_ids = {entry["id"] for entry in task["postconditions"]}
    assert {
        "replacement_count_equals",
        "canonical_text_after_equals_expected",
        "save_reported_success",
        "title_has_no_unsaved_marker",
        "disk_content_matches_canonical_after",
        "rollback_anchors_are_ready",
        "approval_evidence_present",
    } <= postcondition_ids

    assert expected["approval_contract"] == {
        "replace_text": {
            "risk": "medium",
            "show_diff": True,
            "scope_options": ["once"],
        },
        "save": {
            "risk": "high",
            "show_diff": True,
            "scope_options": ["once"],
            "point_of_no_return": False,
        },
    }
    assert expected["rollback_contract"]["ctrl_z_after_save_restores_disk"] is False
    assert expected["cases"]["replace_single_occurrence"]["repeat_runs"] == 10
    assert expected["cases"]["replace_single_occurrence"]["minimum_successful_runs"] == 9
    assert expected["cases"]["ambiguous_editor_target_negative"]["error_code"] == (
        "TargetAmbiguous"
    )
    assert expected["cases"]["replacement_count_mismatch_negative"]["error_code"] == (
        "VerifyFailed"
    )
    assert expected["cases"]["approval_denied_negative"]["error_code"] == (
        "UserInteraction"
    )
    assert expected["cases"]["save_postcondition_failed_negative"]["error_code"] == (
        "VerifyFailed"
    )
    assert expected["cases"]["undo_l0_before_save"]["undo_level_used"] == "L0"
    assert expected["cases"]["undo_l1_after_save"]["undo_level_used"] == "L1"
    assert expected["cases"]["undo_l1_after_save"]["ctrl_z_alone_sufficient"] is False
    assert expected["cases"]["undo_l1_when_l0_unavailable"]["fallback_used"] is True

    script = (EVAL_DIR / "generate-fixtures.py").read_text(encoding="utf-8")
    for fixture in {
        "single_occurrence.txt",
        "multiple_occurrences.txt",
        "crlf_cjk.txt",
        "no_match.txt",
        "wrong_count.txt",
    }:
        assert fixture in script, fixture

    assert_error_codes_are_known(task)
    print(f"t1_2_static_validation_ok cases={len(case_ids)}")


if __name__ == "__main__":
    main()
