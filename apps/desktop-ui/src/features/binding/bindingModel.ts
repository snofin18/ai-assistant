/**
 * Target binding wizard model.
 *
 * The wizard consumes validated picker snapshots and produces an in-memory
 * Adapter selector draft. It does not persist files or define a published
 * schema; callers own the host integration boundary.
 */

import {
  generateSelectorCandidates,
  parsePickerSnapshot,
  validateSelectorCandidates
} from "../picker/pickerModel.js";
import type {
  PickerSnapshot,
  SelectorCandidate
} from "../picker/pickerModel.js";

export const bindingWizardSteps = [
  "application",
  "target",
  "review",
  "export"
] as const;
export type BindingWizardStep = (typeof bindingWizardSteps)[number];
export const MIN_SCORE_TO_TRY = 0.15;

export interface AdapterMetadata {
  appId: string;
  displayName: string;
  targetName: string;
  versionRange: string;
}

export interface ResolutionPolicyDraft {
  on_ambiguous: "error_and_ask";
  on_not_found: {
    retry_after_ms: number[];
    then_escalate: true;
  };
  max_resolve_ms: number;
  min_score_to_try: number;
}

export interface TargetDescriptorDraft {
  descriptor_version: "2.0";
  app_id: string;
  window_candidates: SelectorCandidate[];
  element_candidates: SelectorCandidate[];
  resolution_policy: ResolutionPolicyDraft;
}

export interface AdapterSelectorDraft {
  draft_version: "0.1";
  app_id: string;
  display_name: string;
  target_name: string;
  version_range: string;
  descriptor: TargetDescriptorDraft;
}

export interface BindingWizardInput {
  metadata: unknown;
  snapshot: unknown;
}

export interface ParsedBindingWizardInput {
  metadata: AdapterMetadata;
  snapshot: PickerSnapshot;
}

export type BindingInputParseResult =
  | { ok: true; value: ParsedBindingWizardInput }
  | { ok: false; errors: string[] };

export interface BindingWizardState {
  parseResult: BindingInputParseResult;
  step: BindingWizardStep;
  selectedElementId: string | null;
  candidates: SelectorCandidate[];
  serializedDraft: string | null;
  errorKey: string | null;
}

export type BindingWizardAction =
  | { type: "replace_input"; input: unknown }
  | { type: "select_element"; elementId: string }
  | { type: "continue" }
  | { type: "back" }
  | { type: "reset" };

export function parseBindingWizardInput(input: unknown): BindingInputParseResult {
  if (!isRecord(input)) {
    return { ok: false, errors: ["binding input must be an object"] };
  }
  const metadataResult = parseAdapterMetadata(input.metadata);
  const snapshotResult = parsePickerSnapshot(input.snapshot);
  if (!metadataResult.ok || !snapshotResult.ok) {
    return {
      ok: false,
      errors: [
        ...(!metadataResult.ok ? metadataResult.errors : []),
        ...(!snapshotResult.ok ? snapshotResult.errors : [])
      ]
    };
  }
  if (metadataResult.metadata.appId !== snapshotResult.snapshot.appId) {
    return { ok: false, errors: ["metadata.appId must match snapshot.appId"] };
  }
  return {
    ok: true,
    value: { metadata: metadataResult.metadata, snapshot: snapshotResult.snapshot }
  };
}

export function createInitialBindingWizardState(
  input: unknown
): BindingWizardState {
  return {
    parseResult: parseBindingWizardInput(input),
    step: "application",
    selectedElementId: null,
    candidates: [],
    serializedDraft: null,
    errorKey: null
  };
}

export function bindingWizardReducer(
  state: BindingWizardState,
  action: BindingWizardAction
): BindingWizardState {
  if (action.type === "replace_input") {
    return createInitialBindingWizardState(action.input);
  }
  if (action.type === "reset") {
    return { ...state, step: "application", serializedDraft: null, errorKey: null };
  }
  if (!state.parseResult.ok) {
    return state;
  }
  if (action.type === "select_element") {
    const element = getSnapshotElement(
      state.parseResult.value.snapshot,
      action.elementId
    );
    if (element === null) {
      return { ...state, errorKey: "binding.error.element_not_found" };
    }
    const candidates = generateSelectorCandidates(element);
    if (!validateSelectorCandidates(candidates)) {
      return { ...state, errorKey: "binding.error.candidates_invalid" };
    }
    return {
      ...state,
      step: "target",
      selectedElementId: element.elementId,
      candidates,
      serializedDraft: null,
      errorKey: null
    };
  }
  if (action.type === "back") {
    const index = bindingWizardSteps.indexOf(state.step);
    const previous = index <= 0 ? "application" : bindingWizardSteps[index - 1];
    return { ...state, step: previous ?? "application", errorKey: null };
  }
  if (state.step === "target" && state.selectedElementId === null) {
    return { ...state, errorKey: "binding.error.selection_required" };
  }
  if (state.step === "application") {
    return { ...state, step: "target", errorKey: null };
  }
  if (state.step === "target") {
    const blockReason = getBindingBlockReason(state.candidates);
    return blockReason === null
      ? { ...state, step: "review", errorKey: null }
      : { ...state, errorKey: blockReason };
  }
  if (state.step === "review") {
    if (state.selectedElementId === null) {
      return { ...state, errorKey: "binding.error.selection_required" };
    }
    const draft = createAdapterSelectorDraft(
      state.parseResult.value.snapshot,
      state.parseResult.value.metadata,
      state.selectedElementId
    );
    return draft.ok
      ? { ...state, step: "export", serializedDraft: draft.serialized, errorKey: null }
      : { ...state, errorKey: draft.errorKey };
  }
  return state;
}

export function createAdapterSelectorDraft(
  snapshotInput: unknown,
  metadataInput: unknown,
  elementId: unknown
):
  | { ok: true; draft: AdapterSelectorDraft; serialized: string }
  | { ok: false; errorKey: string } {
  if (typeof elementId !== "string" || elementId.trim() === "") {
    return { ok: false, errorKey: "binding.error.element_not_found" };
  }
  const metadataResult = parseAdapterMetadata(metadataInput);
  const snapshotResult = parsePickerSnapshot(snapshotInput);
  if (!metadataResult.ok || !snapshotResult.ok) {
    return { ok: false, errorKey: "binding.error.input_invalid" };
  }
  const { metadata } = metadataResult;
  const { snapshot } = snapshotResult;
  if (metadata.appId !== snapshot.appId) {
    return { ok: false, errorKey: "binding.error.app_id_mismatch" };
  }
  const element = getSnapshotElement(snapshot, elementId);
  if (element === null) {
    return { ok: false, errorKey: "binding.error.element_not_found" };
  }
  const windowCandidates = generateSelectorCandidates(snapshot.window);
  if (!validateSelectorCandidates(windowCandidates)) {
    return { ok: false, errorKey: "binding.error.window_candidates_invalid" };
  }
  const elementCandidates = generateSelectorCandidates(element);
  const blockReason = getBindingBlockReason(elementCandidates);
  if (blockReason !== null) {
    return { ok: false, errorKey: blockReason };
  }
  const draft: AdapterSelectorDraft = {
    draft_version: "0.1",
    app_id: metadata.appId,
    display_name: metadata.displayName,
    target_name: metadata.targetName,
    version_range: metadata.versionRange,
    descriptor: {
      descriptor_version: "2.0",
      app_id: metadata.appId,
      window_candidates: windowCandidates,
      element_candidates: elementCandidates,
      resolution_policy: {
        on_ambiguous: "error_and_ask",
        on_not_found: {
          retry_after_ms: [50, 150, 400],
          then_escalate: true
        },
        max_resolve_ms: 3000,
        min_score_to_try: MIN_SCORE_TO_TRY
      }
    }
  };
  return { ok: true, draft, serialized: JSON.stringify(draft, null, 2) };
}

export function getBindingBlockReason(
  candidates: readonly SelectorCandidate[]
): string | null {
  if (!validateSelectorCandidates(candidates)) {
    return "binding.error.candidates_invalid";
  }
  if (
    candidates.filter((candidate) => candidate.score >= MIN_SCORE_TO_TRY).length < 3
  ) {
    return "binding.error.candidate_count_insufficient";
  }
  return null;
}

export function parseAdapterMetadata(
  input: unknown
): { ok: true; metadata: AdapterMetadata } | { ok: false; errors: string[] } {
  if (!isRecord(input)) {
    return { ok: false, errors: ["metadata must be an object"] };
  }
  const errors: string[] = [];
  const appId = readText(input, "appId", errors);
  const displayName = readText(input, "displayName", errors);
  const targetName = readText(input, "targetName", errors);
  const versionRange = readText(input, "versionRange", errors);
  if (appId === null || displayName === null || targetName === null || versionRange === null) {
    return { ok: false, errors };
  }
  if (!/^[a-z0-9]+(?:\.[a-z0-9_-]+)+$/.test(appId)) {
    errors.push("metadata.appId must be a reverse-domain identifier");
  }
  return errors.length === 0
    ? { ok: true, metadata: { appId, displayName, targetName, versionRange } }
    : { ok: false, errors };
}

function getSnapshotElement(snapshot: PickerSnapshot, elementId: string) {
  if (snapshot.window.elementId === elementId) {
    return snapshot.window;
  }
  return snapshot.elements.find((element) => element.elementId === elementId) ?? null;
}

function readText(
  record: Record<string, unknown>,
  key: string,
  errors: string[]
): string | null {
  const value = record[key];
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(`metadata.${key} must be a non-empty string`);
    return null;
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
