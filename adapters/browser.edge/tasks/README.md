# Browser Edge benchmark tasks

This directory owns declarative Edge benchmark task packages.

## T5.1

`t5.1.books-list-to-csv.json` declares the approved T5.1 extraction contract:

1. open the three deterministic loopback pages;
2. read each bounded product-list DOM fragment through
   `edge.cdp.dom_read`;
3. leave CSV normalization and serialization to the zero-dependency eval
   harness under `eval/tasks/edge/t5.1/`.

The external smoke target is `https://books.toscrape.com/`, but it is not used
for the 10-run acceptance baseline.

`t5.1.books-list-to-csv.external.json` is the separate three-run smoke package.
It navigates only the three URLs approved in
`eval/tasks/edge/t5.1/external-smoke-contract.json` and reads bounded `body`
fragments before handing the same DOM outputs to the eval-only CSV normalizer.
It waits for the page `load` event rather than whole-page network idle because
the approved static site references third-party assets that may remain active.
It does not replace the local 10-run package.

## Runtime integration

TASK-107 added deterministic task-package assertions for
`edge.cdp.navigate`, `edge.cdp.dom_read`, and
`edge.cdp.wait_network_idle`. The T5.1 package now renders six
executable steps without `MissingAssertion`, and its rendered postconditions are
accepted by `assistant_verify`.

This resolves the runtime-contract portion of `DRIFT-056-2`. It does not claim a
real browser run: TASK-056 remains Blocked until Edge uses the dedicated profile,
the local fixture passes 10/10, and the approved external smoke passes 3/3.

`RuntimeCdpProvider` now uses the real loopback CDP transport, and TASK-112
provides production handlers for `navigate`, `dom.read`, and
`wait_network_idle` plus an Edge/Chrome `ToolRegistry` builder. The builder is
verified through the real in-process `ToolBus` with a scripted CDP runtime.

`edge.cdp.form_fill` is now a production handler: it validates a bounded,
unique field set, writes only text-like controls, and never submits or clicks.
TASK-057 still owns the diff display, instruction-origin attribution, and
human-confirmed submit workflow. Download, screenshot, click, browser
auto-launch wiring inside the CLI production root, and the remaining T5.2
acceptance remain pending.

## T5.2 human approval

`t5.2.profile-human-approval.json` is the private human-present loopback package
for the current T5.2 slice. It navigates to the local `127.0.0.1:8766` form,
reads and fills three synthetic fields, then stops at `form_submit` with the
TASK-355 `step_approval_requested` event. The private runner resumes only after
the human types `once`; the same event binds the task, step, high/L3/once scope
and expiry, and Policy consumes one `ApprovalGrants` use before the submit tool
runs.

The package is not a real UI-session acceptance and does not close TASK-057.
It does not add a generic click tool, accept real personal data, or authorize a
second submit. The private runner may pre-approve the navigate and fill setup
steps with `--approve-step navigate_form --approve-step fill_form`, but it
rejects any pre-approval of `form_submit`; the submit itself must be approved
interactively with `once`.
