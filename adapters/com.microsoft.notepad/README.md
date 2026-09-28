# com.microsoft.notepad Adapter v0

This directory is the declarative Notepad adapter pack for Windows 11 modern
Notepad. It contains data only. Runtime loading, selector resolution, tool
execution, approval wiring, and the TASK-036~038 task workflows are not part of
this card.

## Files

- `adapter.toml`: app identity, Win11 version range, UIA channel, health, and
  safety defaults.
- `app_map.json`: architecture v2 section 6.7 knowledge pack.
- `memory/app_map.v1.json`: projection that conforms to the current
  `assistant-core` App Map v1 loader.
- `selectors/targets.json`: ordered selector candidates for the main window,
  editor, tabs, file menu, and Save As dialog controls.
- `tools/tools.json`: the five Notepad tool declarations, including
  postconditions and rollback metadata.
- `rollback/recipes.json`: L0 `Ctrl+Z` and L1 content-snapshot rollback paths.
- `interrupts/interrupts.json`: unsaved changes, Save As, and transient
  TeachingTip handling.

## Current Contract Gaps

The stage-2 adapter abstraction is intentionally not implemented yet:

- The current protocol `ToolSchema` does not include `postconditions` and does
  not expose `none_readonly`. `tools/tools.json` follows architecture v2
  section 5.2 and keeps those adapter-level fields explicit instead of hiding
  them.
- The current core App Map v1 loader is deliberately small. The rich
  `app_map.json` is the architecture v2 knowledge pack; `memory/app_map.v1.json`
  is the loadable projection.
- The selector file uses the adapter-level declaration shape until stage 2
  formalizes selector-chain schemas.

## Validation

```powershell
python -c "import json, pathlib; [json.loads(p.read_text(encoding='utf-8')) for p in pathlib.Path('adapters/com.microsoft.notepad').rglob('*.json')]"
python -c "import tomllib, pathlib; tomllib.loads(pathlib.Path('adapters/com.microsoft.notepad/adapter.toml').read_text(encoding='utf-8'))"
```

## Safety Boundaries

- The editor `AutomationId` is empty. Visible text is never the primary
  selector.
- `Ctrl+Z` is explicitly declared, but it is only an L0 path. Text writes
  require an L1 snapshot.
- Unsaved changes default to Cancel. The adapter never chooses Save or Don't
  Save on the user's behalf.
- Save As is a cross-process Win32 `#32770` dialog. Existing files are never
  overwritten silently.
- Large files should use the L1 file channel instead of the UIA text path.
