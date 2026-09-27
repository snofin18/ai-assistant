import {
  getReplayAvailability,
  getUndoAvailability,
  type TimelineStepModel,
} from "./timelineModel.js";
import type { TimelineCopy } from "./timelineCopy.js";

interface TimelineStepCardProps {
  step: TimelineStepModel;
  copy: TimelineCopy;
  isSelected: boolean;
  onSelect: () => void;
  onUndo: () => void;
  onReplay: () => void;
}

export function TimelineStepCard({
  step,
  copy,
  isSelected,
  onSelect,
  onUndo,
  onReplay,
}: TimelineStepCardProps) {
  const undo = getUndoAvailability(step);
  const replay = getReplayAvailability(step);
  return (
    <li className={isSelected ? "border-l-4 border-cyan-400 bg-slate-900" : "bg-slate-950"}>
      <button
        aria-pressed={isSelected}
        className="w-full border border-slate-700 p-3 text-left"
        onClick={onSelect}
        type="button"
      >
        <div className="flex flex-wrap items-center justify-between gap-2">
          <strong>{step.action}</strong>
          <span className="border border-slate-600 px-2 py-1 text-xs">
            {copy.stepStatusLabels[step.status]}
          </span>
        </div>
        <p className="mt-1 text-sm text-slate-300">{step.targetLabel}</p>
        <dl className="mt-2 grid gap-2 text-xs text-slate-300 md:grid-cols-2">
          <div>
            <dt className="font-medium">{copy.preFingerprintLabel}</dt>
            <dd className="font-mono">{step.preFingerprint ?? copy.empty}</dd>
          </div>
          <div>
            <dt className="font-medium">{copy.postFingerprintLabel}</dt>
            <dd className="font-mono">{step.postFingerprint ?? copy.empty}</dd>
          </div>
        </dl>
        <div className="mt-2 flex flex-wrap gap-2 text-xs">
          <span className="border border-slate-600 px-2 py-1">
            {copy.verificationLabels[step.verification]}
          </span>
          <span className="border border-slate-600 px-2 py-1">
            {copy.reversibilityLabels[step.reversibility]}
          </span>
          <span>{copy.durationLabel(step.durationMs)}</span>
          <span>{copy.costLabel(step.cost.usd)}</span>
          <span>{copy.tokensLabel(step.cost.tokensIn, step.cost.tokensOut)}</span>
        </div>
      </button>
      <div className="border-x border-b border-slate-700 p-3">
        <div className="flex flex-wrap gap-2">
          {step.evidence.map((evidence) => (
            <a
              className="border border-slate-600 px-2 py-1 text-xs underline"
              href={evidence.href}
              key={evidence.id}
            >
              {copy.evidenceMessages[evidence.labelKey] ?? evidence.labelKey} (
              {copy.evidenceKindLabels[evidence.kind]})
            </a>
          ))}
        </div>
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            className="border border-amber-500 px-3 py-1 text-sm disabled:opacity-50"
            disabled={!undo.isAvailable}
            onClick={onUndo}
            title={undo.reasonKey === null ? copy.undoLabel : copy.operationMessages[undo.reasonKey]}
            type="button"
          >
            {copy.undoLabel}
          </button>
          {undo.reasonKey === null ? null : (
            <span className="self-center text-xs text-amber-200">
              {copy.operationMessages[undo.reasonKey] ?? undo.reasonKey}
            </span>
          )}
          <button
            className="border border-cyan-500 px-3 py-1 text-sm disabled:opacity-50"
            disabled={!replay.isAvailable}
            onClick={onReplay}
            title={
              replay.reasonKey === null
                ? copy.replayLabel
                : copy.operationMessages[replay.reasonKey]
            }
            type="button"
          >
            {copy.replayLabel}
          </button>
          {replay.reasonKey === null ? null : (
            <span className="self-center text-xs text-cyan-200">
              {copy.operationMessages[replay.reasonKey] ?? replay.reasonKey}
            </span>
          )}
        </div>
      </div>
    </li>
  );
}
