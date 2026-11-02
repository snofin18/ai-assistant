/**
 * Strict parser for the Tauri event-bridge envelope.
 *
 * Core's UiEvent is already contract-validated by `uiEventSchema`. The bridge
 * adds only an internal discriminator so transport failures share one ordered
 * channel with normal events.
 */

import { z } from "zod";

import type { ContractParseResult, UiEvent } from "./contract.js";

export const UI_BRIDGE_EVENT = "assistant://ui-bridge";
export const START_UI_EVENT_STREAM_COMMAND = "start_ui_event_stream";
export const UI_EVENT_STREAM_ALREADY_RUNNING = "ui_event_stream_already_running";

const bridgeFailureSchema = z.strictObject({
  code: z.string().trim().min(1),
  message: z.string().trim().min(1),
});

const bridgeMessageSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("event"),
    event: z.unknown(),
  }),
  z.strictObject({
    kind: z.literal("failure"),
    failure: bridgeFailureSchema,
  }),
]);

export type UiBridgeFailure = z.infer<typeof bridgeFailureSchema>;
export type UiBridgeMessage =
  { kind: "event"; event: UiEvent } | { kind: "failure"; failure: UiBridgeFailure };

export function parseUiBridgeMessage(
  input: unknown,
  parseEvent: (value: unknown) => ContractParseResult<UiEvent>,
): ContractParseResult<UiBridgeMessage> {
  const result = bridgeMessageSchema.safeParse(input);
  if (!result.success) {
    return {
      ok: false,
      errors: result.error.issues.map((issue) => {
        const path = issue.path.join(".");
        return path.length === 0 ? issue.message : `${path}: ${issue.message}`;
      }),
    };
  }
  if (result.data.kind === "failure") {
    return { ok: true, value: result.data };
  }
  const event = parseEvent(result.data.event);
  return event.ok
    ? { ok: true, value: { kind: "event", event: event.value } }
    : { ok: false, errors: event.errors.map((error) => `event: ${error}`) };
}

export function eventFromBridge(message: UiBridgeMessage): UiEvent | null {
  return message.kind === "event" ? message.event : null;
}
