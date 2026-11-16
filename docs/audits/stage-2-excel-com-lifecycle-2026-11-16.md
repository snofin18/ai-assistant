# Stage 2 Excel COM lifecycle evidence

Status: **Review pending PR / merge / post-merge CI**. Date: 2026-11-16.
Related: TASK-375, TASK-372, ADR-0063.

## 1. Scope

This task extends the existing Excel COM preflight with an isolated lifecycle
regression. It does not implement the production COM runner, Adapter pack,
public schema, or T2.1/T2.2/T2.3. The public Adapter ADR/spec decision remains
a human gate.

The regression refuses to start if any `EXCEL.EXE` already exists. It then
drives four isolated cases through child `pwsh` processes:

| Case | Expected exit | Expected result |
|---|---:|---|
| Normal formula probe | 0 | `passed` |
| Injected exception after workbook create | 2 | `failed` / `injected_failure` |
| Cooperative cancellation after workbook create | 3 | `cancelled` |
| Existing Excel refusal | 2 | `failed` / `existing_process` |

## 2. Real run

Command:

```powershell
pwsh -NoProfile -File eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1 `
  -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-375
```

Result:

```text
excel-com-lifecycle passed scenarios=4 final_excel_process_count=0
```

Observed probes:

| Case | Probe PID | Close / Quit / release | Probe processes after | Case result |
|---|---:|---|---:|---|
| Normal | 5588 | success / success / success | 0 | `passed` |
| Injected failure | 1140 | success / success / success | 0 | `failed` as injected |
| Cooperative cancel | 23760 | success / success / success | 0 | `cancelled` |
| Existing refusal | none created | fail-closed before COM creation | 1 pre-existing | `failed` as intended |

The existing-process case used a temporary, unsaved bootstrap workbook with
PID 24616. The probe recorded and refused that PID without terminating it.
The harness then closed its own workbook and Quit call, but bounded PID
convergence still required its own `taskkill /T /F`; exit code 0 was recorded
and the final owned-process count converged to zero. The aggregate run ended
with `final_excel_process_count=0`.

## 3. Failure injection and cancellation

The probe exposes two test-only controls:

- `-FailureInjectionPoint` throws at a named checkpoint;
- `-CancellationSignalPath` plus `-CancellationCheckpoint` requests cooperative
  cancellation at a named safe point.

Normal success assertions are unchanged. Both controls are visible in
`summary.json`, including the selected checkpoint.

An earlier run used `after-com-create` for cancellation. A pre-created signal
then caused `Application.Quit()` to block before Excel had finished
initialising. The evidence preserved this as a lifecycle fact; the final
cooperative cancellation case uses `after-workbook-create`, the same
initialised shape as the successful exception case.

## 4. Resource evidence

- Every run records workbook close, Quit, release, taskkill, remaining PID,
  and final process-count fields.
- Probe-created Excel PIDs are derived after startup by subtracting the
  pre-existing PID set.
- Existing Excel is never terminated by the probe.
- Every wait is bounded by `CleanupTimeoutSeconds`.
- Final real run: all four scenarios passed their expected exit/result pair
  and `final_excel_process_count=0`.

## 5. Limitations

- Cancellation is cooperative at an explicit probe checkpoint. It does not
  claim cleanup after `TerminateProcess`, console-session loss, machine crash,
  or a wedged COM call that never returns.
- This is an evaluation harness, not the production runner. Product COM access
  still belongs in `crates/platform/windows` behind existing platform traits.
- Protected View, AutoSave=true, user workbooks, read-only/occupied files,
  modal dialogs, and multimodal Workbook behaviour remain untested.
- Public Adapter contract decisions D1-D6 remain for human adjudication.

## 6. Gate evidence

- `cargo fmt --all --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed; the existing unknown
  `clippy::assert_is_empty` warning remains non-fatal as recorded by the repo.
- `cargo test --workspace`: 510 passed, 0 failed; existing real-machine tests
  remained ignored by their TASK references.
- `verify-schemas`, `codegen --check`, `hygiene`, `card-check`, `docscan`,
  `refscan`, `memory-counts`, `check-ledger`, `adr-index`, `check-comments`,
  architecture tests, core replay, and `cargo deny check` all passed.
- `cargo llvm-cov --fail-under-lines 75`: 79.76% line coverage.
- `apps/desktop-ui`: `pnpm lint`, `pnpm typecheck`, model tests 90/90, and
  Vitest 5/5 passed.

PR number, checks, merge commit, and post-merge main CI are intentionally
pending until the implementation and docs-only closeout PRs complete.
