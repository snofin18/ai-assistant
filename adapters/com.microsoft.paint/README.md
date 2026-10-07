# com.microsoft.paint Adapter v0

This directory is the declarative Paint adapter pack for Windows 11 Paint. It
contains data only. Runtime loading, selector execution, pointer input, pixel
snapshots, and the TASK-044 through TASK-046 task workflows are not part of this
card.

## Files

- `adapter.toml`: app identity, version range, UIA plus L4 input channel, health,
  coordinate, and safety defaults.
- `app_map.json`: Paint identity, command table, UI map, coordinate model,
  workflows, state machine, undo capability, and evidence status.
- `memory/app_map.v1.json`: projection that conforms to the current
  `assistant-core` App Map v1 loader.
- `selectors/targets.json`: declarative selector candidates for the window,
  toolbar, tools, color control, layer panel, canvas, status bar, and dialogs.
- `tools/tools.json`: Paint tool declarations, including postconditions,
  reversibility, risk, approval, coordinate, snapshot, and visual assertions.
- `rollback/recipes.json`: L0 `Ctrl+Z`, L1 pixel snapshot, and L2 compensating
  actions for tool, color, layer, file, and canvas changes.
- `interrupts/interrupts.json`: unsaved canvas, Save As, overwrite, unsupported
  AI feature, color picker, and transient popup handling.
- `tests/validate_adapter_pack.py`: contract checks for the data pack.

## Current Status

The package is **provisional**. The Windows 11 Paint UIA tree was not captured
in this card, so every selector target carries `probe_status: required`. No
selector id, coordinate calibration, or visual tolerance in this package is a
real-machine acceptance claim. TASK-044 must run the first real Paint
calibration and replace provisional selector evidence before the tools are
treated as executable.

The observed package version on this host is `11.2605.81.0`; the package
family is `Microsoft.Paint_8wekyb3d8bbwe`.

## Coordinate Contract

Paint drawing uses canvas coordinates, while pointer input uses physical screen
pixels. The adapter declares the conversion inputs explicitly:

```text
screen_px = viewport_origin_px
          + (canvas_point_px - viewport_offset_px) * zoom_ratio
```

The canvas transform must be resolved immediately before each write. A point
resolved before a zoom, scroll, resize, DPI, or monitor change is expired and
must not be reused.

## Validation

```powershell
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
python -c "import json, pathlib; [json.loads(p.read_text(encoding='utf-8')) for p in pathlib.Path('adapters/com.microsoft.paint').rglob('*.json')]"
python -c "import tomllib, pathlib; tomllib.loads(pathlib.Path('adapters/com.microsoft.paint/adapter.toml').read_text(encoding='utf-8'))"
```

The contract test checks app id consistency, tool names, postconditions,
rollback references, selector scores, probe status, coordinate fields, and
unsafe interrupt defaults.

## Safety Boundaries

- Visible text is never the primary selector. `name_regex` and `title_regex`
  candidates are low-score locale-dependent fallbacks.
- A canvas write requires a valid coordinate transform, an exclusive target
  lease, a pre-write pixel snapshot, and a post-write visual assertion.
- Tool, color, and layer changes are not document content and are not covered by
  `Ctrl+Z`; they use L2 compensating actions.
- `Ctrl+Z` is declared by this adapter, but it is not sufficient alone. Undo
  success must be verified against the pre-write pixel snapshot.
- Unsaved canvas dialogs default to Cancel. The adapter never chooses Save or
  Don't Save on the user's behalf.
- Save As is a cross-process Shell dialog. Existing files are never overwritten
  silently.
- AI features that require network access are disabled and escalate to
  `NeedsHuman` when encountered.

## Known Gaps

- Real UIA control ids and class names for Paint tools and layers are not yet
  measured.
- Canvas coordinate error and mixed-DPI behavior are not yet measured.
- Pixel tolerance values are declared contracts, not calibrated values.
- The current protocol `ToolSchema` does not yet expose every adapter-level
  field used here, such as `postconditions`, `risk_level`, and
  `requires_approval`. The package keeps those fields explicit instead of
  hiding them in prose.
- The selector file uses the adapter-level declaration shape until a later
  stage formalizes selector-chain schemas.
