# Excel T2.2 repeated write acceptance

This directory contains the human-present test-only acceptance harness for TASK-409.
It drives a local `.xlsx` fixture through fresh temporary copies and fresh owned Excel
processes. The production Excel Adapter remains empty.

The harness runs three separate scenarios:

1. ten normal repeated write cycles by default;
2. one injected failure after a real scalar write;
3. one intentional hang after writable open, terminated by an independent watchdog.

Every normal cycle uses a new `ExcelComRunner`, owned process, session, working copy, and
shadow copy. Before writable open and cleanup, the Rust test writes an ownership journal
containing PID, Windows creation FILETIME, executable path, and workbook path. The outer
PowerShell watchdog may only terminate an Excel process after revalidating all identity
fields. It never kills by PID alone.

The harness starts a small wrapper that is assigned to a Job Object before the wrapper
launches cargo. Cargo, the test executable, and any inherited descendants therefore enter
the Job before doing work. The wrapper records independent Excel observations for
diagnostics, but only journal identities or observed processes that are still members of
that Job are eligible for cleanup; an unrelated user Excel process is reported as a
conflict and is never killed.

Run:

```powershell
pwsh -NoProfile -File eval/tasks/excel/t22/test_excel_t22_repeated_write.ps1 `
  -SourceWorkbook '<explicit local .xlsx fixture>' `
  -EvidenceRoot eval/tasks/excel/t22/evidence/2026-11-26-task-409
```

The fixture must be a local `.xlsx` file. It must contain `Sheet1` with:

- `A1 = 2`
- `B1 = 3`
- `F1 = =A1+B1`

The source fixture is hashed before and after the run and is never written. The harness
requires `EXCEL.EXE=0` before starting and after every scenario. This is evaluation
evidence, not a production ownership-journal/watchdog implementation and not a production
Tool registration.

The formal normal path is fixed at exactly 10 runs. The Rust test independently rejects
any other normal run count; the single-run count is accepted only for the explicit
failure and watchdog injection scenarios.
