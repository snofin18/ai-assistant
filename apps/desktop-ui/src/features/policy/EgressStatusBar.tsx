import { getEffectiveEgressLevel, parseEgressPolicy } from "./egressPolicy.js";
import type { EgressPolicyCopy } from "./policyCopy.js";

interface EgressStatusBarProps {
  policy: unknown;
  copy: EgressPolicyCopy;
  currentAppId?: string;
}

export function EgressStatusBar({ policy, copy, currentAppId }: EgressStatusBarProps) {
  const parsed = parseEgressPolicy(policy);
  const effectiveLevel = parsed.ok
    ? getEffectiveEgressLevel(parsed.policy, currentAppId ?? null)
    : null;
  return (
    <footer
      aria-live="polite"
      className="fixed inset-x-0 bottom-0 border-t border-slate-700 bg-slate-950 px-4 py-2 text-xs text-slate-200"
      role="contentinfo"
    >
      <span>{copy.statusLabel}: </span>
      <strong>
        {effectiveLevel === null ? copy.invalidPolicyLabel : copy.levelLabels[effectiveLevel]}
      </strong>
      {currentAppId === undefined ? null : (
        <span className="ml-2 font-mono text-slate-400">{currentAppId}</span>
      )}
    </footer>
  );
}
