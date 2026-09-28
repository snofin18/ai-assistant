"""Static validation for the T1.3 Notepad task and evaluation declarations."""

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


def assert_known_error_codes(task: dict) -> None:
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
    task = load_json(TASK_DIR / "t1.3.new-tab-write-save-as.json")
    cases = load_json(EVAL_DIR / "cases.json")
    expected = load_json(EVAL_DIR / "expected.json")
    tools = load_json(TOOLS_PATH)

    assert task["schema_version"] == cases["schema_version"] == 1
    assert expected["schema_version"] == 1
    assert task["task_id"] == cases["task_id"] == expected["task_id"]
    assert task["read_only"] is False
    assert task["open_mode"] == "none"
    assert task["metrics"]["repeat_runs"] == 10
    assert task["metrics"]["silent_failure_rate"] == 0
    assert task["metrics"]["silent_overwrite_rate"] == 0
    assert task["metrics"]["existing_target_mutation_rate"] == 0
    assert task["metrics"]["approval_bypass_rate"] == 0
    assert task["metrics"]["cross_process_dialog_success_rate_min"] == 0.9

    case_ids = [case["id"] for case in cases["cases"]]
    assert len(case_ids) == 14, case_ids
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
        "notepad.tab.new",
        "notepad.file.read_text",
        "notepad.file.save_as",
    }
    assert "notepad.file.write_text" not in referenced_tools

    approval = task["approval"]["save_as"]
    assert approval["risk"] == "high"
    assert approval["show_diff"] is True
    assert approval["scope_options"] == ["once"]
    assert approval["point_of_no_return"] is True

    path_policy = task["target_path_policy"]
    assert path_policy["existing_target_action"] == "reject_if_exists"
    assert path_policy["overwrite_existing"] is False
    assert path_policy["silent_overwrite_forbidden"] is True
    assert path_policy["tool_argument_overwrite_existing"] is False
    assert path_policy["future_overwrite_requires_backup_and_confirmation"] is True

    dialog = task["dialog_contract"]
    assert dialog["window_class"] == "#32770"
    assert dialog["must_differ_from_notepad_process"] is True
    assert dialog["identity_check_required"] is True
    assert dialog["filename_entry_requires_foreground"] is True
    assert dialog["allow_display_alerts_false"] is False

    save_as_step = next(
        step for step in task["steps"] if step["id"] == "save_as_new_file"
    )
    assert save_as_step["tool"] == "notepad.file.save_as"
    assert save_as_step["args"]["overwrite_existing"] is False

    step_ids = {step["id"] for step in task["steps"]}
    assert {
        "capture_initial_tab_state",
        "preflight_target_path",
        "reject_existing_target",
        "create_new_tab",
        "resolve_new_editor",
        "write_text",
        "read_back_text",
        "prepare_save_as_diff",
        "approve_save_as",
        "save_as_new_file",
        "verify_disk_readback",
        "verify_postconditions",
    } <= step_ids
    write_step = next(step for step in task["steps"] if step["id"] == "write_text")
    assert write_step["kind"] == "host_service"
    assert write_step["operation"] == "set_editor_value"
    assert "tool" not in write_step

    postcondition_ids = {entry["id"] for entry in task["postconditions"]}
    assert {
        "new_tab_count_increased_by_one",
        "new_tab_initially_empty",
        "written_text_read_back_equals",
        "save_as_approval_present",
        "save_dialog_is_cross_process",
        "target_file_created",
        "target_existed_before_is_false",
        "disk_content_matches_written_text",
        "title_has_no_unsaved_marker",
    } <= postcondition_ids

    assert expected["approval_contract"] == {
        "save_as": {
            "risk": "high",
            "show_diff": True,
            "scope_options": ["once"],
            "point_of_no_return": True,
        }
    }
    policy = expected["target_path_policy"]
    assert policy["existing_target_must_remain_byte_identical"] is True
    assert policy["save_as_tool_must_not_run_on_existing_target"] is True
    assert expected["cases"]["new_file_ascii"]["repeat_runs"] == 10
    assert expected["cases"]["new_file_ascii"]["minimum_successful_runs"] == 9
    assert expected["cases"]["existing_target_reject_negative"]["error_code"] == (
        "PolicyDenied"
    )
    assert expected["cases"]["target_race_before_save_negative"]["save_as_tool_called"] is (
        False
    )
    assert expected["cases"]["approval_denied_negative"]["error_code"] == (
        "UserInteraction"
    )
    assert expected["cases"]["save_dialog_not_found_negative"]["error_code"] == (
        "TargetNotFound"
    )
    assert expected["cases"]["save_dialog_identity_mismatch_negative"]["error_code"] == (
        "TargetNotFound"
    )
    assert expected["cases"]["filename_entry_capability_missing_negative"][
        "error_code"
    ] == "CapabilityMissing"
    assert expected["cases"]["user_cancelled_save_dialog_negative"]["error_code"] == (
        "UserInteraction"
    )
    assert expected["cases"]["disk_readback_mismatch_negative"]["error_code"] == (
        "VerifyFailed"
    )

    script = (EVAL_DIR / "generate-fixtures.py").read_text(encoding="utf-8")
    for fixture in {
        "ascii.txt",
        "utf8_cjk.txt",
        "path_with_spaces.txt",
        "empty.txt",
        "existing-target-source.txt",
        "race-source.txt",
        "existing-target.txt",
        "race-target.txt",
        "folder with spaces",
    }:
        assert fixture in script, fixture

    assert_known_error_codes(task)
    print(f"t1_3_static_validation_ok cases={len(case_ids)}")


if __name__ == "__main__":
    main()
