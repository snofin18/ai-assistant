import { ElementPicker } from "../picker/ElementPicker.js";
import { SelectorCandidateList } from "../picker/SelectorCandidateList.js";
import type { BindingWizardCopy } from "./bindingCopy.js";
import { bindingWizardSteps } from "./bindingModel.js";
import { useTargetBindingWizard } from "./useTargetBindingWizard.js";

interface TargetBindingWizardProps {
  input: unknown;
  copy: BindingWizardCopy;
  onDraftReady: (serializedDraft: string) => void;
}

export function TargetBindingWizard({ input, copy, onDraftReady }: TargetBindingWizardProps) {
  const [state, dispatch] = useTargetBindingWizard(input);
  if (!state.parseResult.ok) {
    return (
      <div className="border border-red-500 bg-red-950/40 p-3 text-sm text-red-100" role="alert">
        <strong>{copy.validationErrorLabel}</strong>
      </div>
    );
  }
  const parsed = state.parseResult.value;
  return (
    <section className="grid gap-4" aria-label={copy.heading}>
      <header>
        <h2 className="text-lg font-semibold">{copy.heading}</h2>
        <ol className="mt-2 flex flex-wrap gap-2 text-xs">
          {bindingWizardSteps.map((step) => (
            <li
              aria-current={step === state.step ? "step" : undefined}
              className={
                step === state.step
                  ? "border border-cyan-500 px-2 py-1 text-cyan-200"
                  : "border border-slate-700 px-2 py-1 text-slate-400"
              }
              key={step}
            >
              {copy.stepLabels[step]}
            </li>
          ))}
        </ol>
      </header>
      {state.step === "application" ? (
        <dl className="grid gap-2 border border-slate-700 p-3 text-sm sm:grid-cols-2">
          <Field label={copy.appIdLabel} value={parsed.metadata.appId} />
          <Field label={copy.displayNameLabel} value={parsed.metadata.displayName} />
          <Field label={copy.targetNameLabel} value={parsed.metadata.targetName} />
          <Field label={copy.versionRangeLabel} value={parsed.metadata.versionRange} />
        </dl>
      ) : null}
      {state.step === "target" ? (
        <ElementPicker
          copy={copy.picker}
          model={parsed.snapshot}
          onSelectElement={(elementId) => dispatch({ type: "select_element", elementId })}
        />
      ) : null}
      {state.step === "review" ? (
        <section aria-label={copy.candidateHeading}>
          <h3 className="mb-2 text-sm font-semibold">{copy.candidateHeading}</h3>
          <SelectorCandidateList candidates={state.candidates} copy={copy.picker} />
        </section>
      ) : null}
      {state.step === "export" && state.serializedDraft !== null ? (
        <pre className="max-h-96 overflow-auto border border-slate-700 p-3 font-mono text-xs">
          {state.serializedDraft}
        </pre>
      ) : null}
      {state.errorKey === null ? null : (
        <p className="text-sm text-red-200" role="alert">
          {copy.errorMessages[state.errorKey] ?? copy.unknownErrorLabel}
        </p>
      )}
      <div className="flex gap-2">
        {state.step !== "application" ? (
          <button
            className="border border-slate-500 px-3 py-2 text-sm"
            onClick={() => dispatch({ type: "back" })}
            type="button"
          >
            {copy.backLabel}
          </button>
        ) : null}
        {state.step !== "export" ? (
          <button
            className="border border-cyan-600 px-3 py-2 text-sm"
            onClick={() => dispatch({ type: "continue" })}
            type="button"
          >
            {copy.continueLabel}
          </button>
        ) : (
          <button
            className="border border-emerald-600 px-3 py-2 text-sm"
            onClick={() => {
              if (state.serializedDraft !== null) {
                onDraftReady(state.serializedDraft);
              }
            }}
            type="button"
          >
            {copy.exportLabel}
          </button>
        )}
      </div>
    </section>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="font-medium text-slate-400">{label}</dt>
      <dd className="break-all font-mono">{value}</dd>
    </div>
  );
}
