# com.microsoft.notepad Adapter v1

This directory is the declarative Notepad adapter pack for Windows 11 modern
Notepad. It contains data only. Runtime loading, selector resolution, tool
execution, approval wiring, and the TASK-036~038 task workflows are not part of
this card.

## Files

- `adapter.toml`: app identity, Win11 version range, UIA channel, health, and
  safety defaults plus the ADR-0104 bundle envelope.
- `app_map.json`: authored knowledge pack with the deterministic projection
  manifest and omission coverage.
- `memory/app_map.v1.json`: canonical `core_app_map_v1` projection generated
  only from `projection_manifest.entries`.
- `selectors/targets.json`: ordered selector candidates for the main window,
  editor, tabs, file menu, and Save As dialog controls in
  `uia_candidate_chain_v1`.
- `tools/tools.json`: the five Notepad tool declarations, including
  ToolSchema 1.1, capabilities, and canonical postconditions.
- `rollback/recipes.json`: L0 `Ctrl+Z`, L1 content snapshot/shadow copy, and
  the ADR-0106 rollback-only created-tab compensation.
- `interrupts/interrupts.json`: unsaved changes, Save As, and transient
  TeachingTip handling with typed actions and explicit priority.

## Bundle Contract

- `adapter.toml` declares exactly one six-slot `[bundle]` envelope.
- The App Map projection is deterministic and byte-checked by the generic
  validator.
- Selector candidates use the frozen `uia_candidate_chain_v1` discriminated
  union. Visible text remains fallback-only and locale-dependent.
- Every write tool has exactly one rollback coverage row. Save As remains L3
  with `no_recovery`; `notepad.tab.new` uses the rollback-only
  `close_created_target` builtin and does not add a model-visible close tool.
- Interrupts use explicit priority and typed actions. Save, Don't Save, and
  Cancel remain human decisions.

## Validation

```powershell
cargo run -p xtask -- adapter-validate adapters/com.microsoft.notepad --report-only
python adapters/com.microsoft.notepad/tests/validate_adapter_pack.py
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
- The created-tab compensation only closes the same invocation's recorded,
  still-empty tab. Any content or save prompt fails closed to human handling.
