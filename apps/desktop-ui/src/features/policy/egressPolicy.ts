/**
 * Egress policy view model and interaction controller.
 *
 * Host snapshots are untrusted: `parseEgressPolicy` validates every level,
 * override, and local-model prerequisite before a component can render it.
 * This module never sends data or makes a policy decision; it only prepares a
 * requested policy change for the caller to persist.
 */

export const egressLevels = ["local_only", "redacted", "full"] as const;
export type EgressLevel = (typeof egressLevels)[number];

export interface EgressPolicy {
  defaultLevel: EgressLevel;
  appOverrides: Record<string, EgressLevel>;
  hasLocalModel: boolean;
}

export type EgressPolicyParseResult =
  { ok: true; policy: EgressPolicy } | { ok: false; errors: string[] };

export interface EgressPolicyChange {
  defaultLevel: EgressLevel;
  appOverrides: Record<string, EgressLevel>;
  changedLevel: EgressLevel;
  appId: string | null;
}

export type EgressPolicyRequest =
  | { kind: "default"; level: EgressLevel }
  | { kind: "app_override"; appId: string; level: EgressLevel }
  | { kind: "clear_app_override"; appId: string };

export interface EgressPolicyControllerState {
  parseResult: EgressPolicyParseResult;
  pendingChange: EgressPolicyRequest | null;
  errorKey: string | null;
  change: EgressPolicyChange | null;
}

export type EgressPolicyControllerAction =
  | { type: "replace_policy"; policy: unknown }
  | { type: "request_change"; request: EgressPolicyRequest }
  | { type: "confirm_pending_change" }
  | { type: "cancel_pending_change" }
  | { type: "reset_change" };

const egressRank: Record<EgressLevel, number> = {
  local_only: 0,
  redacted: 1,
  full: 2,
};

export function parseEgressPolicy(input: unknown): EgressPolicyParseResult {
  if (!isRecord(input)) {
    return { ok: false, errors: ["egress policy must be an object"] };
  }
  const errors: string[] = [];
  const defaultLevel = readEgressLevel(input, "defaultLevel", errors);
  const hasLocalModel = input.hasLocalModel;
  if (typeof hasLocalModel !== "boolean") {
    errors.push("hasLocalModel must be boolean");
  }
  const appOverrides = parseAppOverrides(input.appOverrides, errors);
  if (defaultLevel === null || typeof hasLocalModel !== "boolean" || appOverrides === null) {
    return { ok: false, errors };
  }
  const policy = { defaultLevel, appOverrides, hasLocalModel };
  if (
    !hasLocalModel &&
    (defaultLevel === "local_only" || Object.values(appOverrides).includes("local_only"))
  ) {
    errors.push("local_only requires an available local model");
  }
  return errors.length === 0 ? { ok: true, policy } : { ok: false, errors };
}

export function createInitialEgressPolicyState(input: unknown): EgressPolicyControllerState {
  return {
    parseResult: parseEgressPolicy(input),
    pendingChange: null,
    errorKey: null,
    change: null,
  };
}

export function egressPolicyControllerReducer(
  state: EgressPolicyControllerState,
  action: EgressPolicyControllerAction,
): EgressPolicyControllerState {
  if (action.type === "replace_policy") {
    return createInitialEgressPolicyState(action.policy);
  }
  if (!state.parseResult.ok) {
    return state;
  }
  switch (action.type) {
    case "request_change":
      return requestPolicyChange(state, action.request);
    case "confirm_pending_change":
      return state.pendingChange === null
        ? { ...state, errorKey: "policy.error.no_pending_change" }
        : applyPolicyChange(state, state.pendingChange, true);
    case "cancel_pending_change":
      return { ...state, pendingChange: null, errorKey: null, change: null };
    case "reset_change":
      return { ...state, pendingChange: null, errorKey: null, change: null };
    default:
      return state;
  }
}

export function getEffectiveEgressLevel(
  policy: EgressPolicy,
  appId: string | null,
): EgressLevel | null {
  if (appId === null) {
    return policy.defaultLevel;
  }
  if (!isValidAppId(appId)) {
    return null;
  }
  return policy.appOverrides[appId] ?? policy.defaultLevel;
}

export function isValidAppId(appId: string): boolean {
  return /^[a-z][a-z0-9_.-]{2,127}$/.test(appId);
}

function applyPolicyChange(
  state: EgressPolicyControllerState,
  request: EgressPolicyRequest,
  isUpgradeConfirmed: boolean,
): EgressPolicyControllerState {
  if (!state.parseResult.ok) {
    return state;
  }
  const policy = state.parseResult.policy;
  const resolved = resolvePolicyRequest(policy, request);
  if (!resolved.ok) {
    return { ...state, pendingChange: null, errorKey: resolved.errorKey, change: null };
  }
  const change = resolved.change;
  if (change.changedLevel === "local_only" && !policy.hasLocalModel) {
    return {
      ...state,
      pendingChange: null,
      errorKey: "policy.error.local_model_unavailable",
      change: null,
    };
  }
  if (egressRank[change.changedLevel] > egressRank[resolved.currentLevel] && !isUpgradeConfirmed) {
    return {
      ...state,
      pendingChange: request,
      errorKey: "policy.error.upgrade_confirmation_required",
      change: null,
    };
  }
  const nextPolicy: EgressPolicy = {
    defaultLevel: change.defaultLevel,
    appOverrides: change.appOverrides,
    hasLocalModel: policy.hasLocalModel,
  };
  return {
    parseResult: { ok: true, policy: nextPolicy },
    pendingChange: null,
    errorKey: null,
    change,
  };
}

function requestPolicyChange(
  state: EgressPolicyControllerState,
  request: EgressPolicyRequest,
): EgressPolicyControllerState {
  return applyPolicyChange(state, request, false);
}

function resolvePolicyRequest(
  policy: EgressPolicy,
  request: EgressPolicyRequest,
):
  | {
      ok: true;
      currentLevel: EgressLevel;
      change: EgressPolicyChange;
    }
  | { ok: false; errorKey: string } {
  if (request.kind === "default") {
    return {
      ok: true,
      currentLevel: policy.defaultLevel,
      change: {
        defaultLevel: request.level,
        appOverrides: { ...policy.appOverrides },
        changedLevel: request.level,
        appId: null,
      },
    };
  }
  if (!isValidAppId(request.appId)) {
    return { ok: false, errorKey: "policy.error.invalid_app_id" };
  }
  const currentLevel = getEffectiveEgressLevel(policy, request.appId);
  if (currentLevel === null) {
    return { ok: false, errorKey: "policy.error.invalid_app_id" };
  }
  if (request.kind === "app_override") {
    return {
      ok: true,
      currentLevel,
      change: {
        defaultLevel: policy.defaultLevel,
        appOverrides: { ...policy.appOverrides, [request.appId]: request.level },
        changedLevel: request.level,
        appId: request.appId,
      },
    };
  }
  const nextOverrides = { ...policy.appOverrides };
  delete nextOverrides[request.appId];
  return {
    ok: true,
    currentLevel,
    change: {
      defaultLevel: policy.defaultLevel,
      appOverrides: nextOverrides,
      changedLevel: policy.defaultLevel,
      appId: request.appId,
    },
  };
}

function parseAppOverrides(value: unknown, errors: string[]): Record<string, EgressLevel> | null {
  if (!isRecord(value)) {
    errors.push("appOverrides must be an object");
    return null;
  }
  const overrides: Record<string, EgressLevel> = {};
  for (const [appId, rawLevel] of Object.entries(value)) {
    if (!isValidAppId(appId)) {
      errors.push(`invalid app id: ${appId}`);
      continue;
    }
    if (!isEgressLevel(rawLevel)) {
      errors.push(`unsupported egress level for ${appId}`);
      continue;
    }
    overrides[appId] = rawLevel;
  }
  return overrides;
}

function readEgressLevel(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
): EgressLevel | null {
  const value = record[key];
  if (!isEgressLevel(value)) {
    errors.push(`${key} has an unsupported egress level`);
    return null;
  }
  return value;
}

function isEgressLevel(value: unknown): value is EgressLevel {
  return typeof value === "string" && egressLevels.includes(value as EgressLevel);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
