import { TimelineStepCard } from "./TimelineStepCard.js";
import type { TimelineCopy } from "./timelineCopy.js";
import { useTimeline } from "./useTimeline.js";

interface ExecutionTimelineProps {
  model: unknown;
  copy: TimelineCopy;
  onUndoRequested: (stepId: string) => void;
  onReplayRequested: (stepId: string) => void;
}

export function ExecutionTimeline({
  model,
  copy,
  onUndoRequested,
  onReplayRequested,
}: ExecutionTimelineProps) {
  const { state, dispatch } = useTimeline(model);
  if (!state.parseResult.ok) {
    return (
      <div className="border border-red-500 bg-red-950/50 p-4 text-red-100" role="alert">
        <strong>{copy.validationErrorLabel}</strong>
        <ul className="mt-2 list-disc pl-5 text-sm">
          {state.parseResult.errors.map((error) => (
            <li key={error}>{error}</li>
          ))}
        </ul>
      </div>
    );
  }
  if (state.parseResult.steps.length === 0) {
    return <p className="text-sm text-slate-400">{copy.empty}</p>;
  }
  return (
    <section className="border border-slate-700 bg-slate-950 p-4 text-slate-100">
      <h2 className="text-lg font-semibold">{copy.heading}</h2>
      {state.errorKey === null ? null : (
        <p className="mt-3 border border-red-500 p-2 text-sm text-red-100" role="alert">
          {copy.operationMessages[state.errorKey] ?? state.errorKey}
        </p>
      )}
      <ol className="mt-4 space-y-3">
        {state.parseResult.steps.map((step) => (
          <TimelineStepCard
            copy={copy}
            isSelected={state.selectedStepId === step.stepId}
            key={step.stepId}
            onReplay={() => {
              dispatch({ type: "request_replay", stepId: step.stepId });
              onReplayRequested(step.stepId);
            }}
            onSelect={() => dispatch({ type: "select_step", stepId: step.stepId })}
            onUndo={() => {
              dispatch({ type: "request_undo", stepId: step.stepId });
              onUndoRequested(step.stepId);
            }}
            step={step}
          />
        ))}
      </ol>
    </section>
  );
}
