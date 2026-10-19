# Paint T3.2 evaluation

This evaluation covers the deterministic path:

```text
open PNG -> read canvas size and zoom -> mark a region -> save as a new PNG
```

`prepare_fixture.py` generates a 2048 x 1536 PNG using only the Python standard
library. The PNG uses stored DEFLATE blocks so its bytes and SHA-256 are
deterministic across local zlib compression levels.

The frozen metadata is in `expected.json`:

- SHA-256: `a82b130e9beb6a1d8935395b4d95eb27f30748d74f221db057039a8325dfcc8c`
- bytes: `9439508`
- dimensions: `2048 x 1536`

The mark contract is fixed: a 3 px red outline at canvas coordinates
`(64,48) -> (1984,1488)` with no fill. The save contract requires a new target
path and must never overwrite an existing file.

Validate static declarations with:

```powershell
python eval/tasks/paint/t3.2/prepare_fixture.py --check
python eval/tasks/paint/t3.2/validate.py
```

Real-machine evidence is produced separately by the ignored Paint T3.2 tests:
`run-ten.ps1` records ten runs and `coordinate-error.ps1` records the measured
physical drag endpoint error. The latter is never inferred from run success.
