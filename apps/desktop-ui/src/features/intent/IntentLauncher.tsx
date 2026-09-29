/**
 * Minimal intent launcher: the first real UI → Core call site.
 *
 * It only submits the user's goal. Planning, policy, and execution stay in
 * Core; this component renders whatever Core reports and never infers success.
 */

import { useState } from "react";

import { HostIpcError, sendUiCommand } from "../../ipc/client.js";
import { defaultIntentLauncherCopy } from "./intentCopy.js";
import type { IntentLauncherCopy } from "./intentCopy.js";
import { buildSubmitIntentCommand, nextIntentId } from "./intentModel.js";

type LauncherState =
  | { kind: "idle" }
  | { kind: "submitting" }
  | { kind: "accepted" }
  | { kind: "rejected"; detail: string };

export interface IntentLauncherProps {
  copy?: IntentLauncherCopy;
  now?: () => number;
}

export function IntentLauncher({
  copy = defaultIntentLauncherCopy,
  now = Date.now,
}: IntentLauncherProps) {
  const [goal, setGoal] = useState("");
  const [state, setState] = useState<LauncherState>({ kind: "idle" });

  async function submit() {
    setState({ kind: "submitting" });
    const command = buildSubmitIntentCommand(nextIntentId(now(), 1), goal);
    try {
      await sendUiCommand(command);
      setState({ kind: "accepted" });
    } catch (error) {
      const detail = error instanceof HostIpcError ? error.code : "ipc_unavailable";
      setState({ kind: "rejected", detail });
    }
  }

  const isSubmitting = state.kind === "submitting";

  return (
    <section className="border border-slate-700 p-6">
      <h2 className="text-base font-semibold">{copy.heading}</h2>
      <label className="mt-3 block text-sm text-slate-300" htmlFor="intent-goal">
        {copy.goalLabel}
      </label>
      <input
        id="intent-goal"
        className="mt-1 w-full border border-slate-700 bg-slate-900 px-3 py-2 text-sm"
        value={goal}
        placeholder={copy.goalPlaceholder}
        onChange={(event) => setGoal(event.target.value)}
        disabled={isSubmitting}
      />
      <button
        type="button"
        className="mt-3 border border-slate-500 px-3 py-1 text-sm disabled:opacity-50"
        onClick={() => {
          void submit();
        }}
        disabled={isSubmitting || goal.trim().length === 0}
      >
        {isSubmitting ? copy.submitting : copy.submit}
      </button>
      {state.kind === "accepted" ? (
        <p className="mt-3 text-sm text-emerald-400">{copy.accepted}</p>
      ) : null}
      {state.kind === "rejected" ? (
        <p className="mt-3 text-sm text-rose-400">
          {copy.rejectedPrefix}: {state.detail}
        </p>
      ) : null}
    </section>
  );
}
