import assert from "node:assert/strict";
import test from "node:test";

import {
  createInitialPickerControllerState,
  generateSelectorCandidates,
  getHighlightRect,
  getPrimaryCandidateId,
  parsePickerSnapshot,
  pickerControllerReducer,
} from "./pickerModel.ts";

function createElement(overrides = {}) {
  return {
    elementId: "editor",
    role: "Document",
    name: "Text editor",
    automationId: null,
    className: "RichEditD2DPT",
    runtimeId: "42,1,0",
    isEnabled: true,
    isFocused: true,
    isKeyboardFocusable: true,
    actions: ["focus"],
    patterns: ["text"],
    bounds: { x: 20, y: 60, width: 700, height: 500 },
    parentPath: [
      {
        role: "Pane",
        name: "Notepad text box",
        automationId: null,
        className: "NotepadTextBox",
      },
    ],
    ...overrides,
  };
}

function createSnapshot(overrides = {}) {
  return {
    appId: "com.microsoft.notepad",
    windowTitle: "Untitled - Notepad",
    workspaceBounds: { x: 0, y: 0, width: 1000, height: 700 },
    window: {
      elementId: "window",
      role: "Window",
      name: "Untitled - Notepad",
      automationId: null,
      className: "Notepad",
      runtimeId: "11,1,0",
      isEnabled: true,
      isFocused: true,
      isKeyboardFocusable: false,
      actions: [],
      patterns: ["window"],
      bounds: { x: 0, y: 0, width: 1000, height: 700 },
      parentPath: [],
    },
    elements: [createElement()],
    ...overrides,
  };
}

test("test_parse_picker_snapshot_valid_model_preserves_properties", () => {
  const result = parsePickerSnapshot(createSnapshot());
  assert.equal(result.ok, true);
  assert.equal(result.snapshot.elements[0]?.className, "RichEditD2DPT");
  assert.equal(result.snapshot.elements[0]?.isFocused, true);
});

test("test_parse_picker_snapshot_duplicate_element_id_is_rejected", () => {
  const result = parsePickerSnapshot(
    createSnapshot({
      elements: [createElement({ elementId: "window" })],
    }),
  );
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /duplicate elementId/);
});

test("test_parse_picker_snapshot_out_of_workspace_bounds_is_rejected", () => {
  const result = parsePickerSnapshot(
    createSnapshot({
      elements: [createElement({ bounds: { x: 900, y: 60, width: 200, height: 100 } })],
    }),
  );
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /exceed workspaceBounds/);
});

test("test_picker_notepad_editor_generates_three_threshold_eligible_candidates", () => {
  const snapshotResult = parsePickerSnapshot(createSnapshot());
  assert.equal(snapshotResult.ok, true);
  const element = snapshotResult.snapshot.elements[0];
  assert.ok(element);
  const candidates = generateSelectorCandidates(element);
  assert.equal(candidates.filter((candidate) => candidate.score > 0).length, 3);
  assert.equal(
    candidates.some((candidate) => candidate.kind === "role_and_parent"),
    false,
  );
  assert.equal(
    candidates.some((candidate) => candidate.score === 0),
    false,
  );
  const primaryId = getPrimaryCandidateId(candidates);
  const primary = candidates.find((candidate) => candidate.id === primaryId);
  assert.equal(primary?.locale_dependent, false);
  assert.notEqual(primary?.kind, "name_regex");
});

test("test_picker_validation_rejects_localized_candidate_out_ranking_stable", () => {
  const candidates = [
    {
      id: "name-fallback",
      kind: "name_regex",
      value: { text: "Editor" },
      score: 1,
      locale_dependent: true,
      ttl_ms: null,
    },
    {
      id: "automation-id",
      kind: "automation_id",
      value: { text: "Editor" },
      score: 0.1,
      locale_dependent: false,
      ttl_ms: null,
    },
  ];
  assert.equal(getPrimaryCandidateId(candidates), null);
});

test("test_picker_validation_rejects_unsorted_or_ephemeral_candidates", () => {
  const unsorted = [
    {
      id: "class-and-role",
      kind: "class_and_role",
      value: { class_and_role: { class: "Editor", role: "Document" } },
      score: 0.8,
      locale_dependent: false,
      ttl_ms: null,
    },
    {
      id: "automation-id",
      kind: "automation_id",
      value: { text: "Editor" },
      score: 0.9,
      locale_dependent: false,
      ttl_ms: null,
    },
  ];
  assert.equal(getPrimaryCandidateId(unsorted), null);
  assert.equal(
    getPrimaryCandidateId([
      {
        id: "runtime-id",
        kind: "runtime_id",
        value: { text: "42,1,0" },
        score: 1,
        locale_dependent: false,
        ttl_ms: 0,
      },
    ]),
    null,
  );
});

test("test_picker_validation_rejects_kind_value_mismatch_and_invalid_score", () => {
  const mismatched = [
    {
      id: "automation-id",
      kind: "automation_id",
      value: { class_and_role: { class: "Editor", role: "Document" } },
      score: 0.9,
      locale_dependent: false,
      ttl_ms: null,
    },
  ];
  assert.equal(getPrimaryCandidateId(mismatched), null);
  const invalidScore = [
    {
      id: "automation-id",
      kind: "automation_id",
      value: { text: "Editor" },
      score: 1.1,
      locale_dependent: false,
      ttl_ms: null,
    },
  ];
  assert.equal(getPrimaryCandidateId(invalidScore), null);
  const unknownKind = [
    {
      id: "unknown",
      kind: "made_up",
      value: { text: "Editor" },
      score: 0.9,
      locale_dependent: false,
      ttl_ms: null,
    },
  ];
  assert.equal(getPrimaryCandidateId(unknownKind), null);
});

test("test_picker_hover_and_generate_reducer_tracks_selection", () => {
  let state = createInitialPickerControllerState(createSnapshot());
  state = pickerControllerReducer(state, { type: "hover_element", elementId: "editor" });
  assert.equal(state.hoveredElementId, "editor");
  state = pickerControllerReducer(state, { type: "select_element", elementId: "editor" });
  state = pickerControllerReducer(state, { type: "generate_candidates" });
  assert.equal(state.errorKey, null);
  assert.equal(state.candidates.length >= 3, true);
});

test("test_picker_generate_without_selection_fails_closed", () => {
  const state = createInitialPickerControllerState(createSnapshot());
  const next = pickerControllerReducer(state, { type: "generate_candidates" });
  assert.equal(next.errorKey, "picker.error.selection_required");
});

test("test_picker_highlight_rect_uses_validated_workspace_coordinates", () => {
  const snapshotResult = parsePickerSnapshot(createSnapshot());
  assert.equal(snapshotResult.ok, true);
  const element = snapshotResult.snapshot.elements[0];
  assert.ok(element);
  assert.deepEqual(getHighlightRect(element, snapshotResult.snapshot.workspaceBounds), {
    leftPercent: 2,
    topPercent: 8.571428571428571,
    widthPercent: 70,
    heightPercent: 71.42857142857143,
  });
});

test("test_picker_nonzero_workspace_origin_is_enforced_and_subtracted", () => {
  const input = createSnapshot({
    workspaceBounds: { x: 100, y: 200, width: 1000, height: 700 },
    window: {
      ...createSnapshot().window,
      bounds: { x: 100, y: 200, width: 1000, height: 700 },
    },
    elements: [createElement({ bounds: { x: 120, y: 260, width: 700, height: 500 } })],
  });
  const result = parsePickerSnapshot(input);
  assert.equal(result.ok, true);
  const element = result.snapshot.elements[0];
  assert.ok(element);
  assert.deepEqual(getHighlightRect(element, result.snapshot.workspaceBounds), {
    leftPercent: 2,
    topPercent: 8.571428571428571,
    widthPercent: 70,
    heightPercent: 71.42857142857143,
  });
  const outside = parsePickerSnapshot({
    ...input,
    elements: [createElement({ bounds: { x: 90, y: 260, width: 700, height: 500 } })],
  });
  assert.equal(outside.ok, false);
  assert.match(outside.errors.join("\n"), /exceed workspaceBounds/);
});
