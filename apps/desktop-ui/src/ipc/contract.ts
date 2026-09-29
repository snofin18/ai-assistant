/**
 * UI ↔ Core IPC contract, version 1.0.
 *
 * Both directions are runtime-validated with zod: the UI never trusts Core's
 * payload and Core never trusts the UI's. The shapes mirror
 * `apps/agent-core/src/ui_ipc.rs`; the golden fixtures under
 * `apps/agent-core/tests/fixtures/ui_ipc/` are parsed by both sides' tests, so
 * the two implementations cannot drift apart silently.
 *
 * Unknown fields are rejected (`z.strictObject`), matching the Core-side
 * allow-list: a field nobody agreed on is a defect, not something to ignore.
 */

import { z } from "zod";

export const UI_IPC_VERSION = "1.0" as const;

export const uiAuthorizationScopes = [
  "once",
  "this_step_pattern",
  "this_task",
  "this_app_session",
  "persistent",
] as const;

const nonEmptyText = z
  .string()
  .refine((value) => value.trim().length > 0, "must be a non-empty string");

export const uiAuthorizationScopeSchema = z.enum(uiAuthorizationScopes);

export const uiCommandSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("submit_intent"),
    intent_id: nonEmptyText,
    goal: nonEmptyText,
  }),
  z.strictObject({
    kind: z.literal("approve_request"),
    request_id: nonEmptyText,
    scope: uiAuthorizationScopeSchema,
  }),
  z.strictObject({
    kind: z.literal("deny_request"),
    request_id: nonEmptyText,
    reason: nonEmptyText,
  }),
  z.strictObject({ kind: z.literal("pause_task"), task_id: nonEmptyText }),
  z.strictObject({ kind: z.literal("cancel_task"), task_id: nonEmptyText }),
  z.strictObject({ kind: z.literal("take_over_task"), task_id: nonEmptyText }),
]);

export const uiCommandEnvelopeSchema = z.strictObject({
  version: z.literal(UI_IPC_VERSION),
  command: uiCommandSchema,
});

export const uiEventSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("task_state_changed"),
    task_id: nonEmptyText,
    status: nonEmptyText,
    hold_reason: z.string().nullable(),
  }),
  z.strictObject({
    kind: z.literal("step_state_changed"),
    task_id: nonEmptyText,
    step_id: nonEmptyText,
    status: nonEmptyText,
    phase: z.string().nullable(),
    post_fingerprint: z.string().nullable(),
  }),
]);

export const uiCommandOutcomeSchema = z.discriminatedUnion("status", [
  z.strictObject({
    status: z.literal("intent_accepted"),
    intent_id: nonEmptyText,
  }),
  z.strictObject({
    status: z.literal("approval_granted"),
    request_id: nonEmptyText,
    scope: uiAuthorizationScopeSchema,
  }),
  z.strictObject({
    status: z.literal("approval_denied"),
    request_id: nonEmptyText,
    reason: nonEmptyText,
    task_status: nonEmptyText,
  }),
  z.strictObject({
    status: z.literal("task_paused"),
    task_id: nonEmptyText,
    task_status: nonEmptyText,
  }),
  z.strictObject({
    status: z.literal("task_cancelled"),
    task_id: nonEmptyText,
    task_status: nonEmptyText,
  }),
  z.strictObject({
    status: z.literal("task_taken_over"),
    task_id: nonEmptyText,
    task_status: nonEmptyText,
  }),
]);

export type UiAuthorizationScope = z.infer<typeof uiAuthorizationScopeSchema>;
export type UiCommand = z.infer<typeof uiCommandSchema>;
export type UiCommandEnvelope = z.infer<typeof uiCommandEnvelopeSchema>;
export type UiEvent = z.infer<typeof uiEventSchema>;
export type UiCommandOutcome = z.infer<typeof uiCommandOutcomeSchema>;

export type ContractParseResult<T> = { ok: true; value: T } | { ok: false; errors: string[] };

export function parseUiCommandEnvelope(input: unknown): ContractParseResult<UiCommandEnvelope> {
  return parseWith(uiCommandEnvelopeSchema, input);
}

export function parseUiEvent(input: unknown): ContractParseResult<UiEvent> {
  return parseWith(uiEventSchema, input);
}

export function parseUiCommandOutcome(input: unknown): ContractParseResult<UiCommandOutcome> {
  return parseWith(uiCommandOutcomeSchema, input);
}

function parseWith<Schema extends z.ZodType>(
  schema: Schema,
  input: unknown,
): ContractParseResult<z.infer<Schema>> {
  const result = schema.safeParse(input);
  if (result.success) {
    return { ok: true, value: result.data };
  }
  return {
    ok: false,
    errors: result.error.issues.map((issue) => {
      const path = issue.path.join(".");
      return path.length === 0 ? issue.message : `${path}: ${issue.message}`;
    }),
  };
}
