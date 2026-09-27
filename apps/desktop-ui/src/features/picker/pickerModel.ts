/**
 * Element picker view model.
 *
 * All snapshots cross `parsePickerSnapshot` before rendering. Candidate
 * generation is pure and fail-closed: locale-dependent values may only be
 * fallbacks, never the primary selector.
 */

export const selectorKinds = [
  "runtime_id",
  "automation_id",
  "ax_identifier",
  "class_and_role",
  "role_and_parent",
  "a11y_path",
  "title_regex",
  "name_regex",
  "visual_anchor"
] as const;
export type SelectorKind = (typeof selectorKinds)[number];

export interface PickerBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PickerAncestor {
  role: string;
  name: string;
  automationId: string | null;
  className: string | null;
}

export interface PickerElement {
  elementId: string;
  role: string;
  name: string;
  automationId: string | null;
  className: string | null;
  runtimeId: string | null;
  isEnabled: boolean;
  isFocused: boolean;
  isKeyboardFocusable: boolean;
  actions: string[];
  patterns: string[];
  bounds: PickerBounds;
  parentPath: PickerAncestor[];
}

export interface PickerSnapshot {
  appId: string;
  windowTitle: string;
  workspaceBounds: PickerBounds;
  window: PickerElement;
  elements: PickerElement[];
}

export type SelectorValue =
  | { text: string }
  | { class_and_role: { class: string; role: string } }
  | { role_and_parent: { role: string; parent_id: string } }
  | { path: string[] }
  | { visual_anchor: { ocr_text: string; region: string } };

export interface SelectorCandidate {
  id: string;
  kind: SelectorKind;
  value: SelectorValue;
  score: number;
  locale_dependent: boolean;
  ttl_ms: number | null;
}

export type PickerParseResult =
  | { ok: true; snapshot: PickerSnapshot }
  | { ok: false; errors: string[] };

export interface PickerControllerState {
  parseResult: PickerParseResult;
  hoveredElementId: string | null;
  selectedElementId: string | null;
  candidates: SelectorCandidate[];
  errorKey: string | null;
}

export type PickerControllerAction =
  | { type: "replace_snapshot"; snapshot: unknown }
  | { type: "hover_element"; elementId: string | null }
  | { type: "select_element"; elementId: string }
  | { type: "generate_candidates" }
  | { type: "clear_selection" };

export interface HighlightRect {
  leftPercent: number;
  topPercent: number;
  widthPercent: number;
  heightPercent: number;
}

export function parsePickerSnapshot(input: unknown): PickerParseResult {
  if (!isRecord(input)) {
    return { ok: false, errors: ["snapshot must be an object"] };
  }
  const errors: string[] = [];
  const appId = readText(input, "appId", errors);
  const windowTitle = readText(input, "windowTitle", errors);
  const workspaceBounds = parseBounds(input.workspaceBounds, "workspaceBounds", errors);
  const windowElement = parseElement(input.window, "window", errors);
  const elements = parseElements(input.elements, errors);
  if (
    appId === null ||
    windowTitle === null ||
    workspaceBounds === null ||
    windowElement === null ||
    elements === null
  ) {
    return { ok: false, errors };
  }
  const elementIds = new Set<string>([windowElement.elementId]);
  for (const element of elements) {
    if (elementIds.has(element.elementId)) {
      errors.push(`duplicate elementId: ${element.elementId}`);
    }
    elementIds.add(element.elementId);
    if (!containsBounds(workspaceBounds, element.bounds)) {
      errors.push(`element ${element.elementId} bounds exceed workspaceBounds`);
    }
  }
  if (!containsBounds(workspaceBounds, windowElement.bounds)) {
    errors.push("window bounds exceed workspaceBounds");
  }
  if (errors.length > 0) {
    return { ok: false, errors };
  }
  return {
    ok: true,
    snapshot: { appId, windowTitle, workspaceBounds, window: windowElement, elements }
  };
}

export function createInitialPickerControllerState(input: unknown): PickerControllerState {
  return {
    parseResult: parsePickerSnapshot(input),
    hoveredElementId: null,
    selectedElementId: null,
    candidates: [],
    errorKey: null
  };
}

export function pickerControllerReducer(
  state: PickerControllerState,
  action: PickerControllerAction
): PickerControllerState {
  if (action.type === "replace_snapshot") {
    return createInitialPickerControllerState(action.snapshot);
  }
  if (!state.parseResult.ok) {
    return state;
  }
  const snapshot = state.parseResult.snapshot;
  switch (action.type) {
    case "hover_element":
      if (action.elementId === null) {
        return { ...state, hoveredElementId: null, errorKey: null };
      }
      return getElementById(snapshot, action.elementId) === null
        ? { ...state, errorKey: "picker.error.element_not_found" }
        : { ...state, hoveredElementId: action.elementId, errorKey: null };
    case "select_element": {
      const element = getElementById(snapshot, action.elementId);
      if (element === null) {
        return { ...state, errorKey: "picker.error.element_not_found" };
      }
      return {
        ...state,
        selectedElementId: element.elementId,
        candidates: [],
        errorKey: null
      };
    }
    case "generate_candidates": {
      if (state.selectedElementId === null) {
        return { ...state, errorKey: "picker.error.selection_required" };
      }
      const element = getElementById(snapshot, state.selectedElementId);
      if (element === null) {
        return { ...state, errorKey: "picker.error.element_not_found" };
      }
      const candidates = generateSelectorCandidates(element);
      if (!validateSelectorCandidates(candidates)) {
        return { ...state, errorKey: "picker.error.candidates_invalid" };
      }
      return { ...state, candidates, errorKey: null };
    }
    case "clear_selection":
      return {
        ...state,
        hoveredElementId: null,
        selectedElementId: null,
        candidates: [],
        errorKey: null
      };
    default:
      return state;
  }
}

export function getElementById(
  snapshot: PickerSnapshot,
  elementId: string
): PickerElement | null {
  if (snapshot.window.elementId === elementId) {
    return snapshot.window;
  }
  return snapshot.elements.find((element) => element.elementId === elementId) ?? null;
}

export function generateSelectorCandidates(element: PickerElement): SelectorCandidate[] {
  const candidates: SelectorCandidate[] = [];
  if (element.automationId !== null) {
    candidates.push({
      id: "automation-id",
      kind: "automation_id",
      value: { text: element.automationId },
      score: 0.98,
      locale_dependent: false,
      ttl_ms: null
    });
  }
  if (element.className !== null) {
    candidates.push({
      id: "class-and-role",
      kind: "class_and_role",
      value: {
        class_and_role: { class: element.className, role: element.role }
      },
      score: 0.84,
      locale_dependent: false,
      ttl_ms: null
    });
  }
  if (element.name.trim() !== "") {
    candidates.push({
      id: "name-fallback",
      kind: "name_regex",
      value: { text: element.name },
      score: 0.22,
      locale_dependent: true,
      ttl_ms: null
    });
  }
  if (element.parentPath.length > 0) {
    candidates.push({
      id: "a11y-path-fallback",
      kind: "a11y_path",
      value: {
        path: element.parentPath.map((ancestor) => ancestor.name.trim() || ancestor.role)
      },
      score: 0.16,
      locale_dependent: true,
      ttl_ms: null
    });
  }
  return candidates.sort((left, right) => right.score - left.score);
}

export function validateSelectorCandidates(candidates: readonly SelectorCandidate[]): boolean {
  if (candidates.length === 0) {
    return false;
  }
  const ids = new Set<string>();
  let previousScore = Number.POSITIVE_INFINITY;
  let maximumLocaleScore = 0;
  for (const candidate of candidates) {
    if (
      candidate.id.trim() === "" ||
      ids.has(candidate.id) ||
      !selectorKinds.includes(candidate.kind) ||
      !Number.isFinite(candidate.score) ||
      candidate.score < 0 ||
      candidate.score > 1 ||
      candidate.score > previousScore ||
      candidate.kind === "runtime_id" ||
      candidate.kind === "role_and_parent" ||
      !isValidSelectorValue(candidate) ||
      !isValidTtl(candidate.ttl_ms)
    ) {
      return false;
    }
    previousScore = candidate.score;
    ids.add(candidate.id);
    if (
      (candidate.kind === "name_regex" ||
        candidate.kind === "title_regex" ||
        candidate.kind === "a11y_path" ||
        candidate.kind === "visual_anchor") &&
      !candidate.locale_dependent
    ) {
      return false;
    }
    if (candidate.locale_dependent) {
      if (candidate.score > 0.49) {
        return false;
      }
      maximumLocaleScore = Math.max(maximumLocaleScore, candidate.score);
    }
  }
  const primary = candidates[0];
  return (
    primary !== undefined &&
    !primary.locale_dependent &&
    primary.score > 0 &&
    primary.score > maximumLocaleScore
  );
}

function isValidSelectorValue(candidate: SelectorCandidate): boolean {
  const value = candidate.value;
  switch (candidate.kind) {
    case "automation_id":
    case "ax_identifier":
    case "title_regex":
    case "name_regex":
      return "text" in value && value.text.trim() !== "";
    case "class_and_role":
      return (
        "class_and_role" in value &&
        value.class_and_role.class.trim() !== "" &&
        value.class_and_role.role.trim() !== ""
      );
    case "a11y_path":
      return (
        "path" in value &&
        value.path.length > 0 &&
        value.path.every((segment) => segment.trim() !== "")
      );
    case "visual_anchor":
      return (
        "visual_anchor" in value &&
        value.visual_anchor.ocr_text.trim() !== "" &&
        value.visual_anchor.region.trim() !== ""
      );
    case "role_and_parent":
    case "runtime_id":
      return false;
    default:
      return false;
  }
}

function isValidTtl(value: number | null): boolean {
  return value === null || (Number.isSafeInteger(value) && value >= 0);
}

export function getPrimaryCandidateId(candidates: readonly SelectorCandidate[]): string | null {
  if (!validateSelectorCandidates(candidates)) {
    return null;
  }
  return candidates[0]?.id ?? null;
}

export function getHighlightRect(
  element: PickerElement,
  workspaceBounds: PickerBounds
): HighlightRect {
  return {
    leftPercent: ((element.bounds.x - workspaceBounds.x) / workspaceBounds.width) * 100,
    topPercent: ((element.bounds.y - workspaceBounds.y) / workspaceBounds.height) * 100,
    widthPercent: (element.bounds.width / workspaceBounds.width) * 100,
    heightPercent: (element.bounds.height / workspaceBounds.height) * 100
  };
}

function parseElements(value: unknown, errors: string[]): PickerElement[] | null {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push("elements must be a non-empty array");
    return null;
  }
  const elements: PickerElement[] = [];
  for (const [index, entry] of value.entries()) {
    const element = parseElement(entry, `elements[${index}]`, errors);
    if (element !== null) {
      elements.push(element);
    }
  }
  return elements.length === value.length ? elements : null;
}

function parseElement(value: unknown, prefix: string, errors: string[]): PickerElement | null {
  if (!isRecord(value)) {
    errors.push(`${prefix} must be an object`);
    return null;
  }
  const elementId = readText(value, "elementId", errors, prefix);
  const role = readText(value, "role", errors, prefix);
  const name = readText(value, "name", errors, prefix, true);
  const automationId = readOptionalText(value, "automationId", errors, prefix);
  const className = readOptionalText(value, "className", errors, prefix);
  const runtimeId = readOptionalText(value, "runtimeId", errors, prefix);
  const isEnabled = value.isEnabled;
  const isFocused = value.isFocused;
  const isKeyboardFocusable = value.isKeyboardFocusable;
  const actions = readStringArray(value.actions, "actions", errors, prefix);
  const patterns = readStringArray(value.patterns, "patterns", errors, prefix);
  const bounds = parseBounds(value.bounds, `${prefix}.bounds`, errors);
  const parentPath = parseParentPath(value.parentPath, `${prefix}.parentPath`, errors);
  if (
    elementId === null ||
    role === null ||
    name === null ||
    automationId === undefined ||
    className === undefined ||
    runtimeId === undefined ||
    typeof isEnabled !== "boolean" ||
    typeof isFocused !== "boolean" ||
    typeof isKeyboardFocusable !== "boolean" ||
    actions === null ||
    patterns === null ||
    bounds === null ||
    parentPath === null
  ) {
    return null;
  }
  return {
    elementId,
    role,
    name,
    automationId,
    className,
    runtimeId,
    isEnabled,
    isFocused,
    isKeyboardFocusable,
    actions,
    patterns,
    bounds,
    parentPath
  };
}

function parseParentPath(
  value: unknown,
  prefix: string,
  errors: string[]
): PickerAncestor[] | null {
  if (!Array.isArray(value)) {
    errors.push(`${prefix} must be an array`);
    return null;
  }
  const ancestors: PickerAncestor[] = [];
  for (const [index, entry] of value.entries()) {
    const entryPrefix = `${prefix}[${index}]`;
    if (!isRecord(entry)) {
      errors.push(`${entryPrefix} must be an object`);
      return null;
    }
    const role = readText(entry, "role", errors, entryPrefix);
    const name = readText(entry, "name", errors, entryPrefix, true);
    const automationId = readOptionalText(entry, "automationId", errors, entryPrefix);
    const className = readOptionalText(entry, "className", errors, entryPrefix);
    if (
      role === null ||
      name === null ||
      automationId === undefined ||
      className === undefined
    ) {
      return null;
    }
    ancestors.push({ role, name, automationId, className });
  }
  return ancestors;
}

function parseBounds(value: unknown, prefix: string, errors: string[]): PickerBounds | null {
  if (!isRecord(value)) {
    errors.push(`${prefix} must be an object`);
    return null;
  }
  const { x, y, width, height } = value;
  if (
    !isNonNegativeFinite(x) ||
    !isNonNegativeFinite(y) ||
    !isPositiveFinite(width) ||
    !isPositiveFinite(height)
  ) {
    errors.push(`${prefix} must contain finite bounds with positive width and height`);
    return null;
  }
  return { x, y, width, height };
}

function readStringArray(
  value: unknown,
  key: string,
  errors: string[],
  prefix: string
): string[] | null {
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === "string")) {
    errors.push(`${prefix}.${key} must be a string array`);
    return null;
  }
  const normalized = value.map((entry) => entry.trim()).filter((entry) => entry !== "");
  if (new Set(normalized).size !== normalized.length) {
    errors.push(`${prefix}.${key} must not contain duplicates`);
    return null;
  }
  return normalized;
}

function readText(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
  prefix = "snapshot",
  allowEmpty = false
): string | null {
  const value = record[key];
  if (typeof value !== "string" || (!allowEmpty && value.trim() === "")) {
    errors.push(`${prefix}.${key} must be ${allowEmpty ? "a string" : "a non-empty string"}`);
    return null;
  }
  return value;
}

function readOptionalText(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
  prefix: string
): string | null | undefined {
  const value = record[key];
  if (value === null || (typeof value === "string" && value.trim() !== "")) {
    return value;
  }
  errors.push(`${prefix}.${key} must be a non-empty string or null`);
  return undefined;
}

function containsBounds(workspace: PickerBounds, element: PickerBounds): boolean {
  return (
    element.x >= workspace.x &&
    element.y >= workspace.y &&
    element.x + element.width <= workspace.x + workspace.width &&
    element.y + element.height <= workspace.y + workspace.height
  );
}

function isNonNegativeFinite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

function isPositiveFinite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
