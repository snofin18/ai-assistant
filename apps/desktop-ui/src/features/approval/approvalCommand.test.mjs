import assert from "node:assert/strict";
import test from "node:test";

import { UI_IPC_VERSION, parseUiCommandEnvelope } from "../../ipc/contract.ts";
import { toApprovalCommand } from "./approvalCommand.ts";

function envelope(decision) {
  return parseUiCommandEnvelope({
    version: UI_IPC_VERSION,
    command: toApprovalCommand(decision),
  });
}

test("an approval decision becomes an approve_request command", () => {
  const result = envelope({
    kind: "approved",
    requestId: "a_1",
    action: "notepad.replace_text",
    scope: "once",
    instructionOrigin: "user_request",
  });
  assert.equal(result.ok, true);
  assert.deepEqual(result.value.command, {
    kind: "approve_request",
    request_id: "a_1",
    scope: "once",
  });
});

test("a denial becomes a deny_request command carrying its reason", () => {
  const result = envelope({
    kind: "denied",
    requestId: "a_1",
    reason: "user rejected the diff",
  });
  assert.equal(result.ok, true);
  assert.deepEqual(result.value.command, {
    kind: "deny_request",
    request_id: "a_1",
    reason: "user rejected the diff",
  });
});

test("a denial without a reason is rejected instead of being dropped", () => {
  const result = envelope({
    kind: "denied",
    requestId: "a_1",
    reason: "   ",
  });
  assert.equal(result.ok, false);
  assert.ok(result.errors.length > 0);
});
