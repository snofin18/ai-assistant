/**
 * User-intent model for the desktop UI.
 *
 * The UI only expresses intent; planning stays in Core. The built command still
 * crosses `sendUiCommand`'s contract check before it is sent, so an empty or
 * malformed intent surfaces as an explicit error instead of a silent no-op.
 */

import type { UiCommand } from "../../ipc/contract.js";

export const INTENT_ID_PREFIX = "i_ui_";

export function buildSubmitIntentCommand(intentId: string, goal: string): UiCommand {
  return { kind: "submit_intent", intent_id: intentId, goal };
}

/**
 * Builds a locally unique intent id without pulling in a uuid dependency.
 */
export function nextIntentId(nowMs: number, sequence: number): string {
  return `${INTENT_ID_PREFIX}${nowMs}_${sequence}`;
}
