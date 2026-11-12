# Stage 1 T5.2 ten-run human approval audit

Date: 2026-11-12
Task: TASK-367
Evidence: `eval/tasks/edge/t5.2/evidence/2026-11-12-task-367-ten-run/summary.json`

## 1. Scope

This audit records ten human-present T5.2 approval rounds using the formal
desktop-ui, a real local Edge instance, the authenticated Core UI session, and the
zero-dependency loopback form fixture.

The agent built and observed the environment but did not type the irreversible
confirmation phrase, select `Once`, click `Approve once`, or otherwise approve a
round. Each approval was performed by the human in the desktop-ui window.

## 2. Evidence layout

Each counted round has an independent directory:

```text
run-01/ ... run-10/
  core/command.txt
  core/pid.txt
  core/stdout.log
  core/stderr.log
  ui/pid.txt
  ui/stdout.log
  ui/stderr.log
  human/pre-approval.txt
  human/result.json          # present for the nine successful rounds
  server/stderr.log
  server/shutdown.log
  summary.json
```

Two pre-approval technical attempts were retained separately under
`preapproval-failures/`. They are not counted as human rounds:

- `run-01-attempt-01`: the outer runner used port 8871 while the committed task
  package fixes the URL to port 8766; navigation failed before a UI appeared.
- `run-01-attempt-02`: a direct Cargo build launched the Tauri development URL
  instead of embedded frontend assets; the approval card did not render.

After the Tauri `--debug --no-bundle` build, `run-01` through `run-10` each started
a formal approval card.

## 3. Aggregate result

```text
requested_runs=10
passed_runs=9
failed_runs=1
success_rate=0.9
minimum_success_rate=0.75
result=passed
```

The nine successful rounds each satisfy all of the following:

- exactly one committed `submit_form` step;
- `submitted=true`;
- the query-free local `post_url`;
- the fixed `submitted-marker` success page;
- no result file before the human decision;
- no token value in evidence;
- no remaining fixture listener, Core, desktop-ui, or dedicated Edge process.

## 4. Failed round

Round 4 is retained as the single failed round. It is not counted as a pass and
was not retried.

Observed sequence:

1. The human approval card was shown and the human decision produced exactly one
   successful `GET /t5.2/submitted?...` request.
2. Core then failed its Edge cleanup path with:
   `TargetUnresponsive: taskkill returned exit code: 255, but the browser root was reaped`.
3. Core exited before writing `human/result.json`.
4. The fixture listener and dedicated Edge process tree had converged by the time
   the failure evidence was inspected.

This is a real post-submit cleanup failure, not a fabricated success and not a
pre-approval failure. It remains visible in the aggregate `rounds[]` with
`passed=false`.

The responsible production branch is
`crates/platform/windows/src/browser_launcher.rs::terminate_child_tree`: when the
root process has already been reaped, a non-zero `taskkill` status is still
returned as `TargetUnresponsive`. Fixing that production behavior is outside
TASK-367's write scope and should be handled by a separate card.

## 5. Late shutdown log

Round 10's single-run summary is `passed`, but its first shutdown snapshot recorded:

```text
stopped=False
port_8766_listener=True
```

Immediately afterward the outer bounded convergence check observed:

```text
listener=false
core=0
ui=0
dedicated_edge=0
```

The original shutdown log remains unchanged. The aggregate records the actual
bounded convergence result rather than rewriting the child log.

## 6. Matrix result

The T5.2 evidence adapter is now `round_summary` and consumes the ten-run summary.
The Stage 1 validator reports:

```text
stage1_complete=true
completed_tasks=9
total_tasks=9
missing_tasks=[]
T5.2 observed_runs=10
T5.2 passed_runs=9
T5.2 success_rate=0.9
```

Both commands are green:

```text
python eval/stage-1/validate_acceptance_matrix.py
python eval/stage-1/validate_acceptance_matrix.py --require-complete
```

## 7. Safety checks

- The agent did not perform any desktop-ui approval input.
- The evidence directory contains no token assignment or bearer credential match.
- Every counted round has `token_logged=false`.
- Every counted round has `pre_approval_result_exists=false`.
- The ten-round summary has no row whose submit observation is missing.
- All ten rounds have `fixture_listener_after_run=false` and zero residual
  Core/UI/dedicated Edge processes by the bounded final check.
- Pre-approval setup failures were retained but excluded from the human-round count.

## 8. Verdict

**T5.2 repeated-run acceptance: PASS at 9/10.**

The Stage 1 nine-task repeated-run matrix is now **9/9 complete** at the documented
thresholds. The Stage 1 matrix no longer blocks phase closure.

The round-4 Edge cleanup status remains a production-quality residual risk. It is
visible in evidence and requires a separate bounded repair card; it must not be
silently normalized or removed from this summary.

## 9. Merge evidence

- PR #722 checks: **11/11 success**, pending 0, failed 0.
- Base: `main`; merge state: `MERGEABLE` / `CLEAN`.
- Merge commit: `367f4e5b24b0646d953494cd08723c07ea89f8f8`.
- Post-merge main CI: `43173799070` success.
- All three platform `check` jobs executed the Stage 1 matrix tests and report.
