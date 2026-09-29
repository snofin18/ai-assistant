import assert from "node:assert/strict";
import test from "node:test";

import { UI_IPC_VERSION, parseUiCommandEnvelope } from "../../ipc/contract.ts";
import { INTENT_ID_PREFIX, buildSubmitIntentCommand, nextIntentId } from "./intentModel.ts";

function envelope(intentId, goal) {
  return parseUiCommandEnvelope({
    version: UI_IPC_VERSION,
    command: buildSubmitIntentCommand(intentId, goal),
  });
}

test("a user goal becomes a valid submit_intent command", () => {
  const result = envelope("i_ui_1", "把报表替换成报告并保存");
  assert.equal(result.ok, true);
  assert.deepEqual(result.value.command, {
    kind: "submit_intent",
    intent_id: "i_ui_1",
    goal: "把报表替换成报告并保存",
  });
});

test("a blank goal is rejected instead of being sent", () => {
  const result = envelope("i_ui_1", "   ");
  assert.equal(result.ok, false);
  assert.ok(result.errors.length > 0);
});

test("intent ids are locally unique and prefixed", () => {
  const first = nextIntentId(1_700_000_000_000, 1);
  const second = nextIntentId(1_700_000_000_000, 2);
  assert.ok(first.startsWith(INTENT_ID_PREFIX));
  assert.notEqual(first, second);
});
