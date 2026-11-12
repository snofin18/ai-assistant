/**
 * The single typed IPC entry point for the desktop UI.
 *
 * The UI is an untrusted input source and Core's replies are untrusted too, so
 * both directions cross a zod boundary here. A payload that fails validation
 * throws a typed error instead of being sent or rendered.
 */

import { invoke } from "@tauri-apps/api/core";

import {
  UI_IPC_VERSION,
  parseUiCommandEnvelope,
  parseUiCommandOutcome,
  parseUiEvent,
} from "./contract.js";
import type { UiCommand, UiCommandOutcome, UiEvent } from "./contract.js";
import {
  parseUiBridgePoll,
  READ_UI_EVENT_STREAM_COMMAND,
  START_UI_EVENT_STREAM_COMMAND,
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
 * Starts the single Core event pump and polls the retained bridge message.
 *
 * Polling uses a custom Tauri command so the shell keeps an empty capability
 * set. The returned function stops polling and reports nothing else.
 */
export async function connectUiEventStream(handlers: UiEventStreamHandlers): Promise<() => void> {
  let lastSequence = 0;
  let stopped = false;
  let isPolling = false;
  let timer: ReturnType<typeof setInterval> | null = null;

  function stop() {
    stopped = true;
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }

  function fail(error: HostIpcError) {
    stop();
    handlers.onFailure(error);
  }

  async function poll() {
    if (stopped || isPolling) {
      return;
    }
    isPolling = true;
    try {
      const raw: unknown = await invokeHost<unknown>(READ_UI_EVENT_STREAM_COMMAND);
      if (raw === null) {
        return;
      }
      const parsed = parseUiBridgePoll(raw, parseUiEvent);
      if (!parsed.ok) {
        fail(new HostIpcError("ui_event_payload_invalid", parsed.errors));
        return;
      }
      if (parsed.value.sequence <= lastSequence) {
        return;
      }
      lastSequence = parsed.value.sequence;
      if (parsed.value.message.kind === "failure") {
        fail(
          new HostIpcError(parsed.value.message.failure.code, [
            parsed.value.message.failure.message,
          ]),
        );
        return;
      }
      handlers.onEvent(parsed.value.message.event);
    } catch (error) {
      fail(normalizeInvokeFailure(error));
    } finally {
      isPolling = false;
    }
  }

  try {
    await invokeHost<void>(START_UI_EVENT_STREAM_COMMAND);
  } catch (error) {
    const failure = normalizeInvokeFailure(error);
    if (failure.code === UI_EVENT_STREAM_ALREADY_RUNNING) {
      timer = setInterval(() => {
        void poll();
      }, 100);
      void poll();
      return stop;
    }
    handlers.onFailure(failure);
    throw failure;
  }
  timer = setInterval(() => {
    void poll();
  }, 100);
  void poll();
  return stop;
}

function normalizeInvokeFailure(error: unknown): HostIpcError {
  if (error instanceof HostIpcError) {
    return error;
  }
  if (typeof error === "string") {
    return new HostIpcError("ui_event_stream_unavailable", [error]);
  }
  if (typeof error === "object" && error !== null) {
    const code = "code" in error && typeof error.code === "string" ? error.code : null;
    const message = "message" in error && typeof error.message === "string" ? error.message : null;
    if (code !== null) {
      return new HostIpcError(code, message === null ? [] : [message]);
    }
    let serialized = "unknown object";
    try {
      serialized = JSON.stringify(error);
    } catch {
      // Keep the stable fallback when a host error cannot be serialized.
    }
    return new HostIpcError("ui_event_stream_unavailable", [serialized]);
  }
  return new HostIpcError("ui_event_stream_unavailable", [
    error instanceof Error ? error.message : "unknown Tauri failure",
  ]);
}
