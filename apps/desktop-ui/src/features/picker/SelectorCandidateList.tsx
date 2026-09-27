import type { PickerCopy } from "./pickerCopy.js";
import type { SelectorCandidate } from "./pickerModel.js";
import { getPrimaryCandidateId } from "./pickerModel.js";

interface SelectorCandidateListProps {
  candidates: readonly SelectorCandidate[];
  copy: PickerCopy;
}

export function SelectorCandidateList({
  candidates,
  copy
}: SelectorCandidateListProps) {
  const primaryCandidateId = getPrimaryCandidateId(candidates);
  if (candidates.length === 0) {
    return null;
  }
  return (
    <section className="border border-slate-700 p-3" aria-label={copy.candidateHeading}>
      <h3 className="text-sm font-semibold">{copy.candidateHeading}</h3>
      <ol className="mt-3 space-y-2">
        {candidates.map((candidate) => (
          <li className="border border-slate-700 p-2 text-xs" key={candidate.id}>
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-mono">{candidate.kind}</span>
              <span>{candidate.score.toFixed(2)}</span>
              {candidate.id === primaryCandidateId ? (
                <span className="border border-emerald-500 px-1 text-emerald-200">
                  {copy.primaryLabel}
                </span>
              ) : null}
              {candidate.locale_dependent ? (
                <span className="border border-amber-500 px-1 text-amber-200">
                  {copy.localeDependentLabel}
                </span>
              ) : null}
            </div>
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all font-mono">
              {JSON.stringify(candidate.value)}
            </pre>
          </li>
        ))}
      </ol>
    </section>
  );
}
