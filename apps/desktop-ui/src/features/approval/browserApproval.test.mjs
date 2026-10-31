import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { parseUiEvent } from "../../ipc/contract.ts";
import { toApprovalCardModel } from "./browserApproval.ts";

function fixture() {
  const url = new URL(
    "../../../../agent-core/tests/fixtures/ui_ipc/step_approval_requested.json",
    import.meta.url,
  );
  return JSON.parse(readFileSync(url, "utf8"));
}

test("browser approval event maps to a high-risk once-only card", () => {
  const parsed = parseUiEvent(fixture());
  assert.equal(parsed.ok, true);
  const model = toApprovalCardModel(parsed.value, 1_000);
  assert.equal(model.requestId, "runtime:browser.t5_2.profile:submit_form");
  assert.equal(model.action, "edge.cdp.form_submit");
  assert.equal(model.targetLabel, "http://127.0.0.1:8765");
  assert.equal(model.riskLevel, "high");
  assert.equal(model.reversibility, "L3Irreversible");
  assert.deepEqual(model.scopeOptions, ["once"]);
  assert.deepEqual(model.diff, {
    kind: "fields",
    entries: [
      { field: "#display-name", before: "", after: "[redacted:3]" },
      { field: "#note", before: "[redacted:3]", after: "[redacted:5]" },
    ],
  });
  assert.deepEqual(model.evidence, [
    {
      id: "form-state:runtime:browser.t5_2.profile:submit_form",
      kind: "form_state",
      labelKey: "approval.evidence.form_state",
      href: null,
    },
  ]);
  assert.equal(model.isExpired, false);
});

test("browser approval expiry is derived from the event timestamp", () => {
  const parsed = parseUiEvent(fixture());
  assert.equal(parsed.ok, true);
  const model = toApprovalCardModel(parsed.value, 4_102_444_800_000);
  assert.equal(model.isExpired, true);
});

test("unknown browser approval fields fail before a card is produced", () => {
  const event = fixture();
  event.evidence.raw_fields = [{ selector: "#note", value: "secret" }];
  const result = parseUiEvent(event);
  assert.equal(result.ok, false);
  assert.ok(result.errors.length > 0);
});

test("browser approval rejects a non-once scope", () => {
  const event = fixture();
  event.scope_options = ["once", "this_task"];
  const result = parseUiEvent(event);
  assert.equal(result.ok, false);
});
