# Stage 1 repeated-run acceptance matrix

This directory is the machine-readable acceptance index for the nine Stage 1
product tasks:

- Notepad T1.1, T1.2, T1.3
- Paint T3.1, T3.2, T3.3
- Edge T5.1, T5.2, T5.3

`acceptance-matrix.json` fixes the task set, application, minimum success rate,
evidence adapter, and source evidence path. It does not replace the source
evidence; the validator reads the committed runs and summaries directly.

## Validate

```powershell
python eval/stage-1/validate_acceptance_matrix.py
python eval/stage-1/validate_acceptance_matrix.py --require-complete
python -m unittest discover -s eval/stage-1 -p "test_*.py" -v
```

The default report exits zero when matrix structure and evidence are readable,
even when the stage is incomplete. It must print `stage1_complete=false` and the
missing task list. `--require-complete` exits one until every task has the full
ten-round evidence.

Missing files, malformed fields, forged round counts, and summary/result
contradictions are hard errors in every mode. A readable but short human
approval record is a normal incomplete result, not a structural error.

## Security ten-run evidence

```powershell
python eval/stage-1/run_security_ten.py
```

The runner executes the TASK-365 injection-security integration test ten times
and writes `evidence/t5.3-injection-security-ten-run.json`. T5.3 uses a 100%
threshold because security is a veto, not a statistical target.
