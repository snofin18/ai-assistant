# assistant-dlp

Data-loss-prevention policies (ADR-0071): three-tier egress, redaction rules,
and screenshot occlusion. TASK-050 implements the pure egress-policy contract
frozen by `docs/adr/0007-egress-policy-tiers-and-resolution.md`; TASK-041
implements the redaction rule model.

## Responsibilities

- Model the three egress tiers `local_only` / `redacted` / `full`
  (`docs/adr/0007-egress-policy-tiers-and-resolution.md`),
  with per-application and per-content-type overrides.
- Resolve the effective level, require egress-destination allow-list membership
  for non-local egress, and emit bounded change records for host-side audit.
- Hold redaction rules (password fields, caller-classified regions) and expose
  screenshot occlusion so `crates/capture` can apply them.
- Fail closed: choosing `local_only` with no local model is an explicit error,
  never a silent failover to a cloud path.

## Boundaries

- **No platform access.** No Win32, UIA, COM, or `std::process::Command`
  (iron rule 7).
- **No capture orchestration.** Scroll cleanup, retention, and the pipeline
  itself live in `crates/capture`; this crate only answers "what must be
  redacted / is this egress allowed".
- **No third-party dependency yet.** Regex engines or other helpers are added by
  their own cards through a registered, ADR-approved dependency.
- **No provider selection or policy persistence.** The host supplies local-model
  availability and writes audit records; this crate only returns pure decisions.

## Invariants

1. `local_only` without a local model is an explicit error, not a downgrade.
2. Redaction is fail-closed: an unclassifiable region is over-redacted, never
   left visible.
3. Any long-lived rule table has a hard bound and an eviction or rejection
   policy (ADR-0063).
4. An application override replaces the global default; a content-type override
   can only tighten the resulting level and can never widen it.
5. `local_only` without available local inference fails explicitly and never
  falls back to a cloud egress destination.
