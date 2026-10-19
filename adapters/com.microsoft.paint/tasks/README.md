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

## Production selection

The production entry point defaults Paint to **T3.2** (`--document-mode existing`).
It selects **T3.1** only when the caller explicitly passes `--document-mode new`,
and an explicit `--task-package <path>` still overrides both modes.
