import { useState } from "react";

import {
  costPeriods,
  parseCostPanelModel,
  summarizeCosts
} from "./costModel.js";
import type { CostBreakdown, CostPeriod, CostSummary } from "./costModel.js";
import type { CostPanelCopy } from "./costCopy.js";

interface CostPanelProps {
  model: unknown;
  copy: CostPanelCopy;
  nowIso: string;
}

export function CostPanel({ model, copy, nowIso }: CostPanelProps) {
  const [period, setPeriod] = useState<CostPeriod>("task");
  const parsed = parseCostPanelModel(model);
  if (!parsed.ok) {
    return <ValidationError copy={copy} />;
  }
  const summaryResult = summarizeCosts(parsed.model, period, nowIso);
  if (!summaryResult.ok) {
    return <ValidationError copy={copy} />;
  }
  return (
    <section className="border border-slate-700 bg-slate-950 p-4 text-slate-100">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-lg font-semibold">{copy.heading}</h2>
        <div className="flex flex-wrap gap-2">
          {costPeriods.map((candidatePeriod) => (
            <button
              aria-pressed={candidatePeriod === period}
              className={
                candidatePeriod === period
                  ? "border border-cyan-400 px-3 py-1 text-sm"
                  : "border border-slate-600 px-3 py-1 text-sm"
              }
              key={candidatePeriod}
              onClick={() => setPeriod(candidatePeriod)}
              type="button"
            >
              {copy.periodLabels[candidatePeriod]}
            </button>
          ))}
        </div>
      </div>
      <Totals copy={copy} summary={summaryResult.summary} />
      <BreakdownTable
        breakdowns={summaryResult.summary.byModel}
        copy={copy}
        heading={copy.byModelHeading}
        keyLabel={copy.modelColumnLabel}
      />
      <BreakdownTable
        breakdowns={summaryResult.summary.byApp}
        copy={copy}
        heading={copy.byAppHeading}
        keyLabel={copy.appColumnLabel}
      />
    </section>
  );
}

function Totals({ copy, summary }: { copy: CostPanelCopy; summary: CostSummary }) {
  return (
    <dl className="mt-4 grid gap-2 text-xs md:grid-cols-3">
      <Metric label={copy.totalCostLabel} value={copy.formatMicroUsd(summary.totals.costMicroUsd)} />
      <Metric label={copy.callCountLabel} value={String(summary.totals.callCount)} />
      <Metric label={copy.inputTokensLabel} value={copy.formatTokens(summary.totals.inputTokens)} />
      <Metric
        label={copy.cachedTokensLabel}
        value={copy.formatTokens(summary.totals.cachedInputTokens)}
      />
      <Metric label={copy.outputTokensLabel} value={copy.formatTokens(summary.totals.outputTokens)} />
      <Metric label={copy.latencyLabel} value={copy.formatLatency(summary.totals.latencyMs)} />
    </dl>
  );
}

function BreakdownTable({
  heading,
  keyLabel,
  breakdowns,
  copy
}: {
  heading: string;
  keyLabel: string;
  breakdowns: CostBreakdown[];
  copy: CostPanelCopy;
}) {
  return (
    <>
      <h3 className="mt-5 text-sm font-medium">{heading}</h3>
      <div className="mt-2 overflow-x-auto">
        <table className="w-full border-collapse text-left text-xs">
          <thead>
            <tr className="border-y border-slate-700 text-slate-400">
              <th className="py-2 pr-3 font-medium">{keyLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.totalCostLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.inputTokensLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.outputTokensLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.latencyLabel}</th>
            </tr>
          </thead>
          <tbody>
            {breakdowns.map((breakdown) => (
              <tr className="border-b border-slate-800" key={breakdown.key}>
                <th className="py-2 pr-3 font-mono font-normal">{breakdown.key}</th>
                <td className="py-2 pr-3">{copy.formatMicroUsd(breakdown.costMicroUsd)}</td>
                <td className="py-2 pr-3">{copy.formatTokens(breakdown.inputTokens)}</td>
                <td className="py-2 pr-3">{copy.formatTokens(breakdown.outputTokens)}</td>
                <td className="py-2 pr-3">{copy.formatLatency(breakdown.latencyMs)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3 border border-slate-700 p-2">
      <dt className="text-slate-400">{label}</dt>
      <dd className="text-right font-mono">{value}</dd>
    </div>
  );
}

function ValidationError({ copy }: { copy: CostPanelCopy }) {
  return (
    <div className="border border-red-500 bg-red-950/50 p-4 text-red-100" role="alert">
      {copy.validationErrorLabel}
    </div>
  );
}
