# Excel COM preflight

This directory contains the isolated Microsoft Excel COM probe for Stage 2.

The probe:

- refuses to run when any `EXCEL.EXE` process already exists;
- creates one new unsaved workbook and writes only temporary cells;
- reads `Value2`, `Formula`, and `Text` for a calculated formula;
- records `CanUndo` around programmatic writes and one explicit Undo call;
- reads Protected View count, AutoSave state, and the current `DisplayAlerts` value;
- derives the owned Excel PID from `Application.Hwnd` and records PID, start time, and
  executable path in an ownership journal before risky cleanup calls;
- closes the workbook with `SaveChanges=false`, quits Excel, releases COM objects, and
  verifies that all process IDs created by the probe converge to zero;
- never opens a user workbook and never changes `DisplayAlerts`.

Run:

```powershell
pwsh -NoProfile -File eval/tasks/excel/preflight/probe_excel_com.ps1 `
  -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-372
```

The probe is a Stage 2 environment check, not the production COM runner. Product COM access
must still be implemented inside `crates/platform/windows` behind the existing platform traits.

## Lifecycle regression

`test_excel_com_lifecycle.ps1` drives five isolated paths against the probe:

1. normal success;
2. an injected exception after the temporary workbook is created;
3. cooperative cancellation through a pre-created signal file;
4. an intentional hang that exceeds a 15-second watchdog and proves verified cleanup;
5. refusal when an Excel process already exists, including proof that the probe leaves that
   bootstrap process alive.

Run:

```powershell
pwsh -NoProfile -File eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1 `
  -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-375
```

The `-FailureInjectionPoint`, `-CancellationSignalPath`, `-CancellationCheckpoint`, and
`-HangAfterCheckpoint` probe parameters exist only for this regression. They do not weaken
the normal success assertions. Cancellation is tested after the temporary workbook exists
because a signal immediately after raw COM creation can leave `Application.Quit()` blocked
before Excel has finished initialising.

Synchronous COM calls cannot be interrupted inside one PowerShell process. The harness
therefore runs the probe and the bootstrap helper as bounded children. On timeout it reads
the ownership journal, revalidates PID/start-time/path, and only then terminates that Excel
identity and child process tree. This covers a wedged call from outside the probe; it does
not promise cooperative cancellation inside a COM call or recovery from machine crash.
