"""Run the TASK-365 injection-security regression ten times."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = (
    Path(__file__).parent
    / "evidence"
    / "t5.3-injection-security-ten-run.json"
)


def run_once(run_number: int) -> dict[str, object]:
    command = [
        "cargo",
        "test",
        "-p",
        "assistant-agent-core",
        "--test",
        "injection_security",
    ]
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    passed = (
        completed.returncode == 0
        and "test result: ok. 3 passed; 0 failed" in completed.stdout
    )
    return {
        "run": run_number,
        "exit_code": completed.returncode,
        "passed": passed,
    }


def write_summary(output: Path, runs: int) -> dict[str, object]:
    records = [run_once(run_number) for run_number in range(1, runs + 1)]
    passed_runs = sum(1 for record in records if record["passed"])
    summary = {
        "schema_version": 1,
        "task_id": "t5.3",
        "mode": "injection-security-regression",
        "command": (
            "cargo test -p assistant-agent-core --test injection_security"
        ),
        "requested_runs": runs,
        "passed_runs": passed_runs,
        "failed_runs": runs - passed_runs,
        "success_rate": passed_runs / runs if runs else 0.0,
        "minimum_success_rate": 1.0,
        "result": "passed" if passed_runs == runs else "failed",
        "rounds": records,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + ".tmp")
    temporary.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    temporary.replace(output)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    if args.runs <= 0:
        parser.error("--runs must be positive")
    summary = write_summary(args.output, args.runs)
    print(
        "t5.3-security "
        f"runs={summary['requested_runs']} passed={summary['passed_runs']} "
        f"failed={summary['failed_runs']} result={summary['result']}"
    )
    return 0 if summary["result"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
