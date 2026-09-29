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
} from "./contract.js";
import type { UiCommand, UiCommandOutcome } from "./contract.js";

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

export async function invokeHost<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
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
