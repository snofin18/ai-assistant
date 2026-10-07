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

Validate the declarations with:

```powershell
python eval/tasks/paint/t3.1/validate.py
```
