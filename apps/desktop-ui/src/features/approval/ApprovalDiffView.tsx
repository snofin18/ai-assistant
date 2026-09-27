import type { ApprovalDiff } from "./approvalModel.js";
import type { ApprovalCardCopy } from "./approvalCopy.js";

interface ApprovalDiffViewProps {
  diff: ApprovalDiff;
  copy: ApprovalCardCopy;
}

export function ApprovalDiffView({ diff, copy }: ApprovalDiffViewProps) {
  if (diff.kind === "none") {
    return <p className="text-sm text-slate-400">{copy.noDiff}</p>;
  }
  if (diff.kind === "irreversible") {
    return (
      <div className="border border-red-500 bg-red-950/40 p-3 text-sm text-red-100">
        <strong className="block">{copy.irreversibleWarning}</strong>
        <span>{copy.warningMessages[diff.warningKey] ?? diff.warningKey}</span>
      </div>
    );
  }
  return (
    <div className="overflow-hidden border border-slate-700">
      {diff.kind === "text" ? (
        <ul aria-label={copy.diffLabel} className="divide-y divide-slate-800 font-mono text-xs">
          {diff.entries.map((entry, index) => (
            <li
              className={entry.kind === "added" ? "bg-emerald-950/50" : "bg-red-950/50"}
              key={`${entry.kind}-${entry.beforeLine ?? entry.afterLine ?? index}`}
            >
              <span className="inline-block w-12 px-2 text-slate-400">
                {entry.kind === "added" ? entry.afterLine : entry.beforeLine}
              </span>
              <span className="whitespace-pre-wrap">
                {entry.kind === "added" ? "+ " : "- "}
                {entry.text}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      {diff.kind === "fields" ? (
        <table className="w-full border-collapse text-left text-sm">
          <thead className="bg-slate-800">
            <tr>
              <th className="p-2">{copy.diffField}</th>
              <th className="p-2">{copy.diffBefore}</th>
              <th className="p-2">{copy.diffAfter}</th>
            </tr>
          </thead>
          <tbody>
            {diff.entries.map((entry) => (
              <tr className="border-t border-slate-700" key={entry.field}>
                <th className="p-2 font-medium">{entry.field}</th>
                <td className="p-2 text-red-200">{entry.before}</td>
                <td className="p-2 text-emerald-200">{entry.after}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      {diff.kind === "files" ? (
        <ul aria-label={copy.diffLabel} className="divide-y divide-slate-800 text-sm">
          {diff.entries.map((entry) => (
            <li className="p-2" key={entry.path}>
              <span className="font-medium">{entry.path}</span>
              <span className="ml-3 text-slate-400">
                {copy.fileChangeLabels[entry.kind]} {formatBytes(entry.beforeBytes, copy)} -{" "}
                {formatBytes(entry.afterBytes, copy)}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      {diff.kind === "ui_steps" ? (
        <ol aria-label={copy.diffLabel} className="divide-y divide-slate-800 text-sm">
          {diff.entries.map((entry) => (
            <li className="p-2" key={entry.stepId}>
              <strong>{entry.action}</strong>
              <span className="ml-2 text-slate-400">{entry.targetLabel}</span>
              {entry.screenshotRef === null ? null : (
                <a className="ml-3 underline" href={entry.screenshotRef}>
                  {copy.screenshotEvidence}
                </a>
              )}
            </li>
          ))}
        </ol>
      ) : null}
    </div>
  );
}

function formatBytes(value: number | null, copy: ApprovalCardCopy): string {
  return value === null ? copy.notApplicable : copy.formatBytes(value);
}
