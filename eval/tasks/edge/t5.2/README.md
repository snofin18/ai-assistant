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

## Human approval loopback

Build the private runner, then run:

```powershell
cargo build -p assistant-agent-core
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_real_edge_human_approval.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/<date>-task-356-human-approval `
  -EdgeExecutable "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
```

The runner opens the local form in a dedicated Edge profile, reads and fills the
three synthetic fields, and pauses before `form_submit`. It prints the redacted
`step_approval_requested` event and waits for the exact input `once`. Only then
does Policy consume one `ApprovalGrants` use and call `form_submit` exactly once.
The navigate and fill setup steps are explicitly pre-approved; pre-approving
`form_submit` in human-approval mode is rejected. The fill step never submits.
The script verifies `submitted=true`, the query-free local `post_url`, the
`submitted-marker`, and process/port cleanup. The model-visible URL removes
query/fragment data and redacts any expected field value found in its path.

This is a human-present CLI acceptance, not a real UI-session acceptance. Do not
add real personal data, credentials, files, payment details, CAPTCHA, or external
redirects to this fixture.

## Desktop UI human approval

Build the production binaries and run:

```powershell
pnpm --dir apps/desktop-ui build
cargo build --manifest-path apps/desktop-ui/src-tauri/Cargo.toml
cargo build -p assistant-agent-core
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/2026-11-12-task-364-desktop-ui-human-approval
```

The runner starts the local fixture, Core's authenticated `--ui-approval`
session, a dedicated Edge profile, and the production desktop-ui binary. It
writes `phase.txt=awaiting_human_confirmation` when the approval card is waiting
for a person. The human must inspect the redacted field diff, select `once`,
type the exact irreversible confirmation phrase, and click the approval button
in the desktop-ui window. The runner never approves on the human's behalf.

After the decision, the runner verifies exactly one committed `form_submit`,
`submitted=true`, the query-free local success URL, and the
`submitted-marker`. It then closes the UI and dedicated Edge profile and writes
`summary.json`. Tokens stay only in the child-process environment and are not
written to command or evidence logs.

## Safety boundary

Only synthetic values belong in this form. Do not add real credentials, personal
data, passwords, files, payment details, CAPTCHA flows, external redirects, or
cross-origin OAuth. The fixture is for local approval/diff and human acceptance
only; it is not an external-site smoke target.
