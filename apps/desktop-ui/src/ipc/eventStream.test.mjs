import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { parseUiEvent } from "./contract.ts";
import { parseUiBridgeMessage, parseUiBridgePoll } from "./eventStream.ts";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../agent-core/tests/fixtures/ui_ipc/step_approval_requested.json",
      import.meta.url,
    ),
    "utf8",
  ),
);

test("a valid event envelope keeps the strict UiEvent payload", () => {
  const result = parseUiBridgeMessage({ kind: "event", event: fixture }, parseUiEvent);
  assert.equal(result.ok, true);
  if (result.ok) {
    assert.equal(result.value.kind, "event");
    assert.equal(result.value.event.kind, "step_approval_requested");
  }
});

test("an invalid UiEvent is rejected before rendering", () => {
  const invalid = { ...fixture, unexpected: "field" };
  const result = parseUiBridgeMessage({ kind: "event", event: invalid }, parseUiEvent);
  assert.equal(result.ok, false);
});

test("a structured bridge failure is accepted", () => {
  const result = parseUiBridgeMessage(
    {
      kind: "failure",
      failure: { code: "core_transport_unavailable", message: "Core disconnected" },
    },
    parseUiEvent,
  );
  assert.equal(result.ok, true);
  if (result.ok) {
    assert.equal(result.value.kind, "failure");
  }
});

test("unknown bridge fields and blank failures are rejected", () => {
  assert.equal(
    parseUiBridgeMessage(
      {
        kind: "failure",
        failure: { code: "core_transport_unavailable", message: "closed" },
        extra: true,
      },
      parseUiEvent,
    ).ok,
    false,
  );
  assert.equal(
    parseUiBridgeMessage(
      {
        kind: "failure",
        failure: { code: " ", message: "closed" },
      },
      parseUiEvent,
    ).ok,
    false,
  );
});

test("a sequenced bridge poll reuses the strict bridge parser", () => {
  const result = parseUiBridgePoll(
    { sequence: 4, message: { kind: "event", event: fixture } },
    parseUiEvent,
  );
  assert.equal(result.ok, true);
  if (result.ok) {
    assert.equal(result.value.sequence, 4);
    assert.equal(result.value.message.kind, "event");
  }
});
