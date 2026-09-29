/**
 * Cost panel view model and aggregation.
 *
 * Cost snapshots are untrusted. The parser rejects non-finite or unsafe
 * integers before aggregation so the UI never displays a silently rounded
 * cost. Aggregation is pure and uses integer micro-USD from the model gateway.
 */

export const costPeriods = ["task", "today", "month"] as const;
export type CostPeriod = (typeof costPeriods)[number];

export interface CostRecord {
  recordId: string;
  taskId: string;
  occurredAt: string;
  modelId: string;
  appId: string;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  costMicroUsd: number;
  latencyMs: number;
}

export interface CostPanelModel {
  currentTaskId: string;
  records: CostRecord[];
}

export type CostPanelParseResult =
  { ok: true; model: CostPanelModel } | { ok: false; errors: string[] };

export interface CostTotals {
  callCount: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  costMicroUsd: number;
  latencyMs: number;
}

export interface CostBreakdown extends CostTotals {
  key: string;
}

export interface CostSummary {
  period: CostPeriod;
  totals: CostTotals;
  byModel: CostBreakdown[];
  byApp: CostBreakdown[];
}

export type CostSummaryResult =
  { ok: true; summary: CostSummary } | { ok: false; errors: string[] };

export function parseCostPanelModel(input: unknown): CostPanelParseResult {
  if (!isRecord(input)) {
    return { ok: false, errors: ["cost panel model must be an object"] };
  }
  const errors: string[] = [];
  const currentTaskId = readIdentifier(input, "currentTaskId", "model", errors);
  const records = parseRecords(input.records, errors);
  if (currentTaskId === null || records === null) {
    return { ok: false, errors };
  }
  return { ok: true, model: { currentTaskId, records } };
}

export function summarizeCosts(
  model: CostPanelModel,
  period: CostPeriod,
  nowIso: string,
): CostSummaryResult {
  const now = parseTimestamp(nowIso);
  if (now === null) {
    return { ok: false, errors: ["nowIso must be a UTC RFC 3339 timestamp"] };
  }
  const records = model.records.filter((record) =>
    isRecordInPeriod(record, model.currentTaskId, period, now),
  );
  const totals = aggregateRecords(records, "all");
  const byModel = aggregateByKey(records, (record) => record.modelId);
  const byApp = aggregateByKey(records, (record) => record.appId);
  if (!totals.ok) {
    return { ok: false, errors: totals.errors };
  }
  if (!byModel.ok) {
    return { ok: false, errors: byModel.errors };
  }
  if (!byApp.ok) {
    return { ok: false, errors: byApp.errors };
  }
  return {
    ok: true,
    summary: {
      period,
      totals: totals.totals,
      byModel: byModel.breakdowns,
      byApp: byApp.breakdowns,
    },
  };
}

function parseRecords(value: unknown, errors: string[]): CostRecord[] | null {
  if (!Array.isArray(value)) {
    errors.push("records must be an array");
    return null;
  }
  const records: CostRecord[] = [];
  const recordIds = new Set<string>();
  for (const [index, rawRecord] of value.entries()) {
    const record = parseRecord(rawRecord, index, errors);
    if (record === null) {
      continue;
    }
    if (recordIds.has(record.recordId)) {
      errors.push(`duplicate cost record id: ${record.recordId}`);
      continue;
    }
    recordIds.add(record.recordId);
    records.push(record);
  }
  return records.length === value.length ? records : null;
}

function parseRecord(value: unknown, index: number, errors: string[]): CostRecord | null {
  if (!isRecord(value)) {
    errors.push(`records[${index}] must be an object`);
    return null;
  }
  const prefix = `records[${index}]`;
  const recordId = readIdentifier(value, "recordId", prefix, errors);
  const taskId = readIdentifier(value, "taskId", prefix, errors);
  const modelId = readIdentifier(value, "modelId", prefix, errors);
  const appId = readIdentifier(value, "appId", prefix, errors);
  const occurredAt = readTimestamp(value, "occurredAt", prefix, errors);
  const inputTokens = readNonNegativeInteger(value, "inputTokens", prefix, errors);
  const cachedInputTokens = readNonNegativeInteger(value, "cachedInputTokens", prefix, errors);
  const outputTokens = readNonNegativeInteger(value, "outputTokens", prefix, errors);
  const costMicroUsd = readNonNegativeInteger(value, "costMicroUsd", prefix, errors);
  const latencyMs = readNonNegativeInteger(value, "latencyMs", prefix, errors);
  if (
    recordId === null ||
    taskId === null ||
    modelId === null ||
    appId === null ||
    occurredAt === null ||
    inputTokens === null ||
    cachedInputTokens === null ||
    outputTokens === null ||
    costMicroUsd === null ||
    latencyMs === null
  ) {
    return null;
  }
  if (cachedInputTokens > inputTokens) {
    errors.push(`${prefix}.cachedInputTokens exceeds inputTokens`);
    return null;
  }
  return {
    recordId,
    taskId,
    occurredAt,
    modelId,
    appId,
    inputTokens,
    cachedInputTokens,
    outputTokens,
    costMicroUsd,
    latencyMs,
  };
}

function isRecordInPeriod(
  record: CostRecord,
  currentTaskId: string,
  period: CostPeriod,
  now: Date,
): boolean {
  if (period === "task") {
    return record.taskId === currentTaskId;
  }
  const occurredAt = new Date(record.occurredAt);
  if (period === "today") {
    return (
      occurredAt.getUTCFullYear() === now.getUTCFullYear() &&
      occurredAt.getUTCMonth() === now.getUTCMonth() &&
      occurredAt.getUTCDate() === now.getUTCDate()
    );
  }
  return (
    occurredAt.getUTCFullYear() === now.getUTCFullYear() &&
    occurredAt.getUTCMonth() === now.getUTCMonth()
  );
}

function aggregateByKey(
  records: readonly CostRecord[],
  getKey: (record: CostRecord) => string,
): { ok: true; breakdowns: CostBreakdown[] } | { ok: false; errors: string[] } {
  const keys = [...new Set(records.map(getKey))].sort();
  const breakdowns: CostBreakdown[] = [];
  for (const key of keys) {
    const totals = aggregateRecords(
      records.filter((record) => getKey(record) === key),
      key,
    );
    if (!totals.ok) {
      return totals;
    }
    breakdowns.push({ key, ...totals.totals });
  }
  return { ok: true, breakdowns };
}

function aggregateRecords(
  records: readonly CostRecord[],
  key: string,
): { ok: true; totals: CostTotals } | { ok: false; errors: string[] } {
  let callCount = 0;
  let inputTokens = 0;
  let cachedInputTokens = 0;
  let outputTokens = 0;
  let costMicroUsd = 0;
  let latencyMs = 0;
  for (const record of records) {
    const fields = [
      [callCount, 1],
      [inputTokens, record.inputTokens],
      [cachedInputTokens, record.cachedInputTokens],
      [outputTokens, record.outputTokens],
      [costMicroUsd, record.costMicroUsd],
      [latencyMs, record.latencyMs],
    ] as const;
    for (const [current, increment] of fields) {
      if (!Number.isSafeInteger(current + increment)) {
        return { ok: false, errors: [`cost aggregate overflow for ${key}`] };
      }
    }
    callCount += 1;
    inputTokens += record.inputTokens;
    cachedInputTokens += record.cachedInputTokens;
    outputTokens += record.outputTokens;
    costMicroUsd += record.costMicroUsd;
    latencyMs += record.latencyMs;
  }
  return {
    ok: true,
    totals: {
      callCount,
      inputTokens,
      cachedInputTokens,
      outputTokens,
      costMicroUsd,
      latencyMs,
    },
  };
}

function readIdentifier(
  record: Record<string, unknown>,
  key: string,
  prefix: string,
  errors: string[],
): string | null {
  const value = record[key];
  if (
    typeof value !== "string" ||
    value.trim() === "" ||
    value.length > 160 ||
    value.includes("\0")
  ) {
    errors.push(`${prefix}.${key} must be a non-empty identifier`);
    return null;
  }
  return value;
}

function readTimestamp(
  record: Record<string, unknown>,
  key: string,
  prefix: string,
  errors: string[],
): string | null {
  const value = record[key];
  if (typeof value !== "string" || parseUtcTimestamp(value) === null) {
    errors.push(`${prefix}.${key} must be a UTC RFC 3339 timestamp`);
    return null;
  }
  return value;
}

function readNonNegativeInteger(
  record: Record<string, unknown>,
  key: string,
  prefix: string,
  errors: string[],
): number | null {
  const value = record[key];
  if (!Number.isSafeInteger(value) || typeof value !== "number" || value < 0) {
    errors.push(`${prefix}.${key} must be a safe non-negative integer`);
    return null;
  }
  return value;
}

function parseTimestamp(value: string): Date | null {
  return parseUtcTimestamp(value);
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
