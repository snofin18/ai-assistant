import type { InstructionOrigin } from "./approvalModel.js";
import type { ApprovalCardCopy } from "./approvalCopy.js";

interface OriginBadgeProps {
  origin: InstructionOrigin;
  copy: ApprovalCardCopy;
}

export function OriginBadge({ origin, copy }: OriginBadgeProps) {
  const isBlocked = origin === "app_content";
  return (
    <div
      className={
        isBlocked
          ? "border border-red-500 bg-red-950/50 p-3 text-red-100"
          : "border border-slate-600 bg-slate-800/60 p-3"
      }
    >
      <span className="font-medium">{copy.originLabel}</span>
      <span className="ml-2">{copy.originLabels[origin]}</span>
      {isBlocked ? <p className="mt-1 text-sm">{copy.appContentWarning}</p> : null}
    </div>
  );
}
