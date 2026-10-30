#!/usr/bin/env python3
"""Validate the declarative Edge and Chrome adapter packs.

This checker is intentionally dependency-free. It verifies the cross-file
contracts that JSON syntax checking alone cannot catch: app identity, tool
references, safety defaults, site blacklists, and human-only interrupts.
"""

from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[3]
ADAPTER_ROOT = REPO_ROOT / "adapters"

PACKS = {
    "browser.edge": {
        "path": ADAPTER_ROOT / "browser.edge",
        "app_id": "com.microsoft.edge",
        "tool_prefix": "edge.",
    },
    "browser.chrome": {
        "path": ADAPTER_ROOT / "browser.chrome",
        "app_id": "com.google.chrome",
        "tool_prefix": "chrome.",
    },
}

REQUIRED_BLACKLIST_KINDS = {"bank", "payment", "password_change", "captcha"}
REQUIRED_INTERRUPTS = {"cookie-banner", "login-wall", "captcha"}
REQUIRED_TOOLS = {
    "cdp.navigate",
    "cdp.dom_read",
    "cdp.form_fill",
    "cdp.form_read",
    "cdp.form_submit",
    "cdp.download",
    "cdp.screenshot",
    "cdp.wait_network_idle",
    "shell.address_bar_read",
    "shell.tabs_read",
    "shell.download_bar_read",
}


class ValidationFailure(Exception):
    """A pack contract violation."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationFailure(message)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as failure:
        raise ValidationFailure(f"{path}: {failure}") from failure
    require(isinstance(value, dict), f"{path}: root must be an object")
    return value


def load_toml(path: Path) -> dict[str, Any]:
    try:
        value = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as failure:
        raise ValidationFailure(f"{path}: {failure}") from failure
    require(isinstance(value, dict), f"{path}: root must be an object")
    return value


def validate_pack(name: str, expected_app_id: str, tool_prefix: str, pack_path: Path) -> None:
    adapter = load_toml(pack_path / "adapter.toml")
    app_map = load_json(pack_path / "app_map.json")
    targets = load_json(pack_path / "selectors" / "targets.json")
    tools_document = load_json(pack_path / "tools" / "tools.json")
    interrupts = load_json(pack_path / "interrupts" / "interrupts.json")

    require(adapter.get("schema_version") == 1, f"{name}: adapter schema_version")
    adapter_entry = adapter.get("adapter", {})
    require(adapter_entry.get("app_id") == expected_app_id, f"{name}: adapter app_id")
    require(app_map.get("app_id") == expected_app_id, f"{name}: app_map app_id")
    require(targets.get("app_id") == expected_app_id, f"{name}: targets app_id")
    require(tools_document.get("app_id") == expected_app_id, f"{name}: tools app_id")
    require(interrupts.get("app_id") == expected_app_id, f"{name}: interrupts app_id")

    profile = adapter.get("profile", {})
    require(profile.get("dedicated_user_data_dir") is True, f"{name}: dedicated profile required")
    require(profile.get("copy_user_profile") is False, f"{name}: profile copy must be false")

    safety = adapter.get("safety", {})
    require(safety.get("default_interrupt_action") == "cancel", f"{name}: interrupt default")
    require(safety.get("allow_unattended") is False, f"{name}: unattended must be false")
    require(safety.get("site_blacklist") == "enforced", f"{name}: blacklist must be enforced")
    require(safety.get("password_input") == "never_automated", f"{name}: password automation")
    require(safety.get("captcha_bypass") == "forbidden", f"{name}: captcha bypass")

    tool_entries = tools_document.get("tools", [])
    require(isinstance(tool_entries, list), f"{name}: tools must be a list")
    tool_names = {
        entry.get("schema", {}).get("name")
        for entry in tool_entries
        if isinstance(entry, dict)
    }
    require(all(isinstance(tool_name, str) for tool_name in tool_names), f"{name}: tool names")
    expected_tool_names = {tool_prefix + suffix for suffix in REQUIRED_TOOLS}
    require(required_tools_match(tool_names, expected_tool_names), f"{name}: required tool set")

    command_tools = {
        command.get("tool")
        for command in app_map.get("commands", [])
        if isinstance(command, dict)
    }
    require(command_tools <= tool_names, f"{name}: app_map references an unknown tool")

    blacklist_kinds = {
        entry.get("kind")
        for entry in app_map.get("site_blacklist", [])
        if isinstance(entry, dict)
    }
    require(
        REQUIRED_BLACKLIST_KINDS <= blacklist_kinds,
        f"{name}: required site blacklist kinds",
    )

    interrupt_entries = interrupts.get("interrupts", [])
    require(isinstance(interrupt_entries, list), f"{name}: interrupts must be a list")
    interrupt_ids = {
        entry.get("id")
        for entry in interrupt_entries
        if isinstance(entry, dict)
    }
    require(REQUIRED_INTERRUPTS <= interrupt_ids, f"{name}: required interrupt set")
    require(
        all(
            entry.get("requires_human") is True
            for entry in interrupt_entries
            if isinstance(entry, dict)
        ),
        f"{name}: every interrupt must require a human",
    )
    require(
        all(
            entry.get("default_action") == "cancel"
            for entry in interrupt_entries
            if isinstance(entry, dict)
        ),
        f"{name}: every interrupt must default to cancel",
    )

    form_tool = find_tool(tool_entries, tool_prefix + "cdp.form_fill")
    submitted = (
        form_tool.get("schema", {})
        .get("output", {})
        .get("properties", {})
        .get("submitted", {})
        .get("const")
    )
    require(submitted is False, f"{name}: form fill must stop before submit")
    form_read = find_tool(tool_entries, tool_prefix + "cdp.form_read").get("schema", {})
    require(form_read.get("effect") == "read", f"{name}: form read effect")
    require(
        form_read.get("reversibility") == "none_readonly",
        f"{name}: form read reversibility",
    )
    form_submit_entry = find_tool(tool_entries, tool_prefix + "cdp.form_submit")
    form_submit = form_submit_entry.get("schema", {})
    require(form_submit.get("effect") == "write", f"{name}: form submit effect")
    require(
        form_submit.get("reversibility") == "l3_irreversible",
        f"{name}: form submit reversibility",
    )
    require(form_submit.get("risk_level") == "high", f"{name}: form submit risk")
    require(
        form_submit.get("requires_approval") is True,
        f"{name}: form submit approval",
    )
    require(
        form_submit.get("idempotent") is False,
        f"{name}: form submit idempotency",
    )
    form_submit_output = form_submit.get("output", {}).get("properties", {})
    require(
        form_submit_output.get("submitted", {}).get("const") is True,
        f"{name}: form submit success marker",
    )
    require(
        form_submit_output.get("fingerprint_scope", {}).get("const") == "cdp_target",
        f"{name}: form submit fingerprint scope",
    )
    require(
        set(form_submit.get("output", {}).get("required", []))
        == {
            "submitted",
            "post_url",
            "fingerprint_scope",
            "fingerprint",
            "previous_fingerprint",
            "elapsed_ms",
        },
        f"{name}: form submit observation fields",
    )
    postconditions = form_submit_entry.get("postconditions", [])
    require(
        postconditions
        == [
            {
                "kind": "state_changed",
                "fingerprint_scope": "cdp_target",
                "within_ms": 20000,
                "error_code": "VerifyFailed",
            }
        ],
        f"{name}: form submit postcondition",
    )


def find_tool(tool_entries: list[Any], name: str) -> dict[str, Any]:
    for entry in tool_entries:
        if isinstance(entry, dict) and entry.get("schema", {}).get("name") == name:
            return entry
    raise ValidationFailure(f"missing tool: {name}")


def required_tools_match(actual: set[str | None], expected: set[str]) -> bool:
    return actual == expected


def main() -> int:
    try:
        for name, config in PACKS.items():
            validate_pack(
                name,
                config["app_id"],
                config["tool_prefix"],
                config["path"],
            )
            print(f"PACK OK {name}")
    except ValidationFailure as failure:
        print(f"PACK FAIL {failure}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
