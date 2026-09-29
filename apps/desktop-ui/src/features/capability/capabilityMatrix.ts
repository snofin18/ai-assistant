/**
 * Capability Matrix view model.
 *
 * The snapshot crosses a strict runtime parser before rendering. The parser
 * mirrors the runtime matrix invariants from `assistant-platform-api`; it does
 * not probe capabilities, execute platform calls, or make policy decisions.
 */

export const capabilityRiskLevels = ["l1", "l2", "l3", "l4", "l5"] as const;
export type CapabilityRiskLevel = (typeof capabilityRiskLevels)[number];

export const capabilityApprovals = ["auto", "required", "forbidden"] as const;
export type CapabilityApproval = (typeof capabilityApprovals)[number];

export const capabilityResources = ["read", "write", "send", "invoke", "destroy"] as const;
export type CapabilityResource = (typeof capabilityResources)[number];

export const capabilitySideEffects = [
  "none",
  "ui_focus",
  "disk",
  "process_spawn",
  "network_egress",
  "clipboard",
  "external_system",
] as const;
export type CapabilitySideEffect = (typeof capabilitySideEffects)[number];

export const channelAvailabilities = ["available", "unavailable", "needs_consent"] as const;
export type ChannelAvailability = (typeof channelAvailabilities)[number];

const supportedPlatformOs = ["windows", "macos", "linux"] as const;

export interface CapabilityProbe {
  probedAt: string;
  platformOs: string;
  isSessionLocked: boolean;
  isSessionRemote: boolean;
  isSessionHeadless: boolean;
}

export interface CapabilityEntryView {
  id: string;
  resource: CapabilityResource;
  sideEffect: CapabilitySideEffect;
  risk: CapabilityRiskLevel;
  approval: CapabilityApproval;
}

export interface CapabilityChannelView {
  name: string;
  availability: ChannelAvailability;
  detail: string | null;
}

export interface CapabilityDegradationView {
  id: string;
  impact: string;
}

export interface CapabilityMatrixView {
  probe: CapabilityProbe;
  channels: CapabilityChannelView[];
  capabilities: CapabilityEntryView[];
  degradations: CapabilityDegradationView[];
}

export type CapabilityMatrixParseResult =
  { ok: true; matrix: CapabilityMatrixView } | { ok: false; errors: string[] };

export interface CapabilityStats {
  total: number;
  requiresApproval: number;
  forbidden: number;
  degradedChannels: number;
}

export function parseCapabilityMatrix(input: unknown): CapabilityMatrixParseResult {
  if (!isRecord(input)) {
    return { ok: false, errors: ["capability matrix must be an object"] };
  }
  const errors: string[] = [];
  const probe = parseProbe(input.probe, errors);
  const channels = parseChannels(input.channels, errors);
  const capabilities = parseCapabilities(input.capabilities, errors);
  const degradations = parseDegradations(input.degradations, errors);
  if (
    probe === null ||
    channels === null ||
    capabilities === null ||
    degradations === null ||
    errors.length > 0
  ) {
    return { ok: false, errors };
  }
  return { ok: true, matrix: { probe, channels, capabilities, degradations } };
}

export function getCapabilityStats(matrix: CapabilityMatrixView): CapabilityStats {
  return {
    total: matrix.capabilities.length,
    requiresApproval: matrix.capabilities.filter((capability) => capability.approval === "required")
      .length,
    forbidden: matrix.capabilities.filter((capability) => capability.approval === "forbidden")
      .length,
    degradedChannels: matrix.channels.filter((channel) => channel.availability !== "available")
      .length,
  };
}

function parseProbe(value: unknown, errors: string[]): CapabilityProbe | null {
  if (!isRecord(value)) {
    errors.push("probe must be an object");
    return null;
  }
  const probedAt = readUtcTimestamp(value, "probed_at", errors);
  const platformOs = readPlatformOs(value, errors);
  const isSessionLocked = readBoolean(value, "session_locked", errors);
  const isSessionRemote = readBoolean(value, "session_remote", errors);
  const isSessionHeadless = readBoolean(value, "session_headless", errors);
  if (
    probedAt === null ||
    platformOs === null ||
    isSessionLocked === null ||
    isSessionRemote === null ||
    isSessionHeadless === null
  ) {
    return null;
  }
  return { probedAt, platformOs, isSessionLocked, isSessionRemote, isSessionHeadless };
}

function parseChannels(value: unknown, errors: string[]): CapabilityChannelView[] | null {
  if (!isRecord(value)) {
    errors.push("channels must be an object");
    return null;
  }
  const channels: CapabilityChannelView[] = [];
  for (const [name, rawChannel] of Object.entries(value)) {
    if (name.trim() === "" || !isRecord(rawChannel)) {
      errors.push("channel entries must have a non-empty name and object value");
      continue;
    }
    const availability = rawChannel.availability;
    if (!isEnumValue(availability, channelAvailabilities)) {
      errors.push(`channel ${name} has an unsupported availability`);
      continue;
    }
    const detail = rawChannel.detail;
    if (detail !== undefined && detail !== null && typeof detail !== "string") {
      errors.push(`channel ${name} detail must be a string or null`);
      continue;
    }
    channels.push({ name, availability, detail: detail ?? null });
  }
  return channels;
}

function parseCapabilities(value: unknown, errors: string[]): CapabilityEntryView[] | null {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push("capabilities must be a non-empty array");
    return null;
  }
  const capabilities: CapabilityEntryView[] = [];
  const ids = new Set<string>();
  for (const [index, rawCapability] of value.entries()) {
    const capability = parseCapability(rawCapability, index, errors);
    if (capability === null) {
      continue;
    }
    if (ids.has(capability.id)) {
      errors.push(`duplicate capability id: ${capability.id}`);
      continue;
    }
    ids.add(capability.id);
    capabilities.push(capability);
  }
  return capabilities.length === value.length ? capabilities : null;
}

function parseCapability(
  value: unknown,
  index: number,
  errors: string[],
): CapabilityEntryView | null {
  if (!isRecord(value)) {
    errors.push(`capabilities[${index}] must be an object`);
    return null;
  }
  const id = readText(value, "id", errors, `capabilities[${index}]`);
  const resource = value.resource;
  const sideEffect = value.side_effect;
  const risk = value.risk;
  const approval = value.approval;
  if (
    id === null ||
    !isValidCapabilityId(id) ||
    !isEnumValue(resource, capabilityResources) ||
    !isEnumValue(sideEffect, capabilitySideEffects) ||
    !isEnumValue(risk, capabilityRiskLevels) ||
    !isEnumValue(approval, capabilityApprovals)
  ) {
    errors.push(`capabilities[${index}] has invalid fields`);
    return null;
  }
  if (!isApprovalConsistent(risk, approval)) {
    errors.push(`capabilities[${index}] approval is inconsistent with risk`);
    return null;
  }
  return { id, resource, sideEffect, risk, approval };
}

function parseDegradations(value: unknown, errors: string[]): CapabilityDegradationView[] | null {
  if (!Array.isArray(value)) {
    errors.push("degradations must be an array");
    return null;
  }
  const degradations: CapabilityDegradationView[] = [];
  const ids = new Set<string>();
  for (const [index, rawDegradation] of value.entries()) {
    if (!isRecord(rawDegradation)) {
      errors.push(`degradations[${index}] must be an object`);
      continue;
    }
    const id = readText(rawDegradation, "id", errors, `degradations[${index}]`);
    const impact = readText(rawDegradation, "impact", errors, `degradations[${index}]`);
    if (id === null || impact === null) {
      continue;
    }
    if (ids.has(id)) {
      errors.push(`duplicate degradation id: ${id}`);
      continue;
    }
    ids.add(id);
    degradations.push({ id, impact });
  }
  return degradations.length === value.length ? degradations : null;
}

function isApprovalConsistent(risk: CapabilityRiskLevel, approval: CapabilityApproval): boolean {
  if (risk === "l3" || risk === "l4") {
    return approval === "required";
  }
  if (risk === "l5") {
    return approval === "forbidden";
  }
  return approval !== "forbidden";
}

function isValidCapabilityId(value: string): boolean {
  return /^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$/.test(value);
}

function readText(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
  prefix = "probe",
): string | null {
  const value = record[key];
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(`${prefix}.${key} must be a non-empty string`);
    return null;
  }
  return value;
}

function readUtcTimestamp(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
): string | null {
  const value = record[key];
  if (typeof value !== "string" || parseUtcTimestamp(value) === null) {
    errors.push(`probe.${key} must be a UTC RFC 3339 timestamp`);
    return null;
  }
  return value;
}

function readPlatformOs(record: Record<string, unknown>, errors: string[]): string | null {
  const value = record.platform_os;
  if (!isEnumValue(value, supportedPlatformOs)) {
    errors.push("probe.platform_os must be windows, macos, or linux");
    return null;
  }
  return value;
}

function readBoolean(
  record: Record<string, unknown>,
  key: string,
  errors: string[],
): boolean | null {
  const value = record[key];
  if (typeof value !== "boolean") {
    errors.push(`probe.${key} must be boolean`);
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseUtcTimestamp(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?Z$/.exec(value);
  if (match === null) {
    return null;
  }
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const hour = Number(match[4]);
  const minute = Number(match[5]);
  const second = Number(match[6]);
  if (
    year < 1000 ||
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > 31 ||
    hour > 23 ||
    minute > 59 ||
    second > 60
  ) {
    return null;
  }
  const timestamp = new Date(Date.UTC(year, month - 1, day, hour, minute, second));
  if (
    timestamp.getUTCFullYear() !== year ||
    timestamp.getUTCMonth() !== month - 1 ||
    timestamp.getUTCDate() !== day ||
    timestamp.getUTCHours() !== hour ||
    timestamp.getUTCMinutes() !== minute ||
    timestamp.getUTCSeconds() !== second
  ) {
    return null;
  }
  return timestamp;
}
