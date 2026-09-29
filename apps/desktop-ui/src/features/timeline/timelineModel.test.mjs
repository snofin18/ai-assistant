import assert from "node:assert/strict";
import test from "node:test";

import {
  createInitialTimelineControllerState,
  getReplayAvailability,
  getUndoAvailability,
  parseTimelineModel,
  timelineControllerReducer,
} from "./timelineModel.ts";

function createStep(overrides = {}) {
  return {
    stepId: "step-1",
    action: "notepad.replace_text",
    targetLabel: "Quarterly report",
    status: "succeeded",
    preFingerprint: "sha256:before",
    postFingerprint: "sha256:after",
    verification: "passed",
    verificationReasonKey: null,
    evidence: [
      {
        id: "snapshot-1",
        kind: "tree_snapshot",
        labelKey: "timeline.evidence.tree_snapshot",
        href: "evidence://tree-1",
      },
    ],
    durationMs: 412,
    cost: { tokensIn: 120, tokensOut: 8, usd: 0.0012, latencyMs: 900 },
    reversibility: "L1ContentSnapshot",
    undo: {
      isAvailable: true,
      methodKey: "timeline.undo.content_snapshot",
      anchorId: "anchor-55",
      disabledReasonKey: null,
    },
    canReplay: true,
    replayDisabledReasonKey: null,
    ...overrides,
  };
}

test("test_parse_timeline_valid_step_preserves_evidence_and_cost", () => {
  const result = parseTimelineModel([createStep()]);
  assert.equal(result.ok, true);
  assert.equal(result.steps[0]?.evidence[0]?.kind, "tree_snapshot");
  assert.equal(result.steps[0]?.cost.latencyMs, 900);
});

test("test_timeline_undo_failed_step_is_disabled_with_reason", () => {
  const result = parseTimelineModel([createStep({ status: "failed" })]);
  assert.equal(result.ok, true);
  const step = result.steps[0];
  assert.ok(step);
  assert.deepEqual(getUndoAvailability(step), {
    isAvailable: false,
    reasonKey: "timeline.undo.step_not_succeeded",
  });
});

test("test_timeline_undo_irreversible_step_is_disabled", () => {
  const result = parseTimelineModel([
    createStep({
      reversibility: "L3Irreversible",
      undo: {
        isAvailable: false,
        methodKey: "timeline.undo.none",
        anchorId: null,
        disabledReasonKey: "timeline.undo.irreversible",
      },
    }),
  ]);
  assert.equal(result.ok, true);
  const step = result.steps[0];
  assert.ok(step);
  assert.equal(getUndoAvailability(step).reasonKey, "timeline.undo.irreversible");
});

test("test_timeline_replay_requires_tree_snapshot_evidence", () => {
  const result = parseTimelineModel([
    createStep({
      evidence: [
        {
          id: "screenshot-1",
          kind: "screenshot",
          labelKey: "timeline.evidence.screenshot",
          href: "evidence://screenshot-1",
        },
      ],
    }),
  ]);
  assert.equal(result.ok, true);
  const step = result.steps[0];
  assert.ok(step);
  assert.equal(getReplayAvailability(step).reasonKey, "timeline.replay.missing_snapshot");
});

test("test_timeline_controller_rejects_unavailable_replay", () => {
  const result = parseTimelineModel([
    createStep({ canReplay: false, replayDisabledReasonKey: "timeline.replay.unavailable" }),
  ]);
  assert.equal(result.ok, true);
  const initial = createInitialTimelineControllerState(result.steps);
  const next = timelineControllerReducer(initial, {
    type: "request_replay",
    stepId: "step-1",
  });
  assert.equal(next.errorKey, "timeline.replay.unavailable");
});

test("test_parse_timeline_external_evidence_link_is_rejected", () => {
  const result = parseTimelineModel([
    createStep({
      evidence: [
        {
          id: "snapshot-1",
          kind: "tree_snapshot",
          labelKey: "timeline.evidence.tree_snapshot",
          href: "https://example.invalid/evidence",
        },
      ],
    }),
  ]);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /evidence entry is malformed/);
});
