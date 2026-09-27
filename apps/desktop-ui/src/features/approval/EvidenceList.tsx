import type { ApprovalEvidence } from "./approvalModel.js";
import type { ApprovalCardCopy } from "./approvalCopy.js";

interface EvidenceListProps {
  evidence: ApprovalEvidence[];
  copy: ApprovalCardCopy;
}

export function EvidenceList({ evidence, copy }: EvidenceListProps) {
  return (
    <div>
      <h3 className="text-sm font-medium">{copy.evidenceLabel}</h3>
      <ul className="mt-1 flex flex-wrap gap-2 text-sm">
        {evidence.map((entry) => (
          <li className="border border-slate-700 px-2 py-1" key={entry.id}>
            <a className="underline" href={entry.href}>
              {copy.evidenceMessages[entry.labelKey] ?? entry.labelKey}
            </a>
            <span className="ml-2 text-slate-400">{copy.evidenceKindLabels[entry.kind]}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
