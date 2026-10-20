# Paint T3.3 evaluation

This evaluation covers the rollback path:

```text
open PNG -> capture pre snapshot -> draw one rectangle -> Ctrl+Z -> compare pixels
```

The task package is
`adapters/com.microsoft.paint/tasks/t3.3.open-png-draw-rollback.json`. It uses the
same deterministic `2048 x 1536` PNG fixture as T3.2 and draws a `3 px` red,
unfilled rectangle from `(64,48)` to `(1984,1488)`.

The rollback contract is intentionally strict:

- the reference image must be the content-addressed pre-write snapshot;
- Paint must be foreground and unoccluded before `Ctrl+Z`;
- `undo_steps_completed` must equal `expected_undo_steps`;
- a no-op undo must fail with `VerifyFailed`;
- the post-undo pixels must match the pre-write snapshot with
  `mean_abs_diff <= 2` and `confidence >= 0.9`.

Validate static declarations with:

```powershell
python eval/tasks/paint/t3.3/validate.py
```

Run the real-machine acceptance ten times with:

```powershell
powershell -NoProfile -File eval/tasks/paint/t3.3/run-ten.ps1 -AllowClosePaint
```

The runner starts a fresh Paint process before every run, focuses it before
launching the ignored T3.3 test, records every raw test log, and writes
`ten-run-summary.json` under `target/paint-t3-3-ten-run/` by default.

The 2026-10-20 real-machine evaluation passed `10/10` with the task package's
one-step undo contract and the declared pixel comparison on every run. The
frozen summary is `ten-run-summary.json`.
