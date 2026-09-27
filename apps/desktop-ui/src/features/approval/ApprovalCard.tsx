import { useEffect } from "react";

import { ApprovalDiffView } from "./ApprovalDiffView.js";
import { AuthorizationScopePicker } from "./AuthorizationScopePicker.js";
import { EvidenceList } from "./EvidenceList.js";
import { OriginBadge } from "./OriginBadge.js";
import { getApprovalBlockReason } from "./approvalModel.js";
import type { ApprovalDecisionEvent } from "./approvalModel.js";
import type { ApprovalCardCopy } from "./approvalCopy.js";
import { useApprovalCard } from "./useApprovalCard.js";

interface ApprovalCardProps {
  model: unknown;
  copy: ApprovalCardCopy;
  onDecision: (decision: ApprovalDecisionEvent) => void;
  onRequestModification: () => void;
  onDryRun: () => void;
}

export function ApprovalCard({ model, copy, onDecision, onRequestModification, onDryRun }: ApprovalCardProps) {
  const { state, dispatch } = useApprovalCard(model);
  useEffect(() => {
    if (state.decision === null) {
      return;
    }
    onDecision(state.decision);
    dispatch({ type: "reset_decision" });
  }, [dispatch, onDecision, state.decision]);
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
  const parsedModel = state.parseResult.model;
  const blockReason = getApprovalBlockReason(
    parsedModel,
    state,
    copy.irreversibleConfirmationPhrase
  );
  const isApprovalBlocked = blockReason !== null;
  return (
    <article
      aria-labelledby={`approval-heading-${parsedModel.requestId}`}
      className="border border-amber-600 bg-slate-900 p-4 text-slate-100"
    >
      <header className="border-b border-slate-700 pb-3">
        <h2 className="text-lg font-semibold" id={`approval-heading-${parsedModel.requestId}`}>
          {copy.cardHeading}
        </h2>
        <p className="mt-1 font-mono text-sm text-amber-200">{parsedModel.action}</p>
      </header>
      <dl className="mt-4 grid gap-3 md:grid-cols-2">
        <Field label={copy.targetLabel} value={parsedModel.targetLabel} />
        <Field label={copy.riskLabel} value={copy.riskLabels[parsedModel.riskLevel]} />
        <Field label={copy.summaryLabel} value={parsedModel.summary} />
        <Field label={copy.impactLabel} value={parsedModel.impact} />
        <Field
          label={copy.reversibilityLabel}
          value={copy.reversibilityLabels[parsedModel.reversibility]}
        />
        <Field
          label={copy.undoLabel}
          value={
            parsedModel.undo.isAvailable
              ? `${copy.undoMethodMessages[parsedModel.undo.methodKey] ?? parsedModel.undo.methodKey} (${parsedModel.undo.anchorId})`
              : copy.errorMessages[parsedModel.undo.disabledReasonKey ?? ""] ?? copy.notApplicable
          }
        />
      </dl>
      <div className="mt-4">
        <OriginBadge copy={copy} origin={parsedModel.instructionOrigin} />
      </div>
      {parsedModel.instructionOrigin === "app_content" ? (
        <label className="mt-3 flex items-center gap-2 text-sm text-red-100">
          <input
            checked={state.hasAppContentOverride}
            onChange={(event) =>
              dispatch({
                type: "set_app_content_override",
                isEnabled: event.currentTarget.checked,
              })
            }
            type="checkbox"
          />
          {copy.appContentOverrideLabel}
        </label>
      ) : null}
      <p className="mt-3 text-sm text-slate-300">
        <strong>{copy.toolReasonLabel}: </strong>
        {parsedModel.toolSelectionReason}
      </p>
      <section className="mt-4">
        <h3 className="mb-2 text-sm font-medium">{copy.diffLabel}</h3>
        <ApprovalDiffView copy={copy} diff={parsedModel.diff} />
      </section>
      <div className="mt-4">
        <AuthorizationScopePicker
          copy={copy}
          model={parsedModel}
          onSelectScope={(scope) => dispatch({ type: "select_scope", scope })}
          selectedScope={state.selectedScope}
        />
      </div>
      <div className="mt-4">
        <EvidenceList copy={copy} evidence={parsedModel.evidence} />
      </div>
      {parsedModel.reversibility === "L3Irreversible" ? (
        <label className="mt-4 block border border-red-500 p-3 text-sm text-red-100">
          <span>{copy.irreversibleConfirmationLabel}</span>
          <input
            className="mt-2 w-full border border-red-400 bg-slate-950 p-2"
            onChange={(event) =>
              dispatch({
                type: "set_irreversible_confirmation",
                confirmation: event.currentTarget.value,
              })
            }
            value={state.irreversibleConfirmation}
          />
        </label>
      ) : null}
      {parsedModel.isExpired ? (
        <p className="mt-4 border border-amber-500 p-2 text-sm text-amber-100" role="status">
          {copy.expiredWarning}
        </p>
      ) : null}
      {state.errorKey === null ? null : (
        <p className="mt-3 text-sm text-red-200" role="alert">
          {copy.errorMessages[state.errorKey] ?? state.errorKey}
        </p>
      )}
      <label className="mt-4 block text-sm">
        <span>{copy.denialReasonLabel}</span>
        <textarea
          className="mt-1 w-full border border-slate-600 bg-slate-950 p-2"
          onChange={(event) =>
            dispatch({ type: "set_denial_reason", reason: event.currentTarget.value })
          }
          value={state.denialReason}
        />
      </label>
      <div className="mt-4 flex flex-wrap gap-2">
        <button
          className="border border-red-500 px-3 py-2 text-sm disabled:opacity-50"
          onClick={() => dispatch({ type: "deny" })}
          type="button"
        >
          {copy.denyLabel}
        </button>
        <button
          className="border border-slate-500 px-3 py-2 text-sm disabled:opacity-50"
          onClick={onRequestModification}
          type="button"
        >
          {copy.modifyParametersLabel}
        </button>
        <button
          className="border border-slate-500 px-3 py-2 text-sm disabled:opacity-50"
          onClick={onDryRun}
          type="button"
        >
          {copy.dryRunLabel}
        </button>
        <button
          className="border border-emerald-500 px-3 py-2 text-sm disabled:opacity-50"
          disabled={isApprovalBlocked}
          onClick={() =>
            dispatch({
              type: "approve",
              requiredIrreversibleConfirmation: copy.irreversibleConfirmationPhrase,
            })
          }
          type="button"
        >
          {copy.approveLabel}
        </button>
      </div>
    </article>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs uppercase tracking-wide text-slate-400">{label}</dt>
      <dd className="mt-1 text-sm">{value}</dd>
    </div>
  );
}
