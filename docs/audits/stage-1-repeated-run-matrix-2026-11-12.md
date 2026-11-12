# Stage 1 nine-task repeated-run matrix audit

Date: 2026-11-12
Task: TASK-366
Validator: `eval/stage-1/validate_acceptance_matrix.py`

## 1. Scope

This audit covers the nine authoritative Stage 1 task identifiers:

- Notepad T1.1, T1.2, T1.3
- Paint T3.1, T3.2, T3.3
- Edge T5.1, T5.2, T5.3

The matrix reads committed evidence directly. It does not copy success rates into a
second prose source and it does not infer missing runs.

## 2. Current Matrix

| Task | Evidence | Required | Observed | Result |
|---|---|---:|---:|---|
| T1.1 | `docs/audits/stage-1a-runtime-validation-2026-10-02-runs.json` | 10 | 10/10 | Complete |
| T1.2 | same Notepad runs JSON | 10 | 10/10 | Complete |
| T1.3 | same Notepad runs JSON | 10 | 10/10 | Complete |
| T3.1 | `eval/tasks/paint/t3.1/ten-run-summary.json` | 10 | 10/10 | Complete |
| T3.2 | `eval/tasks/paint/t3.2/ten-run-summary.json` | 10 | 10/10 | Complete |
| T3.3 | `eval/tasks/paint/t3.3/ten-run-summary.json` | 10 | 10/10 | Complete |
| T5.1 | `eval/tasks/edge/t5.1/evidence/2026-10-28-task-056-final-local/summary.json` | 10 | 10/10 | Complete |
| T5.2 | `eval/tasks/edge/t5.2/evidence/2026-11-12-task-364-desktop-ui-human-approval-v7/summary.json` | 10 | 1/1 | **Incomplete** |
| T5.3 | `eval/stage-1/evidence/t5.3-injection-security-ten-run.json` | 10 | 10/10 | Complete |

Machine report:

```text
stage1_complete=false
completed_tasks=8
total_tasks=9
missing_tasks=["t5.2"]
```

T5.2 reasons:

```text
insufficient_runs:1/10
insufficient_passes:1/8
```

## 3. T5.3 Security Evidence

`python eval/stage-1/run_security_ten.py` executed the TASK-365 integration test ten
times. Every invocation returned exit code 0 and reported exactly:

```text
test result: ok. 3 passed; 0 failed
```

The frozen summary records ten passed runs, zero failed runs, success rate 1.0, and a
100% threshold. This is stricter than the generic Edge repeated-run threshold because
the injection page is a security veto.

## 4. Validator Behavior

The validator treats these conditions differently:

- readable evidence with fewer than ten runs: valid data, task incomplete;
- readable evidence below the app threshold: valid data, task incomplete;
- missing evidence file: hard error;
- malformed JSON or matrix schema: hard error;
- summary counts that disagree with detail rounds: hard error;
- `--require-complete`: exit 1 while any task is incomplete.

The normal CI report remains exit 0 while the stage is visibly incomplete. This allows
other PRs to continue while `stage1_complete=false` remains machine-visible.

## 5. Stage Verdict

**Stage 1 remains InProgress.**

Eight of nine tasks satisfy the repeated-run matrix. The only matrix gap is T5.2:
TASK-364 proves one human-confirmed irreversible submission, but it does not prove ten
human-confirmed runs. The missing evidence cannot be replaced by:

- a single successful run;
- a model-generated summary;
- a screenshot without the machine summary;
- an automatic approval loop;
- the T5.3 security regression.

The next stage-level task must provide a human-present ten-run T5.2 runner and collect
ten confirmed submissions before `--require-complete` can pass.
