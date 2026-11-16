# Excel COM preflight

This directory contains the isolated Microsoft Excel COM probe for Stage 2.

The probe:

- refuses to run when any `EXCEL.EXE` process already exists;
- creates one new unsaved workbook and writes only temporary cells;
- reads `Value2`, `Formula`, and `Text` for a calculated formula;
- records `CanUndo` around programmatic writes and one explicit Undo call;
- reads Protected View count, AutoSave state, and the current `DisplayAlerts` value;
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
