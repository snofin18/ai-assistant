#!/usr/bin/env python3
"""Validate the declarative Microsoft Notepad adapter pack."""

from __future__ import annotations

import json
import math
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


PACK_ROOT = Path(__file__).resolve().parents[1]
APP_ID = "com.microsoft.notepad"
ERRORS: list[str] = []

EXPECTED_BUNDLE = {
    "app_map": "app_map.json",
    "app_map_projection": "memory/app_map.v1.json",
    "app_map_projection_model": "core_app_map_v1",
    "selectors": "selectors/targets.json",
    "tools": "tools/tools.json",
    "rollback": "rollback/recipes.json",
    "interrupts": "interrupts/interrupts.json",
}

EXPECTED_TARGETS = [
    "main_window",
    "editor",
    "tab_view",
    "add_tab_button",
    "close_tab_button",
    "file_menu",
    "save_as_dialog",
    "save_as_filename",
    "save_as_save_button",
]

EXPECTED_TOOLS = {
    "notepad.file.read_text": ("read", "none_readonly", "low", False, True),
    "notepad.file.replace_text": ("write", "l1_snapshot", "medium", True, False),
    "notepad.file.save": ("write", "l1_snapshot", "high", True, False),
    "notepad.tab.new": ("write", "l2_compensation", "low", False, False),
    "notepad.file.save_as": ("write", "l3_irreversible", "high", True, False),
}

EXPECTED_CAPABILITIES = {
    "notepad.file.read_text": {"element.read_text", "target.resolve"},
    "notepad.file.replace_text": {"element.set_value", "target.resolve"},
    "notepad.file.save": {"input.keyboard", "target.resolve"},
    "notepad.tab.new": {"input.keyboard", "target.resolve"},
    "notepad.file.save_as": {
        "element.invoke",
        "input.keyboard",
        "target.resolve",
    },
}


def check(condition: bool, message: str) -> None:
    """Record one failed contract check."""
    if not condition:
        ERRORS.append(message)


def load_json(relative_path: str) -> dict[str, Any]:
    """Read one JSON object from the pack."""
    path = PACK_ROOT / relative_path
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        ERRORS.append(f"{relative_path}: cannot parse JSON: {error}")
        return {}
    check(isinstance(value, dict), f"{relative_path}: root must be an object")
    return value if isinstance(value, dict) else {}


def canonical_projection(app_map: dict[str, Any]) -> bytes:
    """Rebuild the canonical App Map projection bytes from its manifest."""
    manifest = app_map.get("projection_manifest", {})
    entries = sorted(manifest.get("entries", []), key=lambda item: item.get("id", ""))
    projection = {
        "version": 1,
        "app_id": APP_ID,
        "entries": [
            {
                "id": entry.get("id"),
                "title": entry.get("title"),
                "content": entry.get("content"),
                "source_reference": entry.get("source_reference"),
                "token_estimate": entry.get("token_estimate"),
            }
            for entry in entries
        ],
    }
    return (
        json.dumps(projection, ensure_ascii=False, indent=2, separators=(",", ": "))
        + "\n"
    ).encode("utf-8")


def validate_projection(app_map: dict[str, Any], projection: dict[str, Any]) -> None:
    """Validate the authored manifest and its generated projection."""
    manifest = app_map.get("projection_manifest", {})
    check(manifest.get("model") == "core_app_map_v1", "projection model mismatch")
    entries = manifest.get("entries", [])
    omissions = manifest.get("omissions", [])
    check(isinstance(entries, list) and entries, "projection entries missing")
    check(isinstance(omissions, list), "projection omissions must be a list")

    entry_ids = [entry.get("id") for entry in entries if isinstance(entry, dict)]
    check(entry_ids == sorted(entry_ids), "projection entries must be sorted by id")
    check(len(entry_ids) == len(set(entry_ids)), "projection entry ids must be unique")
    for entry in entries:
        entry_id = entry.get("id")
        check(
            isinstance(entry_id, str) and re.fullmatch(r"[a-z][a-z0-9_]*", entry_id),
            f"projection entry id is invalid: {entry_id}",
        )
        content = entry.get("content", "")
        expected_tokens = max(1, math.ceil(len(content) / 4))
        check(
            entry.get("token_estimate") == expected_tokens,
            f"projection entry {entry_id}: token_estimate mismatch",
        )

    covered_references = {
        item.get("source_reference")
        for item in [*entries, *omissions]
        if isinstance(item, dict)
    }
    for domain in (
        "identity",
        "commands",
        "ui_map",
        "workflows",
        "known_pitfalls",
        "state_machine",
        "undo_capability",
        "text_conventions",
    ):
        check(
            f"bundle:app_map#/{domain}" in covered_references,
            f"projection does not cover App Map domain: {domain}",
        )

    projection_path = PACK_ROOT / EXPECTED_BUNDLE["app_map_projection"]
    check(
        projection_path.read_bytes() == canonical_projection(app_map),
        "memory/app_map.v1.json: canonical projection byte drift",
    )
    check(projection.get("app_id") == APP_ID, "projection app_id mismatch")


def validate_selectors(selectors: dict[str, Any]) -> None:
    """Lock the Notepad selector vocabulary and fallback order."""
    check(selectors.get("selector_model") == "uia_candidate_chain_v1", "selector model mismatch")
    check(selectors.get("app_version_range") == ">=11.2607.14 <12", "selector version mismatch")
    check(selectors.get("probe_status") == "observed", "selector pack probe status mismatch")
    policy = selectors.get("policy", {})
    check(policy.get("on_ambiguous") == "error_and_ask", "selector ambiguity policy mismatch")
    check(
        policy.get("on_not_found", {}).get("retry_after_ms") == [100, 500],
        "selector retry policy mismatch",
    )
    targets = selectors.get("targets", [])
    target_ids = [target.get("id") for target in targets if isinstance(target, dict)]
    check(target_ids == EXPECTED_TARGETS, "selector target set or order mismatch")
    targets_by_id = {target.get("id"): target for target in targets}

    for target_id in EXPECTED_TARGETS[:6]:
        check(
            targets_by_id.get(target_id, {}).get("probe_status") == "observed",
            f"selector target {target_id}: expected observed probe status",
        )
    for target_id in EXPECTED_TARGETS[6:]:
        check(
            targets_by_id.get(target_id, {}).get("probe_status") == "required",
            f"selector target {target_id}: expected required probe status",
        )

    editor_candidates = targets_by_id.get("editor", {}).get("candidates", [])
    check(
        [candidate.get("id") for candidate in editor_candidates[:2]]
        == ["editor-modern", "editor-legacy"],
        "editor selector primary/fallback order changed",
    )
    modern = editor_candidates[0] if editor_candidates else {}
    check(
        modern.get("value", {}).get("class_and_role")
        == {"class": "RichEditD2DPT", "role": "Document"},
        "editor modern candidate changed",
    )
    check(modern.get("locale_dependent") is False, "editor modern candidate became localized")
    check(modern.get("fallback_only") is False, "editor modern candidate must remain primary")

    for target in targets:
        candidates = target.get("candidates", [])
        check(bool(candidates), f"selector target {target.get('id')}: candidates missing")
        primary_candidates = [
            candidate for candidate in candidates if candidate.get("fallback_only") is False
        ]
        check(
            bool(primary_candidates),
            f"selector target {target.get('id')}: at least one primary candidate required",
        )
        for candidate in candidates:
            if candidate.get("fallback_only") is True:
                check(
                    candidate.get("locale_dependent") is True,
                    f"selector target {target.get('id')}: fallback must be localized",
                )


def validate_tools(tools_document: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Lock tool identity, safety metadata, capabilities, and postconditions."""
    tools = tools_document.get("tools", [])
    check(isinstance(tools, list) and tools, "tools/tools.json: tools missing")
    tool_by_name = {
        entry.get("schema", {}).get("name"): entry
        for entry in tools
        if isinstance(entry, dict)
    }
    check(set(tool_by_name) == set(EXPECTED_TOOLS), "tool set mismatch")

    for name, expected in EXPECTED_TOOLS.items():
        entry = tool_by_name.get(name, {})
        schema = entry.get("schema", {})
        actual = (
            schema.get("effect"),
            schema.get("reversibility"),
            schema.get("risk_level"),
            schema.get("requires_approval"),
            schema.get("idempotent"),
        )
        check(actual == expected, f"tool {name}: safety metadata mismatch")
        check(schema.get("version") == "1.1", f"tool {name}: schema version must be 1.1")
        capabilities = entry.get("capabilities", [])
        check(
            len(capabilities) == len(set(capabilities)),
            f"tool {name}: duplicate capabilities",
        )
        check(
            set(capabilities) == EXPECTED_CAPABILITIES[name],
            f"tool {name}: capability set mismatch",
        )
        postconditions = entry.get("postconditions", [])
        check(bool(postconditions), f"tool {name}: postconditions missing")
        check(
            all(
                postcondition.get("error_code") == "VerifyFailed"
                for postcondition in postconditions
            ),
            f"tool {name}: postcondition error code mismatch",
        )

    read_entry = tool_by_name.get("notepad.file.read_text", {})
    check(
        [item.get("kind") for item in read_entry.get("postconditions", [])]
        == ["state_unchanged"],
        "notepad.file.read_text: state_unchanged postcondition required",
    )
    for name in (
        "notepad.file.replace_text",
        "notepad.file.save",
        "notepad.tab.new",
        "notepad.file.save_as",
    ):
        kinds = {
            item.get("kind")
            for item in tool_by_name.get(name, {}).get("postconditions", [])
        }
        check("state_changed" in kinds or "file_changed" in kinds,
              f"tool {name}: executable write postcondition missing")

    save_as_postconditions = tool_by_name.get("notepad.file.save_as", {}).get(
        "postconditions", []
    )
    check(
        save_as_postconditions
        == [
            {
                "kind": "file_changed",
                "path": "{{target_path}}",
                "expect": "digest",
                "description": "The requested Save As target exists with a file digest.",
                "error_code": "VerifyFailed",
            }
        ],
        "notepad.file.save_as: file_changed postcondition changed",
    )
    return tool_by_name


def validate_rollback(
    rollback: dict[str, Any],
    tool_by_name: dict[str, dict[str, Any]],
) -> None:
    """Lock write coverage and the approved created-tab compensation."""
    recipes = rollback.get("recipes", [])
    recipe_by_tool = {recipe.get("tool"): recipe for recipe in recipes}
    write_tools = {
        name
        for name, entry in tool_by_name.items()
        if entry.get("schema", {}).get("effect") == "write"
    }
    check(set(recipe_by_tool) == write_tools, "rollback coverage set mismatch")

    replace_anchors = recipe_by_tool.get("notepad.file.replace_text", {}).get(
        "anchor_strategy", []
    )
    check(
        [anchor.get("level") for anchor in replace_anchors] == ["l0", "l1"],
        "notepad.file.replace_text: L0 then L1 anchors required",
    )
    save_anchor = recipe_by_tool.get("notepad.file.save", {}).get(
        "anchor_strategy", [{}]
    )[0]
    check(
        (save_anchor.get("level"), save_anchor.get("mechanism"))
        == ("l1", "shadow_copy"),
        "notepad.file.save: L1 shadow copy required",
    )
    check(
        recipe_by_tool.get("notepad.file.save_as", {}).get("coverage") == "no_recovery",
        "notepad.file.save_as: no_recovery required",
    )

    tab_anchor = recipe_by_tool.get("notepad.tab.new", {}).get(
        "anchor_strategy", [{}]
    )[0]
    restore = tab_anchor.get("restore", {})
    check(
        (tab_anchor.get("level"), tab_anchor.get("mechanism"))
        == ("l2", "compensating_action"),
        "notepad.tab.new: L2 compensating action required",
    )
    check(
        restore
        == {
            "kind": "close_created_target",
            "target_kind": "tab",
            "require_empty": True,
        },
        "notepad.tab.new: close_created_target restore changed",
    )
    check(
        "whole_window_fingerprint"
        in tab_anchor.get("capture", {}).get("observations", []),
        "notepad.tab.new: whole-window fingerprint observation required",
    )


def validate_interrupts(interrupts: dict[str, Any]) -> None:
    """Lock human-only dialogs and typed action routing."""
    check(interrupts.get("default_action") == "cancel", "interrupt default action must cancel")
    serialized = json.dumps(interrupts, ensure_ascii=False)
    check("delegate_to_task" not in serialized, "historical delegate_to_task action remains")
    entries = {
        entry.get("id"): entry
        for entry in interrupts.get("interrupts", [])
        if isinstance(entry, dict)
    }
    check(
        set(entries) == {
            "unsaved-changes-dialog",
            "save-as-dialog",
            "transient-teaching-tip",
        },
        "interrupt set mismatch",
    )

    unsaved = entries.get("unsaved-changes-dialog", {})
    check(unsaved.get("priority") == 300, "unsaved dialog priority changed")
    check(unsaved.get("action") == {"kind": "ask_human"}, "unsaved dialog must ask human")
    check(unsaved.get("requires_human") is True, "unsaved dialog must require human")
    check(unsaved.get("resume_after") == "user_decision", "unsaved resume condition changed")
    check(
        all(
            candidate.get("locale_dependent") is True
            for candidate in unsaved.get("match", {}).get("title_candidates", [])
        ),
        "unsaved title candidates must remain locale-dependent",
    )

    save_as = entries.get("save-as-dialog", {})
    check(save_as.get("priority") == 200, "Save As dialog priority changed")
    check(
        save_as.get("action")
        == {"kind": "delegate_to_tool", "tool": "notepad.file.save_as"},
        "Save As dialog delegate changed",
    )
    check(save_as.get("requires_human") is True, "Save As dialog must require human")
    check(save_as.get("resume_after") == "tool_resume", "Save As resume condition changed")

    transient = entries.get("transient-teaching-tip", {})
    check(transient.get("priority") == 100, "TeachingTip priority changed")
    check(
        transient.get("action") == {"kind": "dismiss_explicitly"},
        "TeachingTip must be dismissed explicitly",
    )
    check(transient.get("requires_human") is False, "TeachingTip must not require human")
    check("resume_after" not in transient, "TeachingTip must omit resume_after")


def main() -> int:
    """Run all pack checks and print one deterministic verdict."""
    try:
        adapter = tomllib.loads((PACK_ROOT / "adapter.toml").read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"FAILED: adapter.toml: {error}")
        return 1

    check(adapter.get("schema_version") == 1, "adapter schema_version mismatch")
    check(adapter.get("adapter", {}).get("app_id") == APP_ID, "adapter app_id mismatch")
    check(adapter.get("bundle") == EXPECTED_BUNDLE, "bundle envelope mismatch")
    safety = adapter.get("safety", {})
    check(safety.get("default_interrupt_action") == "cancel", "adapter interrupt default mismatch")
    check(safety.get("allow_unattended") is False, "adapter must forbid unattended writes")

    app_map = load_json(EXPECTED_BUNDLE["app_map"])
    projection = load_json(EXPECTED_BUNDLE["app_map_projection"])
    selectors = load_json(EXPECTED_BUNDLE["selectors"])
    tools_document = load_json(EXPECTED_BUNDLE["tools"])
    rollback = load_json(EXPECTED_BUNDLE["rollback"])
    interrupts = load_json(EXPECTED_BUNDLE["interrupts"])

    for label, document in (
        ("app_map.json", app_map),
        ("selectors/targets.json", selectors),
        ("tools/tools.json", tools_document),
        ("rollback/recipes.json", rollback),
        ("interrupts/interrupts.json", interrupts),
    ):
        check(document.get("app_id") == APP_ID, f"{label}: app_id mismatch")
        if label != "app_map.json" or document:
            check(document.get("schema_version") == 1, f"{label}: schema_version mismatch")

    validate_projection(app_map, projection)
    validate_selectors(selectors)
    tool_by_name = validate_tools(tools_document)
    validate_rollback(rollback, tool_by_name)
    validate_interrupts(interrupts)

    if ERRORS:
        for error in ERRORS:
            print(f"FAILED: {error}")
        print(f"verdict: FAILED ({len(ERRORS)} error(s))")
        return 1

    print(
        "PASSED: "
        f"app_id={APP_ID} "
        f"targets={len(EXPECTED_TARGETS)} "
        f"tools={len(EXPECTED_TOOLS)} "
        f"recipes={len(rollback.get('recipes', []))} "
        "interrupts=3"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
