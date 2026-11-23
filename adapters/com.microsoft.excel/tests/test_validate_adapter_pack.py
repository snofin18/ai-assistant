#!/usr/bin/env python3
"""Negative regression tests for the Excel safety-shell validator."""

from __future__ import annotations

import copy
import importlib.util
import json
import unittest
from pathlib import Path
from types import ModuleType
from typing import Any


PACK_ROOT = Path(__file__).resolve().parents[1]
VALIDATOR_PATH = PACK_ROOT / "tests" / "validate_adapter_pack.py"


def load_validator() -> ModuleType:
    """Load the validator module without packaging the tests directory."""
    spec = importlib.util.spec_from_file_location("excel_pack_validator", VALIDATOR_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load Excel pack validator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


VALIDATOR = load_validator()
EXPECTED_EXCEL_CAPABILITIES = {
    "com.chart_create",
    "com.chart_export",
    "com.range_read",
    "com.range_write",
    "com.workbook_open",
    "com.workbook_read",
    "com.workbook_save",
}


def load_json(relative_path: str) -> dict[str, Any]:
    """Load one mutable JSON object from the pack."""
    return json.loads((PACK_ROOT / relative_path).read_text(encoding="utf-8"))


class ExcelPackNegativeTests(unittest.TestCase):
    """Prove that safety-bearing mutations fail closed."""

    def setUp(self) -> None:
        VALIDATOR.ERRORS.clear()

    def test_selector_scope_mutation_is_rejected(self) -> None:
        selectors = copy.deepcopy(load_json("selectors/targets.json"))
        selectors["targets"][0]["scope"] = "element"

        VALIDATOR.validate_selectors(selectors)

        self.assertTrue(VALIDATOR.ERRORS)

    def test_selector_class_mutation_is_rejected(self) -> None:
        selectors = copy.deepcopy(load_json("selectors/targets.json"))
        selectors["targets"][0]["candidates"][0]["value"]["class_and_role"] = {
            "class": "NotExcelShell",
            "role": "Window",
        }

        VALIDATOR.validate_selectors(selectors)

        self.assertTrue(VALIDATOR.ERRORS)

    def test_interrupt_class_mutation_is_rejected(self) -> None:
        interrupts = copy.deepcopy(load_json("interrupts/interrupts.json"))
        interrupts["interrupts"][0]["match"]["class_name"] = "NotExcelDialog"

        VALIDATOR.validate_interrupts(interrupts)

        self.assertTrue(VALIDATOR.ERRORS)

    def test_interrupt_provisional_evidence_removal_is_rejected(self) -> None:
        interrupts = copy.deepcopy(load_json("interrupts/interrupts.json"))
        interrupts["interrupts"][0]["notes"] = [
            "Enabling editing is a human security decision and must never be automated."
        ]

        VALIDATOR.validate_interrupts(interrupts)

        self.assertTrue(VALIDATOR.ERRORS)

    def test_app_map_process_reuse_mutation_is_rejected(self) -> None:
        app_map = copy.deepcopy(load_json("app_map.json"))
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        app_map["safety_boundaries"]["allow_existing_excel_process_reuse"] = True

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
        )

        self.assertTrue(VALIDATOR.ERRORS)

    def test_app_map_real_gui_claim_mutation_is_rejected(self) -> None:
        app_map = copy.deepcopy(load_json("app_map.json"))
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        app_map["safety_boundaries"]["real_gui_verified"] = True

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
        )

        self.assertTrue(VALIDATOR.ERRORS)

    def test_app_map_interrupt_default_mutation_is_rejected(self) -> None:
        app_map = copy.deepcopy(load_json("app_map.json"))
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        app_map["interrupt_policy"]["default_action"] = "dismiss_explicitly"

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
        )

        self.assertTrue(VALIDATOR.ERRORS)

    def test_app_map_implementation_blocker_mutation_is_rejected(self) -> None:
        app_map = copy.deepcopy(load_json("app_map.json"))
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        app_map["safety_boundaries"]["tools_blocked_by"] = (
            "spreadsheet_and_com_capability_ids_require_adr"
        )

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
        )

        self.assertTrue(VALIDATOR.ERRORS)

    def test_capability_catalog_locks_exact_excel_vocabulary(self) -> None:
        catalog = load_json("../../protocol/capability-matrix/capability-1.0.json")
        entries = {
            entry.get("id"): entry
            for entry in catalog.get("capabilities", [])
            if isinstance(entry, dict)
            and isinstance(entry.get("id"), str)
            and entry["id"].startswith("com.")
        }

        self.assertEqual(set(entries), EXPECTED_EXCEL_CAPABILITIES)
        for capability in EXPECTED_EXCEL_CAPABILITIES:
            self.assertEqual(entries[capability].get("stability"), "experimental")
            self.assertEqual(entries[capability].get("version"), "0.1.0")

    def test_legacy_capability_alias_is_rejected(self) -> None:
        app_map = load_json("app_map.json")
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        entries = [
            {"id": capability, "description": capability}
            for capability in EXPECTED_EXCEL_CAPABILITIES
        ]
        entries.append({"id": "office.execute", "description": "forbidden alias"})

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
            entries,
        )

        self.assertIn(
            "legacy office/spreadsheet capability aliases are forbidden",
            VALIDATOR.ERRORS,
        )

    def test_broad_com_escape_capability_is_rejected(self) -> None:
        app_map = load_json("app_map.json")
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        entries = [
            {"id": capability, "description": capability}
            for capability in EXPECTED_EXCEL_CAPABILITIES
        ]
        entries.append({"id": "com.execute", "description": "forbidden escape"})

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
            entries,
        )

        self.assertIn(
            "broad COM escape capability is forbidden",
            VALIDATOR.ERRORS,
        )

    def test_adapter_forbidden_property_mutation_is_rejected(self) -> None:
        app_map = load_json("app_map.json")
        adapter = VALIDATOR.tomllib.loads(
            (PACK_ROOT / "adapter.toml").read_text(encoding="utf-8")
        )
        tools_document = load_json("tools/tools.json")
        rollback = load_json("rollback/recipes.json")
        adapter["safety"]["forbidden_properties"] = []

        VALIDATOR.validate_empty_surfaces(
            adapter,
            app_map,
            tools_document,
            rollback,
        )

        self.assertTrue(VALIDATOR.ERRORS)


if __name__ == "__main__":
    unittest.main()
