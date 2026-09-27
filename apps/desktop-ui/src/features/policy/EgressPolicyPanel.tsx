import { useEffect, useState } from "react";

import { egressLevels } from "./egressPolicy.js";
import type {
  EgressPolicyChange,
  EgressLevel
} from "./egressPolicy.js";
import type { EgressPolicyCopy } from "./policyCopy.js";
import { useEgressPolicy } from "./useEgressPolicy.js";

interface EgressPolicyPanelProps {
  policy: unknown;
  copy: EgressPolicyCopy;
  onChange: (change: EgressPolicyChange) => void;
}

export function EgressPolicyPanel({ policy, copy, onChange }: EgressPolicyPanelProps) {
  const [state, dispatch] = useEgressPolicy(policy);
  const [appId, setAppId] = useState("");

  useEffect(() => {
    dispatch({ type: "replace_policy", policy });
  }, [dispatch, policy]);

  useEffect(() => {
    if (state.change === null) {
      return;
    }
    onChange(state.change);
    dispatch({ type: "reset_change" });
  }, [dispatch, onChange, state.change]);

  if (!state.parseResult.ok) {
    return (
      <div className="border border-red-500 bg-red-950/50 p-4 text-red-100" role="alert">
        {copy.validationErrorLabel}
      </div>
    );
  }
  const parsedPolicy = state.parseResult.policy;
  return (
    <section className="border border-slate-700 bg-slate-950 p-4 text-slate-100">
      <h2 className="text-lg font-semibold">{copy.panelHeading}</h2>
      <LevelSelector
        copy={copy}
        label={copy.defaultLevelLabel}
        onSelect={(level) => dispatch({ type: "request_change", request: { kind: "default", level } })}
        selectedLevel={parsedPolicy.defaultLevel}
      />
      {state.pendingChange === null ? null : (
        <div className="mt-4 border border-amber-600 bg-amber-950/30 p-3 text-sm">
          <p>{copy.upgradeWarningLabel}</p>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              className="border border-amber-400 px-3 py-2"
              onClick={() => dispatch({ type: "confirm_pending_change" })}
              type="button"
            >
              {copy.confirmUpgradeLabel}
            </button>
            <button
              className="border border-slate-500 px-3 py-2"
              onClick={() => dispatch({ type: "cancel_pending_change" })}
              type="button"
            >
              {copy.cancelUpgradeLabel}
            </button>
          </div>
        </div>
      )}
      <div className="mt-5 border-t border-slate-700 pt-4">
        <h3 className="text-sm font-medium">{copy.appOverrideLabel}</h3>
        {!parsedPolicy.hasLocalModel ? (
          <p className="mt-2 border border-amber-500 p-2 text-sm text-amber-100" role="status">
            {copy.localModelUnavailableLabel}
          </p>
        ) : null}
        <label className="mt-3 block text-sm">
          <span>{copy.appIdLabel}</span>
          <input
            className="mt-1 w-full border border-slate-600 bg-slate-900 p-2 font-mono"
            onChange={(event) => setAppId(event.currentTarget.value)}
            spellCheck={false}
            value={appId}
          />
        </label>
        <div className="mt-3 flex flex-wrap gap-2">
          {egressLevels.map((level) => (
            <button
              className="border border-slate-600 px-3 py-2 text-sm disabled:opacity-50"
              disabled={appId.trim() === ""}
              key={level}
              onClick={() =>
                dispatch({
                  type: "request_change",
                  request: {
                    kind: "app_override",
                    appId: appId.trim(),
                    level
                  }
                })
              }
              type="button"
            >
              {copy.levelLabels[level]}
            </button>
          ))}
          <button
            className="border border-slate-500 px-3 py-2 text-sm disabled:opacity-50"
            disabled={appId.trim() === ""}
            onClick={() =>
              dispatch({
                type: "request_change",
                request: {
                  kind: "clear_app_override",
                  appId: appId.trim()
                }
              })
            }
            type="button"
          >
            {copy.clearOverrideLabel}
          </button>
        </div>
        <dl className="mt-3 grid gap-2 text-xs text-slate-300 md:grid-cols-2">
          {Object.entries(parsedPolicy.appOverrides).map(([overrideAppId, level]) => (
            <div className="flex justify-between gap-3 border border-slate-700 p-2" key={overrideAppId}>
              <dt className="font-mono">{overrideAppId}</dt>
              <dd>{copy.levelLabels[level]}</dd>
            </div>
          ))}
        </dl>
      </div>
      {state.errorKey === null ? null : (
        <p className="mt-4 border border-red-500 p-2 text-sm text-red-100" role="alert">
          {copy.errorMessages[state.errorKey] ?? copy.validationErrorLabel}
        </p>
      )}
    </section>
  );
}

function LevelSelector({
  label,
  selectedLevel,
  copy,
  onSelect
}: {
  label: string;
  selectedLevel: EgressLevel;
  copy: EgressPolicyCopy;
  onSelect: (level: EgressLevel) => void;
}) {
  return (
    <fieldset className="mt-4">
      <legend className="text-sm font-medium">{label}</legend>
      <div className="mt-2 grid gap-2 md:grid-cols-3">
        {egressLevels.map((level) => (
          <button
            aria-pressed={selectedLevel === level}
            className={
              selectedLevel === level
                ? "border border-cyan-400 bg-cyan-950/40 p-3 text-left"
                : "border border-slate-700 p-3 text-left"
            }
            key={level}
            onClick={() => onSelect(level)}
            type="button"
          >
            <strong className="block text-sm">{copy.levelLabels[level]}</strong>
            <span className="mt-1 block text-xs text-slate-300">
              {copy.levelDescriptions[level]}
            </span>
          </button>
        ))}
      </div>
    </fieldset>
  );
}
