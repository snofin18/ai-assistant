# T5.2 local form fixture

This directory is the zero-dependency local target for ADR-0101 T5.2 form
confirmation work. It provides a synthetic same-origin form and a deterministic
success page. It does not call the product runtime and does not by itself prove
TASK-057 complete.

## Generate

```powershell
python eval/tasks/edge/t5.2/generate_fixtures.py
```

The generator writes `fixtures/form.html` and `fixtures/t5.2/submitted`.

## Test

```powershell
python -m unittest discover -s eval/tasks/edge/t5.2 -p "test_*.py" -v
```

The tests cover deterministic generation, the three synthetic fields, the unique
submit control, same-origin action resolution, the fixed success marker, loopback
binding, cache headers, HTML content type for the extensionless success page,
and disabled directory listings.

## Serve

```powershell
python eval/tasks/edge/t5.2/serve_fixture.py
```

The server binds only to `127.0.0.1:8766`. Open:

```text
http://127.0.0.1:8766/form.html
```

The form submits with GET to `/t5.2/submitted`. A successful manual check ends on
the page containing `data-testid="submitted-marker"`.

## Safety boundary

Only synthetic values belong in this form. Do not add real credentials, personal
data, passwords, files, payment details, CAPTCHA flows, external redirects, or
cross-origin OAuth. The fixture is for local approval/diff and human acceptance
only; it is not an external-site smoke target.
