import {
  getCapabilityStats,
  parseCapabilityMatrix
} from "./capabilityMatrix.js";
import type { CapabilityMatrixCopy } from "./capabilityCopy.js";

interface CapabilityMatrixViewProps {
  model: unknown;
  copy: CapabilityMatrixCopy;
}

export function CapabilityMatrixView({ model, copy }: CapabilityMatrixViewProps) {
  const parsed = parseCapabilityMatrix(model);
  if (!parsed.ok) {
    return (
      <div className="border border-red-500 bg-red-950/50 p-4 text-red-100" role="alert">
        {copy.validationErrorLabel}
      </div>
    );
  }
  const matrix = parsed.matrix;
  const stats = getCapabilityStats(matrix);
  return (
    <section className="border border-slate-700 bg-slate-950 p-4 text-slate-100">
      <h2 className="text-lg font-semibold">{copy.heading}</h2>
      <dl className="mt-3 grid gap-2 text-xs md:grid-cols-2">
        <Metric label={copy.probeLabel} value={matrix.probe.probedAt} />
        <Metric label={copy.platformLabel} value={matrix.probe.platformOs} />
        <Metric
          label={copy.sessionLabel}
          value={`${String(matrix.probe.isSessionLocked)} / ${String(matrix.probe.isSessionRemote)} / ${String(matrix.probe.isSessionHeadless)}`}
        />
        <Metric label={copy.totalLabel} value={String(stats.total)} />
        <Metric label={copy.approvalRequiredLabel} value={String(stats.requiresApproval)} />
        <Metric label={copy.forbiddenLabel} value={String(stats.forbidden)} />
        <Metric label={copy.degradedChannelsLabel} value={String(stats.degradedChannels)} />
      </dl>
      <h3 className="mt-5 text-sm font-medium">{copy.channelsHeading}</h3>
      <div className="mt-2 overflow-x-auto">
        <table className="w-full border-collapse text-left text-xs">
          <tbody>
            {matrix.channels.map((channel) => (
              <tr className="border-t border-slate-700" key={channel.name}>
                <th className="py-2 pr-3 font-mono font-normal">{channel.name}</th>
                <td className="py-2 pr-3">
                  {copy.channelAvailabilityLabels[channel.availability]}
                </td>
                <td className="py-2 text-slate-300">{channel.detail ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <h3 className="mt-5 text-sm font-medium">{copy.capabilitiesHeading}</h3>
      <div className="mt-2 overflow-x-auto">
        <table className="w-full border-collapse text-left text-xs">
          <thead>
            <tr className="border-y border-slate-700 text-slate-400">
              <th className="py-2 pr-3 font-medium">{copy.capabilityIdColumnLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.resourceColumnLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.riskColumnLabel}</th>
              <th className="py-2 pr-3 font-medium">{copy.approvalColumnLabel}</th>
            </tr>
          </thead>
          <tbody>
            {matrix.capabilities.map((capability) => (
              <tr className="border-b border-slate-800" key={capability.id}>
                <th className="py-2 pr-3 font-mono font-normal">{capability.id}</th>
                <td className="py-2 pr-3">{copy.resourceLabels[capability.resource]}</td>
                <td className="py-2 pr-3">
                  {copy.riskLabels[capability.risk]} / {copy.sideEffectLabels[capability.sideEffect]}
                </td>
                <td className="py-2 pr-3">{copy.approvalLabels[capability.approval]}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <h3 className="mt-5 text-sm font-medium">{copy.degradationsHeading}</h3>
      {matrix.degradations.length === 0 ? (
        <p className="mt-2 text-sm text-slate-400">{copy.noDegradations}</p>
      ) : (
        <ul className="mt-2 space-y-2 text-sm">
          {matrix.degradations.map((degradation) => (
            <li className="border border-amber-700 p-2" key={degradation.id}>
              <strong className="font-mono text-xs">{degradation.id}</strong>
              <p className="mt-1 text-slate-200">{degradation.impact}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
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
