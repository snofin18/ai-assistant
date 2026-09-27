import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import test from "node:test";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.endsWith(".js")) {
      return nextResolve(`${specifier.slice(0, -3)}.ts`, context);
    }
    return nextResolve(specifier, context);
  }
});

const {
  bindingWizardReducer,
  createAdapterSelectorDraft,
  createInitialBindingWizardState,
  getBindingBlockReason,
  parseBindingWizardInput
} = await import("./bindingModel.ts");

function createInput() {
  return {
    metadata: {
      appId: "com.microsoft.notepad",
      displayName: "Notepad",
      targetName: "editor",
      versionRange: ">=11 <12"
    },
    snapshot: {
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
        parentPath: []
      },
      elements: [
        {
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
              className: "NotepadTextBox"
            }
          ]
        }
      ]
    }
  };
}

test("test_parse_binding_input_requires_matching_app_ids", () => {
  const input = createInput();
  input.metadata.appId = "com.example.other";
  const result = parseBindingWizardInput(input);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /must match snapshot\.appId/);
});

test("test_parse_binding_input_rejects_non_object_without_throwing", () => {
  const result = parseBindingWizardInput(null);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /binding input must be an object/);
});

test("test_binding_wizard_generates_serialized_descriptor_draft", () => {
  const input = createInput();
  let state = createInitialBindingWizardState(input);
  state = bindingWizardReducer(state, { type: "continue" });
  state = bindingWizardReducer(state, { type: "select_element", elementId: "editor" });
  state = bindingWizardReducer(state, { type: "continue" });
  state = bindingWizardReducer(state, { type: "continue" });
  assert.equal(state.step, "export");
  assert.notEqual(state.serializedDraft, null);
  const draft = JSON.parse(state.serializedDraft);
  assert.equal(draft.descriptor.descriptor_version, "2.0");
  assert.equal(draft.descriptor.element_candidates.length >= 3, true);
  assert.equal(draft.descriptor.resolution_policy.on_ambiguous, "error_and_ask");
});

test("test_binding_without_selection_fails_closed", () => {
  let state = createInitialBindingWizardState(createInput());
  state = bindingWizardReducer(state, { type: "continue" });
  state = bindingWizardReducer(state, { type: "continue" });
  assert.equal(state.errorKey, "binding.error.selection_required");
});

test("test_binding_only_locale_dependent_candidates_is_rejected", () => {
  const input = createInput();
  const snapshot = input.snapshot;
  snapshot.elements[0].className = null;
  snapshot.elements[0].parentPath = [];
  const draft = createAdapterSelectorDraft(
    snapshot,
    input.metadata,
    "editor"
  );
  assert.equal(draft.ok, false);
  assert.equal(draft.errorKey, "binding.error.candidates_invalid");
});

test("test_binding_window_without_stable_candidate_is_rejected", () => {
  const input = createInput();
  input.snapshot.window.className = null;
  input.snapshot.window.name = "";
  const draft = createAdapterSelectorDraft(
    input.snapshot,
    input.metadata,
    "editor"
  );
  assert.equal(draft.ok, false);
  assert.equal(draft.errorKey, "binding.error.window_candidates_invalid");
});

test("test_binding_block_reason_requires_three_resolvable_candidates", () => {
  const candidates = [
    {
      id: "automation-id",
      kind: "automation_id",
      value: { text: "Editor" },
      score: 0.98,
      locale_dependent: false,
      ttl_ms: null
    },
    {
      id: "name-fallback",
      kind: "name_regex",
      value: { text: "Text editor" },
      score: 0.2,
      locale_dependent: true,
      ttl_ms: null
    }
  ];
  assert.equal(
    getBindingBlockReason(candidates),
    "binding.error.candidate_count_insufficient"
  );
});
