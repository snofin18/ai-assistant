/**
 * Approval card view model and interaction controller.
 *
 * UI input is untrusted: every model crosses `parseApprovalCardModel` before a
 * component can render it. This module does not call IPC and never authorizes
 * an action; policy and HITL remain the only permission authorities.
 */

export const approvalRiskLevels = ["low", "medium", "high", "critical"] as const;
export type ApprovalRiskLevel = (typeof approvalRiskLevels)[number];

export const reversibilityLevels = [
  "L0UndoStack",
  "L1ContentSnapshot",
  "L2ShadowCopy",
  "L3Irreversible",
] as const;
export type ReversibilityLevel = (typeof reversibilityLevels)[number];

export const instructionOrigins = [
  "user_request",
  "plan_derived",
  "app_content",
  "tool_suggestion",
] as const;
export type InstructionOrigin = (typeof instructionOrigins)[number];

export const authorizationScopes = [
  "once",
  "this_step_pattern",
  "this_task",
  "this_app_session",
  "persistent",
] as const;
export type AuthorizationScope = (typeof authorizationScopes)[number];

export interface ApprovalTextDiffEntry {
  kind: "added" | "removed";
  beforeLine: number | null;
  afterLine: number | null;
  text: string;
}

export interface ApprovalFieldDiff {
  field: string;
  before: string;
  after: string;
}

export interface ApprovalFileDiff {
  path: string;
  kind: "added" | "removed" | "modified";
  beforeBytes: number | null;
  afterBytes: number | null;
}

export interface ApprovalUiStepDiff {
  stepId: string;
  action: string;
  targetLabel: string;
  screenshotRef: string | null;
}

export type ApprovalDiff =
  | { kind: "text"; entries: ApprovalTextDiffEntry[] }
  | { kind: "fields"; entries: ApprovalFieldDiff[] }
  | { kind: "files"; entries: ApprovalFileDiff[] }
  | { kind: "ui_steps"; entries: ApprovalUiStepDiff[] }
  | { kind: "irreversible"; warningKey: string }
  | { kind: "none" };

export interface ApprovalEvidence {
  id: string;
  kind: "tree_snapshot" | "screenshot" | "log";
  labelKey: string;
  href: string;
}

export interface ApprovalUndoPlan {
  isAvailable: boolean;
  methodKey: string;
  anchorId: string | null;
  disabledReasonKey: string | null;
}

export interface ApprovalCardModel {
  requestId: string;
  action: string;
  targetLabel: string;
  summary: string;
  impact: string;
  diff: ApprovalDiff;
  instructionOrigin: InstructionOrigin;
  toolSelectionReason: string;
  riskLevel: ApprovalRiskLevel;
  reversibility: ReversibilityLevel;
  undo: ApprovalUndoPlan;
  scopeOptions: AuthorizationScope[];
  evidence: ApprovalEvidence[];
  isExpired: boolean;
}

export type ApprovalModelParseResult =
  { ok: true; model: ApprovalCardModel } | { ok: false; errors: string[] };

export interface ApprovalControllerState {
  parseResult: ApprovalModelParseResult;
  selectedScope: AuthorizationScope | null;
  hasAppContentOverride: boolean;
  irreversibleConfirmation: string;
  denialReason: string;
  errorKey: string | null;
  decision: ApprovalDecisionEvent | null;
}

export type ApprovalDecisionEvent =
  | {
      kind: "approved";
      requestId: string;
      action: string;
      scope: AuthorizationScope;
      instructionOrigin: InstructionOrigin;
    }
  | { kind: "denied"; requestId: string; reason: string };

export type ApprovalControllerAction =
  | { type: "replace_model"; model: unknown }
  | { type: "select_scope"; scope: AuthorizationScope }
  | { type: "set_app_content_override"; isEnabled: boolean }
  | { type: "set_irreversible_confirmation"; confirmation: string }
  | { type: "set_denial_reason"; reason: string }
  | { type: "approve"; requiredIrreversibleConfirmation: string }
  | { type: "deny" }
  | { type: "reset_decision" };

const highRiskLevels = new Set<ApprovalRiskLevel>(["high", "critical"]);

export function isHighRisk(model: ApprovalCardModel): boolean {
  return highRiskLevels.has(model.riskLevel) || model.reversibility === "L3Irreversible";
}

export function getApprovalBlockReason(
  model: ApprovalCardModel,
  state: ApprovalControllerState,
  requiredIrreversibleConfirmation: string | null = null,
): string | null {
  if (model.isExpired) {
    return "approval.error.expired";
  }
  if (model.instructionOrigin === "app_content" && !state.hasAppContentOverride) {
    return "approval.error.app_content_override_required";
  }
  if (state.selectedScope === null) {
    return "approval.error.scope_required";
  }
  if (!model.scopeOptions.includes(state.selectedScope)) {
    return "approval.error.scope_not_offered";
  }
  if (isHighRisk(model) && state.selectedScope !== "once") {
    return "approval.error.high_risk_once_only";
  }
  if (
    model.reversibility === "L3Irreversible" &&
    (requiredIrreversibleConfirmation === null ||
      state.irreversibleConfirmation !== requiredIrreversibleConfirmation)
  ) {
    return "approval.error.irreversible_confirmation_required";
  }
  return null;
}

export function createInitialApprovalControllerState(model: unknown): ApprovalControllerState {
  const parseResult = parseApprovalCardModel(model);
  return {
    parseResult,
    selectedScope:
      parseResult.ok && parseResult.model.scopeOptions.length === 1
        ? (parseResult.model.scopeOptions[0] ?? null)
        : null,
    hasAppContentOverride: false,
    irreversibleConfirmation: "",
    denialReason: "",
    errorKey: null,
    decision: null,
  };
}

export function approvalControllerReducer(
  state: ApprovalControllerState,
  action: ApprovalControllerAction,
): ApprovalControllerState {
  if (action.type === "replace_model") {
    return createInitialApprovalControllerState(action.model);
  }
  if (!state.parseResult.ok) {
    return state;
  }
  const model = state.parseResult.model;
  switch (action.type) {
    case "select_scope":
      if (!model.scopeOptions.includes(action.scope)) {
        return { ...state, errorKey: "approval.error.scope_not_offered" };
      }
      if (isHighRisk(model) && action.scope !== "once") {
        return { ...state, errorKey: "approval.error.high_risk_once_only" };
      }
      return { ...state, selectedScope: action.scope, errorKey: null };
    case "set_app_content_override":
      return { ...state, hasAppContentOverride: action.isEnabled, errorKey: null };
    case "set_irreversible_confirmation":
      return { ...state, irreversibleConfirmation: action.confirmation, errorKey: null };
    case "set_denial_reason":
      return { ...state, denialReason: action.reason, errorKey: null };
    case "approve": {
      const blockReason = getApprovalBlockReason(
        model,
        state,
        action.requiredIrreversibleConfirmation,
      );
      if (blockReason !== null || state.selectedScope === null) {
        return { ...state, errorKey: blockReason ?? "approval.error.scope_required" };
      }
      return {
        ...state,
        errorKey: null,
        decision: {
          kind: "approved",
          requestId: model.requestId,
          action: model.action,
          scope: state.selectedScope,
          instructionOrigin: model.instructionOrigin,
        },
      };
    }
    case "deny": {
      const reason = state.denialReason.trim();
      if (reason.length === 0) {
        return { ...state, errorKey: "approval.error.denial_reason_required" };
      }
      return {
        ...state,
        errorKey: null,
        decision: { kind: "denied", requestId: model.requestId, reason },
      };
    }
    case "reset_decision":
      return { ...state, decision: null, errorKey: null };
    default:
      return state;
  }
}

export function parseApprovalCardModel(input: unknown): ApprovalModelParseResult {
  if (!isRecord(input)) {
    return { ok: false, errors: ["model must be an object"] };
  }
  const errors: string[] = [];
  const requestId = readText(input, "requestId", errors);
  const action = readText(input, "action", errors);
  const targetLabel = readText(input, "targetLabel", errors);
  const summary = readText(input, "summary", errors);
  const impact = readText(input, "impact", errors);
  const instructionOrigin = readEnum(input, "instructionOrigin", instructionOrigins, errors);
  const toolSelectionReason = readText(input, "toolSelectionReason", errors);
  const riskLevel = readEnum(input, "riskLevel", approvalRiskLevels, errors);
  const reversibility = readEnum(input, "reversibility", reversibilityLevels, errors);
  const diff = parseDiff(input.diff, errors);
  const undo = parseUndoPlan(input.undo, errors);
  const scopeOptions = parseAuthorizationScopes(input.scopeOptions, errors);
  const evidence = parseEvidence(input.evidence, errors);
  const isExpired = input.isExpired;
  if (typeof isExpired !== "boolean") {
    errors.push("isExpired must be boolean");
  }
  if (
    requestId === null ||
    action === null ||
    targetLabel === null ||
    summary === null ||
    impact === null ||
    instructionOrigin === null ||
    toolSelectionReason === null ||
    riskLevel === null ||
    reversibility === null ||
    diff === null ||
    undo === null ||
    scopeOptions === null ||
    evidence === null ||
    typeof isExpired !== "boolean"
  ) {
    return { ok: false, errors };
  }
  const model: ApprovalCardModel = {
    requestId,
    action,
    targetLabel,
    summary,
    impact,
    diff,
    instructionOrigin,
    toolSelectionReason,
    riskLevel,
    reversibility,
    undo,
    scopeOptions,
    evidence,
    isExpired,
  };
  if (isHighRisk(model) && (scopeOptions.length !== 1 || scopeOptions[0] !== "once")) {
    errors.push("high risk or irreversible actions must offer only the once scope");
  }
  if (reversibility === "L3Irreversible" && diff.kind !== "irreversible") {
    errors.push("L3Irreversible actions require an irreversible diff warning");
  }
  if (reversibility === "L3Irreversible" && undo.isAvailable) {
    errors.push("L3Irreversible actions must not advertise an undo path");
  }
  return errors.length === 0 ? { ok: true, model } : { ok: false, errors };
}

function parseDiff(value: unknown, errors: string[]): ApprovalDiff | null {
  if (!isRecord(value) || typeof value.kind !== "string") {
    errors.push("diff must be a tagged object");
    return null;
  }
  if (value.kind === "none") {
    return { kind: "none" };
  }
  if (value.kind === "irreversible") {
    if (typeof value.warningKey !== "string" || value.warningKey.trim() === "") {
      errors.push("irreversible diff requires warningKey");
      return null;
    }
    return { kind: "irreversible", warningKey: value.warningKey };
  }
  if (!Array.isArray(value.entries)) {
    errors.push("diff entries must be an array");
    return null;
  }
  switch (value.kind) {
    case "text": {
      const entries = value.entries.filter(isTextDiffEntry);
      if (entries.length !== value.entries.length) {
        errors.push("text diff entries are malformed");
        return null;
      }
      return { kind: "text", entries };
    }
    case "fields": {
      const entries = value.entries.filter(isFieldDiff);
      if (entries.length !== value.entries.length) {
        errors.push("field diff entries are malformed");
        return null;
      }
      return { kind: "fields", entries };
    }
    case "files": {
      const entries = value.entries.filter(isFileDiff);
      if (entries.length !== value.entries.length) {
        errors.push("file diff entries are malformed");
        return null;
      }
      return { kind: "files", entries };
    }
    case "ui_steps": {
      const entries = value.entries.filter(isUiStepDiff);
      if (entries.length !== value.entries.length) {
        errors.push("UI step diff entries are malformed");
        return null;
      }
      return { kind: "ui_steps", entries };
    }
    default:
      errors.push(`unsupported diff kind: ${value.kind}`);
      return null;
  }
}

function parseUndoPlan(value: unknown, errors: string[]): ApprovalUndoPlan | null {
  if (!isRecord(value) || typeof value.isAvailable !== "boolean") {
    errors.push("undo must contain isAvailable");
    return null;
  }
  const methodKey = readText(value, "methodKey", errors);
  const anchorId = readNullableText(value, "anchorId", errors);
  const disabledReasonKey = readNullableText(value, "disabledReasonKey", errors);
  if (methodKey === null || anchorId === undefined || disabledReasonKey === undefined) {
    return null;
  }
  if (value.isAvailable && anchorId === null) {
    errors.push("available undo requires an anchorId");
  }
  if (!value.isAvailable && disabledReasonKey === null) {
    errors.push("disabled undo requires disabledReasonKey");
  }
  return { isAvailable: value.isAvailable, methodKey, anchorId, disabledReasonKey };
}

function parseAuthorizationScopes(value: unknown, errors: string[]): AuthorizationScope[] | null {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push("scopeOptions must be a non-empty array");
    return null;
  }
  if (!value.every((scope) => isEnumValue(scope, authorizationScopes))) {
    errors.push("scopeOptions contains an unsupported scope");
    return null;
  }
  if (new Set(value).size !== value.length) {
    errors.push("scopeOptions must not contain duplicates");
    return null;
  }
  return value;
}

function parseEvidence(value: unknown, errors: string[]): ApprovalEvidence[] | null {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push("evidence must be a non-empty array");
    return null;
  }
  const evidence: ApprovalEvidence[] = [];
  for (const entry of value) {
    if (
      !isRecord(entry) ||
      typeof entry.id !== "string" ||
      entry.id.trim() === "" ||
      !isEnumValue(entry.kind, ["tree_snapshot", "screenshot", "log"] as const) ||
      typeof entry.labelKey !== "string" ||
      entry.labelKey.trim() === "" ||
      !isSafeEvidenceHref(entry.href)
    ) {
      errors.push("evidence entry is malformed");
      return null;
    }
    evidence.push({
      id: entry.id,
      kind: entry.kind,
      labelKey: entry.labelKey,
      href: entry.href,
    });
  }
  return evidence;
}

function isTextDiffEntry(value: unknown): value is ApprovalTextDiffEntry {
  return (
    isRecord(value) &&
    isEnumValue(value.kind, ["added", "removed"] as const) &&
    isNullableNonNegativeInteger(value.beforeLine) &&
    isNullableNonNegativeInteger(value.afterLine) &&
    typeof value.text === "string"
  );
}

function isFieldDiff(value: unknown): value is ApprovalFieldDiff {
  return (
    isRecord(value) &&
    typeof value.field === "string" &&
    value.field.trim() !== "" &&
    typeof value.before === "string" &&
    typeof value.after === "string"
  );
}

function isFileDiff(value: unknown): value is ApprovalFileDiff {
  return (
    isRecord(value) &&
    typeof value.path === "string" &&
    value.path.trim() !== "" &&
    isEnumValue(value.kind, ["added", "removed", "modified"] as const) &&
    isNullableNonNegativeInteger(value.beforeBytes) &&
    isNullableNonNegativeInteger(value.afterBytes)
  );
}

function isUiStepDiff(value: unknown): value is ApprovalUiStepDiff {
  return (
    isRecord(value) &&
    typeof value.stepId === "string" &&
    value.stepId.trim() !== "" &&
    typeof value.action === "string" &&
    value.action.trim() !== "" &&
    typeof value.targetLabel === "string" &&
    value.targetLabel.trim() !== "" &&
    (value.screenshotRef === null || isSafeEvidenceHref(value.screenshotRef))
  );
}

function readText(record: Record<string, unknown>, key: string, errors: string[]): string | null {
  const value = record[key];
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(`${key} must be a non-empty string`);
    return null;
  }
  return value;
}

function readNullableText(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
): string | null | undefined {
  const value = record[key];
  if (value === null || (typeof value === "string" && value.trim() !== "")) {
    return value;
  }
  errors.push(`${key} must be a non-empty string or null`);
  return undefined;
}

function readEnum<const Values extends readonly string[]>(
  record: Record<string, unknown>,
  key: string,
  values: Values,
  errors: string[],
): Values[number] | null {
  const value = record[key];
  if (!isEnumValue(value, values)) {
    errors.push(`${key} has an unsupported value`);
    return null;
  }
  return value;
}

function isEnumValue<const Values extends readonly string[]>(
  value: unknown,
  values: Values,
): value is Values[number] {
  return typeof value === "string" && values.includes(value as Values[number]);
}

function isNullableNonNegativeInteger(value: unknown): value is number | null {
  return value === null || (typeof value === "number" && Number.isInteger(value) && value >= 0);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSafeEvidenceHref(value: unknown): value is string {
  return typeof value === "string" && value.startsWith("evidence://") && value.length > 11;
}
