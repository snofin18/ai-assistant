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

const httpUrlText = nonEmptyText.refine(isAbsoluteHttpUrl, "must be an absolute HTTP(S) URL");

const canonicalHttpOriginText = nonEmptyText.refine(
  isCanonicalHttpOrigin,
  "must be an absolute HTTP(S) origin without credentials, path, query, or fragment",
);

const approvalFieldDiffSchema = z
  .strictObject({
    field: nonEmptyText.max(4 * 1024),
    before: z.string().max(16 * 1024),
    after: z.string().max(16 * 1024),
  })
  .refine((entry) => entry.before !== entry.after, "field diff must contain a change");

const approvalDiffSchema = z.strictObject({
  kind: z.literal("fields"),
  entries: z.array(approvalFieldDiffSchema).min(1).max(100),
});

const instructionAttributionSchema = z
  .strictObject({
    origin: z.enum(["user_request", "plan_derived", "app_content", "tool_suggestion"]),
    parent_goal: nonEmptyText.max(4 * 1024).nullable(),
    source_ref: nonEmptyText.max(1024).nullable(),
  })
  .superRefine((attribution, context) => {
    if (
      attribution.origin === "user_request" &&
      (attribution.parent_goal !== null || attribution.source_ref !== null)
    ) {
      context.addIssue({
        code: "custom",
        message: "user_request must not carry attribution metadata",
      });
    }
    if (
      attribution.origin === "plan_derived" &&
      (attribution.parent_goal === null || attribution.source_ref !== null)
    ) {
      context.addIssue({
        code: "custom",
        message: "plan_derived requires parent_goal and forbids source_ref",
      });
    }
    if (
      (attribution.origin === "app_content" || attribution.origin === "tool_suggestion") &&
      (attribution.parent_goal !== null || attribution.source_ref === null)
    ) {
      context.addIssue({
        code: "custom",
        message: `${attribution.origin} requires source_ref and forbids parent_goal`,
      });
    }
  });

const formEvidenceSchema = z.strictObject({
  form_origin: canonicalHttpOriginText,
  form_action: httpUrlText,
});

const stepApprovalRequestedEventSchema = z
  .strictObject({
    kind: z.literal("step_approval_requested"),
    request_id: nonEmptyText,
    task_id: nonEmptyText,
    step_id: nonEmptyText,
    action: nonEmptyText.max(512),
    target: nonEmptyText,
    summary: nonEmptyText,
    impact: nonEmptyText,
    diff: approvalDiffSchema,
    instruction_attribution: instructionAttributionSchema,
    risk: z.literal("high"),
    reversibility: z.literal("l3_irreversible"),
    scope_options: z.tuple([z.literal("once")]),
    evidence: formEvidenceSchema,
    expires_at_ms: z.number().int().nonnegative(),
  })
  .superRefine((event, context) => {
    if (event.target !== event.evidence.form_origin) {
      context.addIssue({
        code: "custom",
        message: "target must equal the evidence form_origin",
        path: ["target"],
      });
    }
    const actionOrigin = getHttpOrigin(event.evidence.form_action);
    if (actionOrigin !== event.evidence.form_origin) {
      context.addIssue({
        code: "custom",
        message: "form_action must use the form_origin",
        path: ["evidence", "form_action"],
      });
    }
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
  stepApprovalRequestedEventSchema,
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

function isAbsoluteHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      url.username === "" &&
      url.password === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
}

function isCanonicalHttpOrigin(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      url.origin === value &&
      url.username === "" &&
      url.password === "" &&
      url.pathname === "/" &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
}

function getHttpOrigin(value: string): string | null {
  try {
    const url = new URL(value);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return null;
    }
    return url.origin;
  } catch {
    return null;
  }
}
