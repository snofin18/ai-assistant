/**
 * Execution timeline view model and interaction controller.
 *
 * The model is runtime-validated before rendering. Undo and replay availability
 * are derived from explicit evidence and anchors; missing data never becomes a
 * false success state.
 */

export const timelineStepStatuses = ["succeeded", "failed", "awaiting_approval", "undone"] as const;
export type TimelineStepStatus = (typeof timelineStepStatuses)[number];

export const verificationStatuses = ["passed", "failed", "not_applicable"] as const;
export type VerificationStatus = (typeof verificationStatuses)[number];

export const timelineReversibilityLevels = [
  "L0UndoStack",
  "L1ContentSnapshot",
  "L2ShadowCopy",
  "L3Irreversible",
] as const;
export type TimelineReversibilityLevel = (typeof timelineReversibilityLevels)[number];

export interface TimelineEvidence {
  id: string;
  kind: "tree_snapshot" | "screenshot" | "log";
  labelKey: string;
  href: string;
}

export interface TimelineUndoPlan {
  isAvailable: boolean;
  methodKey: string;
  anchorId: string | null;
  disabledReasonKey: string | null;
}

export interface TimelineCost {
  tokensIn: number;
  tokensOut: number;
  usd: number;
  latencyMs: number;
}

export interface TimelineStepModel {
  stepId: string;
  action: string;
  targetLabel: string;
  status: TimelineStepStatus;
  preFingerprint: string | null;
  postFingerprint: string | null;
  verification: VerificationStatus;
  verificationReasonKey: string | null;
  evidence: TimelineEvidence[];
  durationMs: number;
  cost: TimelineCost;
  reversibility: TimelineReversibilityLevel;
  undo: TimelineUndoPlan;
  canReplay: boolean;
  replayDisabledReasonKey: string | null;
}

export type TimelineModelParseResult =
  { ok: true; steps: TimelineStepModel[] } | { ok: false; errors: string[] };

export interface TimelineControllerState {
  parseResult: TimelineModelParseResult;
  selectedStepId: string | null;
  errorKey: string | null;
}

export type TimelineControllerAction =
  | { type: "replace_model"; model: unknown }
  | { type: "select_step"; stepId: string }
  | { type: "request_undo"; stepId: string }
  | { type: "request_replay"; stepId: string }
  | { type: "reset_operation" };

export interface Availability {
  isAvailable: boolean;
  reasonKey: string | null;
}

export function getUndoAvailability(step: TimelineStepModel): Availability {
  if (step.reversibility === "L3Irreversible") {
    return { isAvailable: false, reasonKey: "timeline.undo.irreversible" };
  }
  if (!step.undo.isAvailable) {
    return {
      isAvailable: false,
      reasonKey: step.undo.disabledReasonKey ?? "timeline.undo.unavailable",
    };
  }
  if (step.undo.anchorId === null) {
    return { isAvailable: false, reasonKey: "timeline.undo.missing_anchor" };
  }
  if (step.status !== "succeeded") {
    return { isAvailable: false, reasonKey: "timeline.undo.step_not_succeeded" };
  }
  return { isAvailable: true, reasonKey: null };
}

export function getReplayAvailability(step: TimelineStepModel): Availability {
  if (!step.canReplay) {
    return {
      isAvailable: false,
      reasonKey: step.replayDisabledReasonKey ?? "timeline.replay.unavailable",
    };
  }
  if (!step.evidence.some((entry) => entry.kind === "tree_snapshot")) {
    return { isAvailable: false, reasonKey: "timeline.replay.missing_snapshot" };
  }
  return { isAvailable: true, reasonKey: null };
}

export function createInitialTimelineControllerState(model: unknown): TimelineControllerState {
  return {
    parseResult: parseTimelineModel(model),
    selectedStepId: null,
    errorKey: null,
  };
}

export function timelineControllerReducer(
  state: TimelineControllerState,
  action: TimelineControllerAction,
): TimelineControllerState {
  if (action.type === "replace_model") {
    return createInitialTimelineControllerState(action.model);
  }
  if (!state.parseResult.ok) {
    return state;
  }
  if (action.type === "reset_operation") {
    return { ...state, errorKey: null };
  }
  const step = state.parseResult.steps.find((candidate) => candidate.stepId === action.stepId);
  if (step === undefined) {
    return { ...state, errorKey: "timeline.error.step_not_found" };
  }
  if (action.type === "select_step") {
    return { ...state, selectedStepId: step.stepId, errorKey: null };
  }
  if (action.type === "request_undo") {
    const availability = getUndoAvailability(step);
    if (!availability.isAvailable) {
      return { ...state, errorKey: availability.reasonKey };
    }
    return {
      ...state,
      errorKey: null,
    };
  }
  const availability = getReplayAvailability(step);
  if (!availability.isAvailable) {
    return { ...state, errorKey: availability.reasonKey };
  }
  return {
    ...state,
    errorKey: null,
  };
}

export function parseTimelineModel(input: unknown): TimelineModelParseResult {
  if (!Array.isArray(input)) {
    return { ok: false, errors: ["timeline model must be an array"] };
  }
  if (input.length === 0) {
    return { ok: false, errors: ["timeline model must contain at least one step"] };
  }
  const errors: string[] = [];
  const steps: TimelineStepModel[] = [];
  const stepIds = new Set<string>();
  for (const [index, value] of input.entries()) {
    const step = parseTimelineStep(value, index, errors);
    if (step !== null) {
      if (stepIds.has(step.stepId)) {
        errors.push(`step ${index}: duplicate stepId`);
      } else {
        stepIds.add(step.stepId);
        steps.push(step);
      }
    }
  }
  return errors.length === 0 ? { ok: true, steps } : { ok: false, errors };
}

function parseTimelineStep(
  value: unknown,
  index: number,
  errors: string[],
): TimelineStepModel | null {
  if (!isRecord(value)) {
    errors.push(`step ${index}: must be an object`);
    return null;
  }
  const prefix = `step ${index}`;
  const stepId = readText(value, "stepId", prefix, errors);
  const action = readText(value, "action", prefix, errors);
  const targetLabel = readText(value, "targetLabel", prefix, errors);
  const status = readEnum(value, "status", timelineStepStatuses, prefix, errors);
  const verification = readEnum(value, "verification", verificationStatuses, prefix, errors);
  const reversibility = readEnum(
    value,
    "reversibility",
    timelineReversibilityLevels,
    prefix,
    errors,
  );
  const preFingerprint = readNullableText(value, "preFingerprint", prefix, errors);
  const postFingerprint = readNullableText(value, "postFingerprint", prefix, errors);
  const verificationReasonKey = readNullableText(value, "verificationReasonKey", prefix, errors);
  const replayDisabledReasonKey = readNullableText(
    value,
    "replayDisabledReasonKey",
    prefix,
    errors,
  );
  const evidence = parseEvidence(value.evidence, prefix, errors);
  const undo = parseUndoPlan(value.undo, prefix, errors);
  const cost = parseCost(value.cost, prefix, errors);
  const durationMs = value.durationMs;
  const canReplay = value.canReplay;
  if (!isNonNegativeInteger(durationMs)) {
    errors.push(`${prefix}: durationMs must be a non-negative integer`);
  }
  if (typeof canReplay !== "boolean") {
    errors.push(`${prefix}: canReplay must be boolean`);
  }
  if (
    stepId === null ||
    action === null ||
    targetLabel === null ||
    status === null ||
    verification === null ||
    reversibility === null ||
    preFingerprint === undefined ||
    postFingerprint === undefined ||
    verificationReasonKey === undefined ||
    replayDisabledReasonKey === undefined ||
    evidence === null ||
    undo === null ||
    cost === null ||
    !isNonNegativeInteger(durationMs) ||
    typeof canReplay !== "boolean"
  ) {
    return null;
  }
  const step: TimelineStepModel = {
    stepId,
    action,
    targetLabel,
    status,
    preFingerprint,
    postFingerprint,
    verification,
    verificationReasonKey,
    evidence,
    durationMs,
    cost,
    reversibility,
    undo,
    canReplay,
    replayDisabledReasonKey,
  };
  if (verification === "failed" && verificationReasonKey === null) {
    errors.push(`${prefix}: failed verification requires verificationReasonKey`);
  }
  if (status === "succeeded" && verification === "failed") {
    errors.push(`${prefix}: succeeded step cannot have failed verification`);
  }
  if (canReplay && replayDisabledReasonKey !== null) {
    errors.push(`${prefix}: replay reason must be null when replay is available`);
  }
  if (!canReplay && replayDisabledReasonKey === null) {
    errors.push(`${prefix}: disabled replay requires replayDisabledReasonKey`);
  }
  if (reversibility === "L3Irreversible" && undo.isAvailable) {
    errors.push(`${prefix}: irreversible steps must not advertise undo`);
  }
  return step;
}

function parseEvidence(
  value: unknown,
  prefix: string,
  errors: string[],
): TimelineEvidence[] | null {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push(`${prefix}: evidence must be a non-empty array`);
    return null;
  }
  const evidence: TimelineEvidence[] = [];
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
      errors.push(`${prefix}: evidence entry is malformed`);
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

function parseUndoPlan(value: unknown, prefix: string, errors: string[]): TimelineUndoPlan | null {
  if (!isRecord(value) || typeof value.isAvailable !== "boolean") {
    errors.push(`${prefix}: undo must contain isAvailable`);
    return null;
  }
  const methodKey = readText(value, "methodKey", prefix, errors);
  const anchorId = readNullableText(value, "anchorId", prefix, errors);
  const disabledReasonKey = readNullableText(value, "disabledReasonKey", prefix, errors);
  if (methodKey === null || anchorId === undefined || disabledReasonKey === undefined) {
    return null;
  }
  if (value.isAvailable && anchorId === null) {
    errors.push(`${prefix}: available undo requires an anchorId`);
  }
  if (!value.isAvailable && disabledReasonKey === null) {
    errors.push(`${prefix}: disabled undo requires disabledReasonKey`);
  }
  return { isAvailable: value.isAvailable, methodKey, anchorId, disabledReasonKey };
}

function parseCost(value: unknown, prefix: string, errors: string[]): TimelineCost | null {
  if (!isRecord(value)) {
    errors.push(`${prefix}: cost must be an object`);
    return null;
  }
  const tokensIn = value.tokensIn;
  const tokensOut = value.tokensOut;
  const usd = value.usd;
  const latencyMs = value.latencyMs;
  if (
    !isNonNegativeInteger(tokensIn) ||
    !isNonNegativeInteger(tokensOut) ||
    typeof usd !== "number" ||
    !Number.isFinite(usd) ||
    usd < 0 ||
    !isNonNegativeInteger(latencyMs)
  ) {
    errors.push(`${prefix}: cost fields must be finite non-negative numbers`);
    return null;
  }
  return { tokensIn, tokensOut, usd, latencyMs };
}

function readText(
  record: Record<string, unknown>,
  key: string,
  prefix: string,
  errors: string[],
): string | null {
  const value = record[key];
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(`${prefix}: ${key} must be a non-empty string`);
    return null;
  }
  return value;
}

function readNullableText(
  record: Record<string, unknown>,
  key: string,
  prefix: string,
  errors: string[],
): string | null | undefined {
  const value = record[key];
  if (value === null || (typeof value === "string" && value.trim() !== "")) {
    return value;
  }
  errors.push(`${prefix}: ${key} must be a non-empty string or null`);
  return undefined;
}

function readEnum<const Values extends readonly string[]>(
  record: Record<string, unknown>,
  key: string,
  values: Values,
  prefix: string,
  errors: string[],
): Values[number] | null {
  const value = record[key];
  if (!isEnumValue(value, values)) {
    errors.push(`${prefix}: ${key} has an unsupported value`);
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

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSafeEvidenceHref(value: unknown): value is string {
  return typeof value === "string" && value.startsWith("evidence://") && value.length > 11;
}
