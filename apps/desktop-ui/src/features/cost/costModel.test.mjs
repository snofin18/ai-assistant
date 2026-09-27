import assert from "node:assert/strict";
import test from "node:test";

import {
  parseCostPanelModel,
  summarizeCosts
} from "./costModel.ts";

function createRecord(overrides = {}) {
  return {
    recordId: "record-1",
    taskId: "task-1",
    occurredAt: "2026-09-28T02:00:00Z",
    modelId: "plan_large",
    appId: "com.microsoft.notepad",
    inputTokens: 1000,
    cachedInputTokens: 200,
    outputTokens: 100,
    costMicroUsd: 4200,
    latencyMs: 900,
    ...overrides
  };
}

test("test_parse_cost_panel_valid_record_preserves_micro_usd", () => {
  const result = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord()]
  });
  assert.equal(result.ok, true);
  assert.equal(result.model.records[0]?.costMicroUsd, 4200);
});

test("test_parse_cost_panel_cached_tokens_above_input_is_rejected", () => {
  const result = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord({ cachedInputTokens: 1001 })]
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /cachedInputTokens exceeds/);
});

test("test_parse_cost_panel_unsafe_integer_cost_is_rejected", () => {
  const result = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord({ costMicroUsd: Number.MAX_SAFE_INTEGER + 1 })]
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /safe non-negative integer/);
});

test("test_parse_cost_panel_invalid_calendar_timestamp_is_rejected", () => {
  const result = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord({ occurredAt: "2026-02-30T00:00:00Z" })]
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /UTC RFC 3339/);
});

test("test_parse_cost_panel_invalid_clock_timestamp_is_rejected", () => {
  const result = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord({ occurredAt: "2026-09-28T99:00:00Z" })]
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /UTC RFC 3339/);
});

test("test_parse_cost_panel_leap_second_timestamp_is_rejected", () => {
  const result = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord({ occurredAt: "2026-09-28T00:00:60Z" })]
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /UTC RFC 3339/);
});

test("test_summarize_cost_panel_filters_task_today_and_month", () => {
  const parsed = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [
      createRecord(),
      createRecord({
        recordId: "record-2",
        taskId: "task-2",
        occurredAt: "2026-09-28T01:00:00Z",
        costMicroUsd: 100,
        inputTokens: 10,
        cachedInputTokens: 0,
        outputTokens: 1,
        latencyMs: 10
      }),
      createRecord({
        recordId: "record-3",
        taskId: "task-3",
        occurredAt: "2026-09-01T23:00:00Z",
        costMicroUsd: 50,
        inputTokens: 5,
        cachedInputTokens: 0,
        outputTokens: 1,
        latencyMs: 5
      })
    ]
  });
  assert.equal(parsed.ok, true);
  const task = summarizeCosts(parsed.model, "task", "2026-09-28T12:00:00Z");
  const today = summarizeCosts(parsed.model, "today", "2026-09-28T12:00:00Z");
  const month = summarizeCosts(parsed.model, "month", "2026-09-28T12:00:00Z");
  assert.equal(task.ok && task.summary.totals.costMicroUsd, 4200);
  assert.equal(today.ok && today.summary.totals.costMicroUsd, 4300);
  assert.equal(month.ok && month.summary.totals.costMicroUsd, 4350);
});

test("test_summarize_cost_panel_splits_by_model_and_app", () => {
  const parsed = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [
      createRecord(),
      createRecord({
        recordId: "record-2",
        modelId: "chat_small",
        appId: "browser.edge",
        costMicroUsd: 100,
        inputTokens: 10,
        cachedInputTokens: 0,
        outputTokens: 1,
        latencyMs: 10
      })
    ]
  });
  assert.equal(parsed.ok, true);
  const summary = summarizeCosts(parsed.model, "task", "2026-09-28T12:00:00Z");
  assert.equal(summary.ok, true);
  assert.deepEqual(
    summary.summary.byModel.map((entry) => entry.key),
    ["chat_small", "plan_large"]
  );
  assert.deepEqual(
    summary.summary.byApp.map((entry) => entry.key),
    ["browser.edge", "com.microsoft.notepad"]
  );
});

test("test_summarize_cost_panel_invalid_now_is_rejected", () => {
  const parsed = parseCostPanelModel({
    currentTaskId: "task-1",
    records: [createRecord()]
  });
  assert.equal(parsed.ok, true);
  const summary = summarizeCosts(parsed.model, "today", "2026-09-28");
  assert.equal(summary.ok, false);
  assert.match(summary.errors.join("\n"), /nowIso/);
});
