import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  UI_IPC_VERSION,
  parseUiCommandEnvelope,
  parseUiCommandOutcome,
  parseUiEvent,
} from "./contract.ts";

// The same golden fixtures are parsed by the Core-side Rust test, so a contract
// change on either side fails one of the two suites.
function fixture(name) {
  const url = new URL(`../../../agent-core/tests/fixtures/ui_ipc/${name}`, import.meta.url);
  return JSON.parse(readFileSync(url, "utf8"));
}

test("golden fixtures satisfy the UI contract", () => {
  assert.equal(UI_IPC_VERSION, "1.0");
  for (const name of [
    "submit_intent.json",
    "approve_request.json",
    "deny_request.json",
    "take_over_task.json",
  ]) {
    const result = parseUiCommandEnvelope(fixture(name));
    assert.equal(result.ok, true, `${name}: ${JSON.stringify(result)}`);
  }
});

test("golden approve fixture maps to the expected command", () => {
  const result = parseUiCommandEnvelope(fixture("approve_request.json"));
  assert.equal(result.ok, true);
  assert.deepEqual(result.value, {
    version: "1.0",
    command: { kind: "approve_request", request_id: "a_1", scope: "once" },
  });
});

test("illegal golden fixtures are rejected", () => {
  for (const name of [
    "invalid_unknown_field.json",
    "invalid_unknown_kind.json",
    "invalid_version.json",
  ]) {
    const result = parseUiCommandEnvelope(fixture(name));
    assert.equal(result.ok, false, `${name} must be rejected`);
    assert.ok(result.errors.length > 0);
  }
});

test("unknown fields are rejected even when the rest of the payload is valid", () => {
  const result = parseUiCommandEnvelope({
    version: "1.0",
    command: { kind: "pause_task", task_id: "t_1", extra: "attacker" },
  });
  assert.equal(result.ok, false);
});

test("blank identifiers are rejected", () => {
  const result = parseUiCommandEnvelope({
    version: "1.0",
    command: { kind: "cancel_task", task_id: "   " },
  });
  assert.equal(result.ok, false);
});

test("events require the nullable fields Core always sends", () => {
  const good = parseUiEvent({
    kind: "step_state_changed",
    task_id: "t_1",
    step_id: "s_1",
    status: "committed",
    phase: null,
    post_fingerprint: "sha256:abc",
  });
  assert.equal(good.ok, true);

  const missing = parseUiEvent({
    kind: "step_state_changed",
    task_id: "t_1",
    step_id: "s_1",
    status: "committed",
    post_fingerprint: null,
  });
  assert.equal(missing.ok, false);
});

test("outcomes must match a known status", () => {
  assert.equal(
    parseUiCommandOutcome({
      status: "approval_granted",
      request_id: "a_1",
      scope: "once",
    }).ok,
    true,
  );
  assert.equal(
    parseUiCommandOutcome({ status: "silently_succeeded", request_id: "a_1" }).ok,
    false,
  );
});
