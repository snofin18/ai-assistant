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

The package declares `tolerance_px <= 2` and `silent_failure_rate = 0`. It
does not claim that those metrics were measured. The real ten-run evaluation
and coordinate error measurement remain open until a real Paint GUI run is
performed.
