# Paint task packages

This directory contains declarative task packages for the Paint adapter.

## T3.1

`t3.1.new-canvas-rectangle-color-screenshot.json` describes:

1. create a new canvas;
2. select the rectangle tool and foreground color;
3. verify the active layer;
4. capture a pre-write pixel snapshot;
5. resolve both canvas points through the current zoom and scroll transform;
6. draw one rectangle;
7. capture post-write pixels for visual verification.

The package declares `tolerance_px <= 2` and `silent_failure_rate = 0`. The
real T3.1 standard-case ten-run evaluation on 2026-10-19 passed 10/10
(`success_rate=1.0`), and the measured drag end-point error is `0 px` against
the 2 px bound.

## T3.2

`t3.2.continue-existing-canvas-rectangle.json` describes:

1. use the already-open document; it never calls `paint.document.new`;
2. select the rectangle tool and foreground color;
3. verify the active layer;
4. capture a pre-write pixel snapshot;
5. resolve both canvas points through the current zoom and scroll transform;
6. draw one rectangle;
7. capture post-write pixels for visual verification.

The handler additionally requires the pre-write and post-write capture content
addresses to differ. `changed_ratio_within` is an upper bound and can pass on
identical images, so T3.2 uses the content address as the explicit
"something actually changed" lower bound.

## T3.3

`t3.3.open-png-draw-rollback.json` describes:

1. open the deterministic T3.2 PNG fixture without discarding unsaved work;
2. select the rectangle tool, red foreground, `实心边框`, `无填充`, and `3 像素`;
3. capture a content-addressed pre-write pixel snapshot;
4. draw one rectangle;
5. bring the resolved Paint window forward, require it to be foreground and
   unoccluded, send the adapter-declared `Ctrl+Z`, and compare post-undo pixels
   with the real pre-write snapshot.

The real-machine ten-run evaluation on 2026-10-20 passed `10/10`
(`success_rate=1.0`). Every run completed the declared one undo step and passed
`mean_abs_diff_within(max=2, confidence_min=0.9)`. A no-op undo is a
`VerifyFailed`, never a successful rollback.

Paint keeps closed `Microsoft.UI.Content.PopupWindowSiteBridge` flyout handles
on its UI thread. The Windows popup probe therefore treats only a visible
popup or an invisible popup whose owner is disabled as blocking. This preserves
the unsaved-dialog guard without treating stale flyout handles as blockers.

## Production selection

The production entry point defaults Paint to **T3.2** (`--document-mode existing`).
It selects **T3.1** only when the caller explicitly passes `--document-mode new`,
and an explicit `--task-package <path>` still overrides both modes.
