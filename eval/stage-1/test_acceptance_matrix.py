"""Tests for the Stage 1 repeated-run acceptance matrix."""

from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

import validate_acceptance_matrix as matrix


class AcceptanceMatrixTests(unittest.TestCase):
    def test_current_matrix_has_exact_nine_task_contract(self) -> None:
        loaded = matrix.load_matrix()
        self.assertEqual(
            [task["task_id"] for task in loaded["tasks"]],
            list(matrix.EXPECTED_TASKS),
        )

    def test_current_matrix_reports_only_t5_2_gap(self) -> None:
        report = matrix.evaluate_matrix(matrix.load_matrix())
        self.assertFalse(report["stage1_complete"])
        self.assertEqual(report["completed_tasks"], 8)
        self.assertEqual(report["total_tasks"], 9)
        self.assertEqual(report["missing_tasks"], ["t5.2"])
        by_task = {task["task_id"]: task for task in report["tasks"]}
        self.assertEqual(by_task["t1.1"]["minimum_passed_runs"], 9)
        self.assertEqual(by_task["t3.1"]["minimum_passed_runs"], 8)
        self.assertEqual(by_task["t5.3"]["minimum_passed_runs"], 10)
        self.assertIn(
            "insufficient_runs:1/10",
            by_task["t5.2"]["reasons"],
        )

    def test_round_summary_rejects_forged_round_count(self) -> None:
        evidence = {
            "requested_runs": 10,
            "passed_runs": 10,
            "success_rate": 1.0,
            "result": "passed",
            "rounds": [{"run": 1, "passed": False}],
        }
        with self.assertRaises(matrix.MatrixError):
            matrix._round_result("t3.1", "fixture.json", evidence, 10)

    def test_round_summary_rejects_mismatched_success_rate(self) -> None:
        evidence = {
            "requested_runs": 10,
            "passed_runs": 1,
            "success_rate": 1.0,
            "result": "passed",
            "rounds": [{"run": index, "passed": index == 1} for index in range(1, 11)],
        }
        with self.assertRaises(matrix.MatrixError):
            matrix._round_result("t3.1", "fixture.json", evidence, 10)

    def test_low_success_is_valid_evidence_but_incomplete(self) -> None:
        evidence = {
            "requested_runs": 10,
            "passed_runs": 1,
            "success_rate": 0.1,
            "result": "failed",
            "rounds": [{"run": index, "passed": index == 1} for index in range(1, 11)],
        }
        observed = matrix._round_result("t3.1", "fixture.json", evidence, 10)
        self.assertEqual(observed["observed_runs"], 10)
        self.assertEqual(observed["passed_runs"], 1)
        self.assertIn("result:failed", observed["reasons"])

    def test_notepad_adapter_counts_real_detail_records(self) -> None:
        data = matrix._load_json(
            matrix.ROOT
            / "docs/audits/stage-1a-runtime-validation-2026-10-02-runs.json"
        )
        result = matrix._notepad_result("t1.2", data, 10)
        self.assertEqual(result["observed_runs"], 10)
        self.assertEqual(result["passed_runs"], 10)
        self.assertEqual(result["reasons"], [])

    def test_single_human_approval_is_readable_but_short(self) -> None:
        data = matrix._load_json(
            matrix.ROOT
            / "eval/tasks/edge/t5.2/evidence/"
            "2026-11-12-task-364-desktop-ui-human-approval-v7/summary.json"
        )
        result = matrix._single_human_result("t5.2", data, 10)
        self.assertEqual(result["observed_runs"], 1)
        self.assertEqual(
            result["reasons"],
            ["insufficient_runs:1/10"],
        )

    def test_missing_evidence_is_hard_error(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing.json"
            with self.assertRaises(matrix.MatrixError):
                matrix._load_json(missing)

    def test_duplicate_task_id_is_rejected(self) -> None:
        data = copy.deepcopy(matrix.load_matrix())
        data["tasks"].append(copy.deepcopy(data["tasks"][0]))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaises(matrix.MatrixError):
                matrix.load_matrix(path)

    def test_unknown_task_id_is_rejected(self) -> None:
        data = copy.deepcopy(matrix.load_matrix())
        data["tasks"][0]["task_id"] = "t9.9"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaises(matrix.MatrixError):
                matrix.load_matrix(path)


if __name__ == "__main__":
    unittest.main()
