#!/usr/bin/env python3
"""Validate the declarative Microsoft Excel adapter safety shell."""

from __future__ import annotations

import json
import math
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


PACK_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = PACK_ROOT.parents[1]
APP_ID = "com.microsoft.excel"
VERSION_RANGE = ">=16.0 <17"
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

APP_MAP_DOMAINS = (
    "identity",
    "connection",
    "target_identity",
    "value_semantics",
    "commands",
    "ui_map",
    "workflows",
    "known_pitfalls",
    "state_machine",
    "undo_capability",
    "interrupt_policy",
    "safety_boundaries",
)

EXPECTED_TARGETS = ["main_window", "modal_dialog"]
EXPECTED_TARGET_SHAPES = {
    "main_window": ("window", "XLMAIN", "Window"),
    "modal_dialog": ("window", "#32770", "Window"),
}

EXPECTED_INTERRUPTS = {
    "protected-view": ("consent_dialog", "Enabling editing"),
    "autosave-network-conflict": ("unsupported_network_feature", "AutoSave"),
    "read-only-or-occupied": ("consent_dialog", "read-only"),
    "overwrite-confirmation": ("overwrite_confirmation", "Replace"),
    "update-links": ("consent_dialog", "external links"),
    "macro-warning": ("consent_dialog", "Macros"),
}

EXPECTED_INTERRUPT_CLASSES = {
    "protected-view": "XLMAIN",
    "autosave-network-conflict": "XLMAIN",
    "read-only-or-occupied": "#32770",
    "overwrite-confirmation": "#32770",
    "update-links": "#32770",
    "macro-warning": "#32770",
}

EXPECTED_HUMAN_CLASSES = {
    "protected_view",
    "autosave_network_conflict",
    "read_only_or_occupied",
    "overwrite_confirmation",
    "update_links",
    "macro_warning",
}
EXPECTED_DANGEROUS_ACTIONS = {
    "enable_editing",
    "save",
    "discard",
    "replace",
    "update_links",
    "accept_macro",
}
EXPECTED_TOOLS_BLOCKER = "excel_com_runner_and_tool_implementations_pending"
LEGACY_CAPABILITY_PREFIXES = ("office.", "spreadsheet.")
FORBIDDEN_COM_CAPABILITIES = {
    "com.execute",
    "com.eval",
    "com.invoke",
    "com.macro",
    "com.raw_dispatch",
    "com.script",
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
    """Rebuild canonical projection bytes from the authored manifest."""
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
    """Validate authored coverage and rendered projection bytes."""
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
            f"projection entry {entry_id}: token_estimate must be {expected_tokens}",
        )

    covered_references = {
        item.get("source_reference")
        for item in [*entries, *omissions]
        if isinstance(item, dict)
    }
    for domain in APP_MAP_DOMAINS:
        check(
            f"bundle:app_map#/{domain}" in covered_references,
            f"projection does not cover App Map domain: {domain}",
        )

    projection_path = PACK_ROOT / EXPECTED_BUNDLE["app_map_projection"]
    if not projection_path.exists():
        check(False, "memory/app_map.v1.json: projection file is missing")
        return
    check(
        projection_path.read_bytes() == canonical_projection(app_map),
        "memory/app_map.v1.json: canonical projection byte drift",
    )
    check(projection.get("app_id") == APP_ID, "projection app_id mismatch")


def validate_selectors(selectors: dict[str, Any]) -> None:
    """Lock selector scope to shell and modal boundaries only."""
    check(
        selectors.get("selector_model") == "uia_candidate_chain_v1",
        "selector model mismatch",
    )
    check(selectors.get("app_version_range") == VERSION_RANGE, "selector version mismatch")
    check(
        selectors.get("probe_status") == "provisional",
        "selector pack must remain provisional",
    )
    policy = selectors.get("policy", {})
    check(policy.get("on_ambiguous") == "error_and_ask", "selector ambiguity policy mismatch")
    check(
        policy.get("on_not_found", {}).get("retry_after_ms") == [250, 1000],
        "selector retry policy mismatch",
    )

    targets = selectors.get("targets", [])
    target_ids = [target.get("id") for target in targets if isinstance(target, dict)]
    check(target_ids == EXPECTED_TARGETS, "selector target set or order mismatch")
    targets_by_id = {target.get("id"): target for target in targets}
    for target_id, expected_shape in EXPECTED_TARGET_SHAPES.items():
        target = targets_by_id.get(target_id, {})
        scope, class_name, role = expected_shape
        check(target.get("scope") == scope, f"selector target {target_id}: scope mismatch")
        class_candidates = [
            candidate
            for candidate in target.get("candidates", [])
            if candidate.get("kind") == "class_and_role"
            and candidate.get("fallback_only") is False
        ]
        check(
            bool(class_candidates),
            f"selector target {target_id}: primary class_and_role required",
        )
        class_value = (
            class_candidates[0].get("value", {}).get("class_and_role", {})
            if class_candidates
            else {}
        )
        check(
            class_value == {"class": class_name, "role": role},
            f"selector target {target_id}: exact shell identity mismatch",
        )
        check(
            target.get("probe_status") == "required",
            f"selector target {target_id}: must remain required",
        )
        candidates = target.get("candidates", [])
        check(bool(candidates), f"selector target {target.get('id')}: candidates missing")
        check(
            any(candidate.get("fallback_only") is False for candidate in candidates),
            f"selector target {target.get('id')}: primary candidate required",
        )
        for candidate in candidates:
            if candidate.get("fallback_only") is True:
                check(
                    candidate.get("locale_dependent") is True,
                    f"selector target {target.get('id')}: fallback must be localized",
                )

    serialized = json.dumps(selectors, ensure_ascii=False).lower()
    for forbidden in ("cell", "grid", "row", "column"):
        check(forbidden not in serialized, f"selector contains forbidden grid scope: {forbidden}")


def validate_empty_surfaces(
    adapter: dict[str, Any],
    app_map: dict[str, Any],
    tools_document: dict[str, Any],
    rollback: dict[str, Any],
    capability_entries: list[dict[str, Any]] | None = None,
) -> None:
    """Prevent fake COM tools before the runner and exact envelopes exist."""
    check(
        tools_document.get("tools") == [],
        "tools must remain empty pending COM runner and Tool implementations",
    )
    check(rollback.get("recipes") == [], "rollback must remain empty without write tools")
    boundaries = app_map.get("safety_boundaries", {})
    check(
        boundaries.get("model_visible_tools_declared") is False,
        "App Map must declare zero model-visible tools",
    )
    check(
        boundaries.get("tools_blocked_by") == EXPECTED_TOOLS_BLOCKER,
        "App Map implementation blocker mismatch",
    )
    safety = adapter.get("safety", {})
    check(safety.get("allow_unattended") is False, "adapter must forbid unattended writes")
    check(
        safety.get("forbidden_properties") == ["DisplayAlerts=false"],
        "DisplayAlerts=false must remain explicitly forbidden",
    )
    if capability_entries is None:
        try:
            capability_catalog = json.loads(
                (
                    REPO_ROOT
                    / "protocol/capability-matrix/capability-1.0.json"
                ).read_text(encoding="utf-8")
            )
            capability_entries = capability_catalog.get("capabilities", [])
        except (OSError, json.JSONDecodeError) as error:
            check(False, f"capability catalog cannot be read: {error}")
            capability_entries = []
    capabilities = {
        entry.get("id")
        for entry in capability_entries
        if isinstance(entry, dict)
    }
    com_capabilities = {
        capability
        for capability in capabilities
        if isinstance(capability, str) and capability.startswith("com.")
    }
    check(bool(com_capabilities), "ADR-0107 COM capability vocabulary is missing")
    check(
        not any(
            isinstance(capability, str) and capability.startswith(LEGACY_CAPABILITY_PREFIXES)
            for capability in capabilities
        ),
        "legacy office/spreadsheet capability aliases are forbidden",
    )
    check(
        not (com_capabilities & FORBIDDEN_COM_CAPABILITIES),
        "broad COM escape capability is forbidden",
    )
    interrupt_policy = app_map.get("interrupt_policy", {})
    check(
        interrupt_policy.get("default_action") == "cancel",
        "App Map interrupt default must be cancel",
    )
    check(
        set(interrupt_policy.get("human_only_classes", [])) == EXPECTED_HUMAN_CLASSES,
        "App Map human-only interrupt set mismatch",
    )
    check(
        set(interrupt_policy.get("dangerous_actions_never_automatic", []))
        == EXPECTED_DANGEROUS_ACTIONS,
        "App Map dangerous action denylist mismatch",
    )
    check(
        boundaries.get("display_alerts_false_forbidden") is True,
        "App Map must forbid DisplayAlerts=false",
    )
    check(
        boundaries.get("allow_existing_excel_process_reuse") is False,
        "App Map must reject existing Excel process reuse",
    )
    check(
        boundaries.get("allow_unattended") is False,
        "App Map must forbid unattended operation",
    )
    check(
        boundaries.get("real_gui_verified") is False,
        "Data-only pack must not claim real GUI verification",
    )
    connection = app_map.get("connection", {})
    check(
        connection.get("instance_policy") == "create_isolated_instance_only",
        "App Map isolated-instance policy mismatch",
    )
    check(
        connection.get("existing_process_policy") == "fail_closed",
        "App Map existing-process policy must fail closed",
    )
    check(connection.get("requires_journal") is True, "App Map journal requirement missing")
    check(connection.get("requires_watchdog") is True, "App Map watchdog requirement missing")
    known_pitfalls = " ".join(app_map.get("known_pitfalls", []))
    check(
        "process identity" in known_pitfalls.lower(),
        "App Map must require dialog process identity",
    )
    readme = " ".join(
        (PACK_ROOT / "README.md").read_text(encoding="utf-8").lower().split()
    )
    check("zero model-visible tools" in readme, "README must disclose the empty tool surface")
    check("adr-0107" in readme, "README must name the approved capability ADR")


def validate_interrupts(interrupts: dict[str, Any]) -> None:
    """Lock every Excel safety boundary to an explicit human decision."""
    check(interrupts.get("default_action") == "cancel", "interrupt default action must cancel")
    entries = [
        entry for entry in interrupts.get("interrupts", []) if isinstance(entry, dict)
    ]
    by_id = {entry.get("id"): entry for entry in entries}
    check(set(by_id) == set(EXPECTED_INTERRUPTS), "interrupt set mismatch")

    priorities = [entry.get("priority") for entry in entries]
    check(len(priorities) == len(set(priorities)), "interrupt priorities must be unique")
    check(
        all(isinstance(priority, int) and priority > 0 for priority in priorities),
        "interrupt priorities must be positive integers",
    )

    for interrupt_id, (classification, note_fragment) in EXPECTED_INTERRUPTS.items():
        entry = by_id.get(interrupt_id, {})
        check(
            entry.get("classification") == classification,
            f"interrupt {interrupt_id}: classification mismatch",
        )
        check(
            entry.get("action") == {"kind": "ask_human"},
            f"interrupt {interrupt_id}: action must ask_human",
        )
        check(
            entry.get("requires_human") is True,
            f"interrupt {interrupt_id}: requires_human must be true",
        )
        check(
            entry.get("resume_after") == "user_decision",
            f"interrupt {interrupt_id}: resume_after must be user_decision",
        )
        match = entry.get("match", {})
        check(
            match.get("class_name") == EXPECTED_INTERRUPT_CLASSES[interrupt_id],
            f"interrupt {interrupt_id}: exact class_name anchor mismatch",
        )
        title_candidates = match.get("title_candidates", [])
        check(
            bool(title_candidates)
            and all(
                isinstance(candidate, dict)
                and candidate.get("locale_dependent") is True
                for candidate in title_candidates
            ),
            f"interrupt {interrupt_id}: localized title candidates required",
        )
        notes = " ".join(entry.get("notes", []))
        check(
            note_fragment.lower() in notes.lower(),
            f"interrupt {interrupt_id}: missing safety note `{note_fragment}`",
        )
        check(
            "provisional" in notes.lower(),
            f"interrupt {interrupt_id}: match must disclose provisional evidence",
        )

    serialized = json.dumps(interrupts, ensure_ascii=False).lower()
    check("delegate_to_task_" not in serialized, "historical task delegation remains")
    check('"kind": "save"' not in serialized, "automatic save action remains")
    check('"kind": "replace"' not in serialized, "automatic replace action remains")


def main() -> int:
    """Run all checks or render the projection for deterministic review."""
    try:
        adapter = tomllib.loads((PACK_ROOT / "adapter.toml").read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"FAILED: adapter.toml: {error}")
        return 1

    app_map = load_json(EXPECTED_BUNDLE["app_map"])
    if "--print-projection" in sys.argv:
        sys.stdout.buffer.write(canonical_projection(app_map))
        return 0

    projection = load_json(EXPECTED_BUNDLE["app_map_projection"])
    selectors = load_json(EXPECTED_BUNDLE["selectors"])
    tools_document = load_json(EXPECTED_BUNDLE["tools"])
    rollback = load_json(EXPECTED_BUNDLE["rollback"])
    interrupts = load_json(EXPECTED_BUNDLE["interrupts"])

    check(adapter.get("schema_version") == 1, "adapter schema_version mismatch")
    check(adapter.get("adapter", {}).get("app_id") == APP_ID, "adapter app_id mismatch")
    check(
        adapter.get("adapter", {}).get("version_range") == VERSION_RANGE,
        "adapter version range mismatch",
    )
    check(adapter.get("bundle") == EXPECTED_BUNDLE, "bundle envelope mismatch")
    check(app_map.get("version_range") == VERSION_RANGE, "App Map version range mismatch")

    for label, document in (
        ("app_map.json", app_map),
        ("selectors/targets.json", selectors),
        ("tools/tools.json", tools_document),
        ("rollback/recipes.json", rollback),
        ("interrupts/interrupts.json", interrupts),
    ):
        check(document.get("app_id") == APP_ID, f"{label}: app_id mismatch")
        check(document.get("schema_version") == 1, f"{label}: schema_version mismatch")

    validate_projection(app_map, projection)
    validate_selectors(selectors)
    validate_empty_surfaces(adapter, app_map, tools_document, rollback)
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
        "tools=0 "
        "recipes=0 "
        f"interrupts={len(EXPECTED_INTERRUPTS)} "
        "probe_status=provisional"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
