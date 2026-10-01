# notepad-like fixture app

## Purpose

`notepad-like` is a Windows WPF target application for UIA and failure-injection tests. It is deliberately simple and deterministic: it is not a replacement for the real Notepad and does not try to reproduce its file dialogs, encoding rules, EOL behavior, or full menu model.

The fixture is implemented with Windows PowerShell 5.1 and the built-in WPF assemblies. It does not require the .NET SDK, NuGet, WinAppSDK, or any third-party dependency.

## Requirements

- Windows 10/11 with Windows PowerShell 5.1.
- A desktop session for real window rendering. `--self-check` does not require a visible window.

## Run

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\notepad-like.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\notepad-like.ps1 --fault busy
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\notepad-like.ps1 --fault dialog
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\notepad-like.ps1 --self-check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\test-notepad-like.ps1
```

Options:

| Option | Meaning |
|---|---|
| `--fault <mode>` | `none`, `disappear`, `timeout`, `ambiguous`, `dialog`, or `busy` |
| `--document <path>` | Load this file into the editor at startup; `Save` writes back to it. |
| `--state-file <path>` | Write a JSON state file after startup or fault application |
| `--auto-close-ms <n>` | Close the window after `n` milliseconds when the dispatcher is responsive |
| `--self-check` | Validate XAML and the AutomationId manifest, then exit |
| `--help` | Print usage |

## Stable AutomationIds

The base IDs are listed under `required` in `automation-ids.json` and verified by `--self-check`. The modal dialog IDs are listed under `runtime`; they are created only when `--fault dialog` runs.

| AutomationId | Purpose |
|---|---|
| `MainWindow` | Main application window |
| `RootGrid` | Root layout container |
| `FileMenuButton` | File menu entry point |
| `OpenButton` | Open command |
| `SaveButton` | Save command |
| `SaveAsButton` | Save As command |
| `EditorHost` | Editor container |
| `EditorTextBox` | Primary editable text area |
| `StatusText` | Current application status |
| `FaultStatusText` | Current fault mode |
| `LineCountText` | Line count |
| `WordCountText` | Word count |
| `AddTabButton` | Create a new tab (increments `TabCountText`, clears the editor) |
| `TabCountText` | Current tab count |
| `BusyOverlay` | Busy-state overlay |
| `BusyMessageText` | Busy-state message |
| `BusyProgressBar` | Busy-state progress indicator |
| `UnexpectedDialog` | Injected modal dialog |
| `DialogMessageText` | Injected dialog message |
| `DialogCancelButton` | Dialog cancel action |
| `DialogContinueButton` | Dialog continue action |

`ambiguous` adds a second visible `TextBox` with the same `EditorTextBox` AutomationId at runtime. The base XAML itself must not contain duplicate IDs.

## Fault semantics

| Mode | Behavior |
|---|---|
| `none` | Normal window; editor and controls remain available |
| `disappear` | `EditorTextBox` is collapsed after startup |
| `timeout` | UI thread blocks for 8000 ms after writing state; a dispatcher probe must not run before the block ends |
| `ambiguous` | A second visible editor with the same AutomationId is added |
| `dialog` | A modal `UnexpectedDialog` blocks the main window |
| `busy` | Busy overlay is visible and write controls are disabled |

The state file records `schema_version`, `app`, `pid`, `fault`, `status`, `started_at`, `window_title`, `automation_ids`, and `details`.

## CI

The fixture is intended to run on a Windows GitHub Actions runner with a desktop session. Wiring it into CI is intentionally left to TASK-039; this card only provides the executable fixture and its focused test script. The local test script uses UI Automation to verify the real fault shapes and adds negative argument cases for fail-closed CLI parsing.

## Known limits

- It is a fixture, not a real Notepad replacement.
- It does not read or write user files.
- It does not implement encoding or EOL normalization.
- The focused tests validate the rendered dialog and all runtime AutomationIds through UIA. The `timeout` probe proves the dispatcher is blocked for at least 2 seconds, but the test does not assert natural auto-close after the modal closes or dispatcher recovery after the timeout block ends.
