/**
 * The single typed IPC entry point for the desktop UI.
 *
 * The UI is an untrusted input source and Core's replies are untrusted too, so
 * both directions cross a zod boundary here. A payload that fails validation
 * throws a typed error instead of being sent or rendered.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  UI_IPC_VERSION,
  parseUiCommandEnvelope,
  parseUiCommandOutcome,
  parseUiEvent,
} from "./contract.js";
import type { UiCommand, UiCommandOutcome, UiEvent } from "./contract.js";
import {
  parseUiBridgeMessage,
  START_UI_EVENT_STREAM_COMMAND,
  UI_BRIDGE_EVENT,
  UI_EVENT_STREAM_ALREADY_RUNNING,
} from "./eventStream.js";

/** Tauri command name registered by `src-tauri` for UI commands. */
export const SEND_UI_COMMAND = "send_ui_command";

/** Structured failure raised at the IPC boundary. */
export class HostIpcError extends Error {
  readonly code: string;
  readonly details: string[];

  constructor(code: string, details: string[]) {
    super(`${code}: ${details.join("; ")}`);
    this.name = "HostIpcError";
    this.code = code;
    this.details = details;
  }
}

export async function invokeHost<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}

/**
 * Validates and sends one UI command, then validates Core's outcome.
 *
 * @throws HostIpcError when the command or the returned outcome fails the
 *   contract, or when the transport reports a failure.
 */
export async function sendUiCommand(command: UiCommand): Promise<UiCommandOutcome> {
  const envelope = parseUiCommandEnvelope({ version: UI_IPC_VERSION, command });
  if (!envelope.ok) {
    throw new HostIpcError("ui_command_invalid", envelope.errors);
  }
  const raw: unknown = await invokeHost<unknown>(SEND_UI_COMMAND, {
    envelope: envelope.value,
  });
  const outcome = parseUiCommandOutcome(raw);
  if (!outcome.ok) {
    throw new HostIpcError("ui_command_outcome_invalid", outcome.errors);
  }
  return outcome.value;
}

export interface UiEventStreamHandlers {
  onEvent: (event: UiEvent) => void;
  onFailure: (error: HostIpcError) => void;
}

/**
 * Subscribes to Core events and asks Tauri to start the single event pump.
 *
 * The returned function removes the webview listener. Core disconnect is
 * reported through `onFailure`; commands are never replayed automatically.
 */
export async function connectUiEventStream(handlers: UiEventStreamHandlers): Promise<UnlistenFn> {
  const unlisten = await listen<unknown>(UI_BRIDGE_EVENT, ({ payload }) => {
    const parsed = parseUiBridgeMessage(payload, parseUiEvent);
    if (!parsed.ok) {
      handlers.onFailure(new HostIpcError("ui_event_payload_invalid", parsed.errors));
      return;
    }
    if (parsed.value.kind === "failure") {
      handlers.onFailure(
        new HostIpcError(parsed.value.failure.code, [parsed.value.failure.message]),
      );
      return;
    }
    handlers.onEvent(parsed.value.event);
  });
  try {
    await invokeHost<void>(START_UI_EVENT_STREAM_COMMAND);
  } catch (error) {
    const failure = normalizeInvokeFailure(error);
    if (failure.code === UI_EVENT_STREAM_ALREADY_RUNNING) {
      return unlisten;
    }
    unlisten();
    handlers.onFailure(failure);
    throw failure;
  }
  return unlisten;
}

function normalizeInvokeFailure(error: unknown): HostIpcError {
  if (error instanceof HostIpcError) {
    return error;
  }
  if (typeof error === "object" && error !== null) {
    const code = "code" in error && typeof error.code === "string" ? error.code : null;
    const message = "message" in error && typeof error.message === "string" ? error.message : null;
    if (code !== null) {
      return new HostIpcError(code, message === null ? [] : [message]);
    }
  }
  return new HostIpcError("ui_event_stream_unavailable", [
    error instanceof Error ? error.message : "unknown Tauri failure",
  ]);
}
