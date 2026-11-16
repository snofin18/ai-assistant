# Stage 2 Excel COM lifecycle evidence

Status: **Done**. Date: 2026-11-16.
Related: TASK-375, TASK-372, ADR-0063.

## 1. Scope

This task extends the existing Excel COM preflight with an isolated lifecycle
regression. It does not implement the production COM runner, Adapter pack,
public schema, or T2.1/T2.2/T2.3. The public Adapter ADR/spec decision remains
a human gate.

The regression refuses to start if any `EXCEL.EXE` already exists. It then
drives five isolated cases through child `pwsh` processes:

| Case | Expected exit | Expected result |
|---|---:|---|
| Normal formula probe | 0 | `passed` |
| Injected exception after workbook create | 2 | `failed` / `injected_failure` |
| Cooperative cancellation after workbook create | 3 | `cancelled` |
| Intentional hang after workbook create | watchdog timeout | verified watchdog cleanup |
| Existing Excel refusal | 2 | `failed` / `existing_process` |

## 2. Real run

Command:

```powershell
pwsh -NoProfile -File eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1 `
  -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-375
```

Result:

```text
excel-com-lifecycle passed scenarios=5 final_excel_process_count=0
```

Observed probes:

| Case | Probe PID | Close / Quit / release | Probe processes after | Case result |
|---|---:|---|---:|---|
| Normal | 41556 | success / success / success | 0 | `passed` |
| Injected failure | 38696 | success / success / success | 0 | `failed` as injected |
| Cooperative cancel | 29612 | success / success / success | 0 | `cancelled` |
| Watchdog hang | 32300 | owned PID killed only after identity recheck | 0 | timeout recovered |
| Existing refusal | none created | fail-closed before COM creation | 1 pre-existing | `failed` as intended |

The existing-process case used a temporary, unsaved bootstrap workbook with
PID 7380. The probe recorded and refused that PID without terminating it;
the aggregate evidence records `bootstrap_process_alive_after_refusal=true`.
A separate bounded helper owned the bootstrap instance and exited successfully
after its own cleanup. The aggregate run ended with
`final_excel_process_count=0`.

An earlier implementation inferred ownership from the difference between
process snapshots. Independent review correctly rejected that design because a
concurrently started Excel could be misclassified. The final implementation
maps `Application.Hwnd` to a PID, records PID/start-time/executable-path in an
ownership journal, and revalidates that identity before any fallback kill.

## 3. Failure injection and cancellation

The probe exposes three test-only controls:

- `-FailureInjectionPoint` throws at a named checkpoint;
- `-CancellationSignalPath` plus `-CancellationCheckpoint` requests cooperative
  cancellation at a named safe point.
- `-HangAfterCheckpoint` intentionally hangs so the outer watchdog can prove
  bounded, ownership-verified cleanup.

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
- Probe ownership comes from `Application.Hwnd`, not a PID-set subtraction.
- Every fallback kill rechecks PID, process start ticks, process name, and
  executable path immediately before invoking `taskkill`.
- Existing Excel is never terminated by the probe.
- Probe waits and taskkill invocations are bounded. Synchronous COM calls are
  bounded from outside by the parent watchdog rather than misleadingly claimed
  to be interruptible.
- Final real run: all five scenarios passed their expected exit/result pair
  and `final_excel_process_count=0`.

## 5. Limitations

- Cooperative cancellation is limited to an explicit probe checkpoint. The
  watchdog covers an externally observed child timeout, not cancellation
  inside a synchronous COM call. It does not claim recovery from machine crash
  or console-session loss.
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

- Implementation PR: #740, checks **11/11 success**, pending 0 / failed 0,
  base `main`, `MERGEABLE` / `CLEAN`.
- Merge commit: `c5cb52fde9d21c82ed922c7d459d5b911bac327b`.
- Post-merge main CI: `43894628610` success.
