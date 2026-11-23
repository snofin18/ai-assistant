# com.microsoft.excel Adapter safety shell v1

This directory is the declarative Microsoft Excel adapter safety shell. It
contains data only. It does not create a COM object, launch Excel, open a
workbook, consume a GUI, or implement T2.1-T2.3.

## Files

- `adapter.toml`: Excel identity, version window, COM/UIA boundary, bundle
  envelope, process ownership requirements, and forbidden properties.
- `app_map.json`: authored workbook/worksheet identity, value semantics,
  lifecycle, safety, and deterministic projection manifest.
- `memory/app_map.v1.json`: canonical `core_app_map_v1` projection.
- `selectors/targets.json`: provisional UIA shell selectors for the Excel
  window and generic modal dialog only.
- `tools/tools.json`: intentionally empty until spreadsheet/COM capability IDs
  receive an ADR.
- `rollback/recipes.json`: intentionally empty because no model-visible write
  tool is declared yet.
- `interrupts/interrupts.json`: Protected View, AutoSave/OneDrive, read-only or
  occupied files, overwrite, update links, and macro warnings.

## Capability Boundary

The current capability catalog has no Excel COM or spreadsheet capability IDs.
Using accessibility or keyboard capabilities for COM cell operations would
misstate the implementation. This pack therefore declares zero model-visible
tools. A later ADR must approve the required capability vocabulary before a
real Excel tool surface and rollback recipes can be added.

## Safety Boundaries

- Excel grid UIA is virtualized; selectors never target cells, rows, columns,
  or grid content.
- Protected View's Enable Editing action is human-only.
- AutoSave/OneDrive conflict, read-only/occupied state, overwrite, update links,
  and macro warnings all fail closed to human decisions.
- `DisplayAlerts=false` is permanently forbidden.
- COM writes cannot use Excel `Application.Undo` as the recovery claim. Future
  write tools must provide L1 value snapshots plus a file shadow copy.
- Existing user `EXCEL.EXE` processes are never reused, attached to, or
  terminated.
- The generic `#32770` dialog class must be bound to the owned Excel process
  identity; class matching alone is not a target identity.

## Validation

```powershell
cargo run -p xtask -- adapter-validate adapters/com.microsoft.excel --report-only
python adapters/com.microsoft.excel/tests/validate_adapter_pack.py
python -m unittest discover -s adapters/com.microsoft.excel/tests -p "test_*.py"
```

Passing these validators proves the declarative shell only. It is not evidence
that Protected View, AutoSave, real workbooks, GUI behavior, or COM execution
has been tested.
