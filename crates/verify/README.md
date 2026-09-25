# assistant-verify

> Stage 1 postcondition assertion engine: 11 assertion kinds, state fingerprints,
> idempotency classification, and `on_violation` dispatch (architecture v2 sections
> 7.3, 7.4, and 8.5).

## Responsibilities

- Parse `PlanStep.postconditions` JSON into a strongly typed `Postcondition`, fail-closed.
- Evaluate every postcondition against a caller-supplied `Observation` and reduce the
  results to one `VerifyOutcome`.
- Build a state fingerprint from a `FingerprintSubject` plus a per-adapter ignore set.
- Classify whether a previous step was applied, for crash recovery.
- Map a violated postcondition to the action the Host must take.

## Boundaries

- **No platform access.** No UIA, no Win32, no AT-SPI, no filesystem, no network, no clock.
  Collecting an observation is the Host's job; this crate only decides.
- **No execution.** It does not retry, roll back, prompt, or abort. It returns a decision;
  acting on it belongs to the Host, `undo`, and `hitl`.
- **No storage.** No checkpoints, no audit rows.
- **No policy.** Whether an action is allowed at all is `crates/policy`'s single release
  point (AGENTS.md rule 3).
- **No `visual_assert`.** Screenshots, perceptual hashing, tolerance, and `confidence_min`
  belong to TASK-042 (`crates/verify/src/visual/**`).
- **No preconditions.** `target_resolvable` and `capability` are `preconditions`
  (architecture section 5.3), not postconditions, and are rejected with that reason.

## Invariants

1. **A failed verification never returns ok.** `Verified` requires at least one postcondition
   and every one of them satisfied. Any falsification gives `Violated`; otherwise any
   unevaluable postcondition gives `Inconclusive`. Both non-success outcomes carry
   `ErrorCode::VerifyFailed`, so an envelope built from them cannot claim `ok: true`.
2. **Unevaluable is not satisfied.** An unprobed element is not an absent element; an
   unrecorded file is not an unchanged file; a missing previous fingerprint is not a stable
   state.
3. **Parsing is fail-closed.** An unknown `kind`, an unexpected extra field, a missing field,
   or a wrong type is rejected at parse time and never evaluated.
4. **Pure functions.** Equal inputs give equal outputs; no IO, clock, randomness, or global
   mutable state. A fingerprint is reproducible across runs and processes.
5. **Idempotency never guesses.** Incomplete evidence yields `Unknown`, never `Applied`.
6. **Ignores are explicit.** There is no default ignore list; a global default would be an
   implicit rule nobody can audit (section 7.3).

## The 11 assertion kinds

`state_assert`, `text_contains`, `text_not_contains`, `state_changed`, `state_unchanged`,
`element_exists`, `element_gone`, `value_equals`, `value_in_range`, `file_changed`,
`app_reported`.

## Typical use

```rust
use assistant_platform_api::Fingerprint;
use assistant_protocol::serde_json::json;
use assistant_verify::{Observation, VerifyOutcome, parse_postconditions, verify_postconditions};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let postconditions = parse_postconditions(&[
    json!({"kind": "text_contains", "value": "hello"}),
    json!({"kind": "state_changed", "within_ms": 2000}),
])?;

let before = Fingerprint::parse(
    "sha256:0000000000000000000000000000000000000000000000000000000000000000",
)?;
let after = Fingerprint::parse(
    "sha256:1111111111111111111111111111111111111111111111111111111111111111",
)?;
let observation = Observation {
    text: "hello world".to_owned(),
    previous_fingerprint: Some(before),
    elapsed_since_previous_ms: Some(120),
    ..Observation::new("document.body", "untitled", after)
};

assert!(matches!(
    verify_postconditions(&postconditions, &observation),
    VerifyOutcome::Verified { .. }
));
# Ok(())
# }
```

## Known limitations

- **`assert` free-form strings are not implemented.** Appendix A declares
  `assertion.assert` as a free-form string (`target.text.contains(new_text)`). Supporting it
  means shipping an expression language the model cannot enumerate, which is the same
  objection section 7.4 raises against `if`-`then`-`else`. Only the structured form
  (`field` + `op` + `value`) is accepted; an `assert` field is rejected with a pointer to it.
  Recorded as `DRIFT-023-4`.
- **`retry_once` is narrowed.** Section 7.4 allows it "only for transient errors". The
  dispatch retries for `Transient` and `TargetNotFound`; every other category escalates to the
  user instead of repeating a call that would fail the same way. In particular `VerifyFailed`
  is *not* retried here even though `ErrorCode::retryable()` is true for it: that flag answers
  "may the agent retry the task", not "may we re-issue this exact call".
- **Fingerprint causation.** `classify_application` reports `Applied` when the fingerprint
  moved. That is evidence of *an* effect, not proof that this step caused it. The per-adapter
  ignore list is what keeps jitter (a clock in the title, a caret) from faking a change.
- **Scope is a string.** `Observation::fingerprint_scope` is a caller-supplied string rather
  than `assistant_platform_api::FingerprintScope`, because the platform enum only has
  `WholeWindow` and `Element(ResolvedElement)` and `ResolvedElement` is a non-serialisable
  handle (AGENTS.md rule 8). Binding to it would drag a handle into this crate.

## Related documents

- Architecture v2 sections 7.3, 7.4, 8.5, 9, and appendix A (`#/$defs/assertion`).
- `docs/spec/tool-schema.md` section 4 invariant 3 (postconditions are mandatory for writes).
- `docs/spec/naming.md` section 7 (`Postcondition` is a controlled term).
- `tasks/TASK-023-verify-postcondition-assertion-engine.md`.
- `docs/DEPENDENCIES.md` (`serde`, `sha2`, `thiserror` usage sites).
