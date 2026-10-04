# assistant-dlp

Data-loss-prevention policies (ADR-0071): three-tier egress, redaction rules,
and screenshot occlusion. This is a **boundary skeleton** created by TASK-238:
the responsibilities below are frozen, the behavior is implemented by TASK-050
(egress policy) and consumed by TASK-041 (`redact`).

## Responsibilities

- Model the three egress tiers `local_only` / `redacted` / `full` (`[ADR:待建 0007]`),
  with per-application and per-content-type overrides.
- Hold redaction rules (password fields, regex-matched regions) and expose
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

## Invariants

1. `local_only` without a local model is an explicit error, not a downgrade.
2. Redaction is fail-closed: an unclassifiable region is over-redacted, never
   left visible.
3. Any long-lived rule table has a hard bound and an eviction or rejection
   policy (ADR-0063).
