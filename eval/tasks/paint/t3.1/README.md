# Paint T3.1 evaluation

This directory defines the T3.1 evaluation contract for:

```text
new canvas -> rectangle tool + color -> draw rectangle -> screenshot verify
```

`probe-evidence.json` records the real Windows 11 Paint UIA observations made on
2026-10-07. The probe found:

- process `mspaint.exe`, window class `MSPaintApp`;
- stable automation ids for `PencilTool`, `EraserTool`, `CanvasSizeTextBlock`,
  `ZoomValuesComboBox`, `ZoomSliderControl`, and `SettingsButton`;
- the canvas as a `Group` with automation id `image`;
- canvas size `418 x 74` pixels, rendered as `209 x 37` pixels at zoom `0.5`;
- rectangle and palette entries as `GridViewItem` / `ListItem` without stable
  automation ids, so visible names remain fallback-only and locale-dependent.

The ten-run success rate, real drag coordinate error, mixed-DPI behavior, and
final pixel tolerance are not measured in this directory. They remain open
until a real Paint GUI acceptance run is executed.

`probe-evidence.json` also records a 2026-10-08 selector-resolution probe that
drove the repository's own `assistant-platform-windows` provider against a live
Paint window. It measured that the provisional selector pack cannot drive the
acceptance run:

- the declared `main_window` class `WinUIDesktopWin32WindowClass` matches
  nothing; the live window class is `MSPaintApp`;
- the declared `canvas` class `Image` / role `Image` matches 38 elements and
  fails as `TargetAmbiguous`; `automation_id=image` resolves the canvas with
  bounds `(696,525)-(905,562)`, size `209x37`;
- the declared `rectangle_tool_button` class `GridViewItem` / role `ListItem`
  matches 43 elements and fails as `TargetAmbiguous`.

Calibrating these selectors is outside TASK-044's write scope and needs its own
card before the real ten-run acceptance can run.

Validate the declarations with:

```powershell
python eval/tasks/paint/t3.1/validate.py
```
