# assistant-undo

Reversibility levels, rollback anchors, rollback recipes, conflict detection, and incident
reports for architecture v2 section 9.

## Responsibilities

- Model the four reversibility levels L0 through L3 and compute the worst level in a task.
- Select and validate one rollback anchor: undo-stack steps, content snapshot, shadow copy,
  compensating action, or evidence-only for an irreversible step.
- Preserve the optional L1 content-snapshot fallback required when an L0 undo stack is treated as
  additive, and use it automatically when the undo recipe does not reach the anchor.
- Build and execute one typed rollback recipe through an injected `RollbackExecutor`.
- Detect user changes between the post-step fingerprint and the current fingerprint.
- Verify the final fingerprint after rollback and turn every unsafe or failed rollback into a
  structured `IncidentReport`.

## Boundaries

- **No platform access.** UIA, Win32, filesystem, network, and process management stay outside
  this crate. The caller injects a `RollbackExecutor`.
- **No policy decision.** Whether a rollback is allowed is decided by `crates/policy`.
- **No persistence or audit writes.** Anchors and incidents are values; persistence belongs to
  storage and audit callers.
- **No clock, randomness, or UUID generation.** Callers provide stable identifiers.
- **No precise diff reversal.** The conservative choices are restore-to-anchor or stop with an
  incident. Applying only the agent's diff is a later capability.
- **No direct incident delivery.** `publish_incident` calls an injected reporter so the caller
  owns the transport and can surface failures.

## Invariants

1. **A rollback is successful only after fingerprint verification.** Every recipe ends with the
   engine comparing the final observed fingerprint with the anchor's pre-step fingerprint.
2. **Unknown evidence is not "no conflict".** A missing post-step fingerprint or missing current
   fingerprint produces an incident and stops before executing any action.
3. **Conflict handling defaults to the conservative choice.** User changes block rollback unless
   the caller explicitly selects `RestoreOverall`; missing evidence always blocks.
4. **Anchors and recipes are validated against each other.** An L0 anchor cannot execute an L1
   recipe, an empty recipe is rejected, and undo counts are bounded to 1 through 50.
5. **Incident reporting never fails silently.** `publish_incident` returns a structured error if
   the reporter fails.
6. **No execution for L3.** An irreversible step yields an evidence-only anchor; asking for a
   rollback recipe is an explicit error.
7. **L0 may fall back to L1.** An undo-stack anchor can carry a content-snapshot digest. If the
   undo recipe fails or ends at the wrong fingerprint, the engine runs that snapshot recipe before
   declaring an incident.

## Typical Use

```rust
use assistant_platform_api::Fingerprint;
use assistant_undo::{
    Anchor, AnchorId, AnchorKind, ContentDigest, IncidentId, Reversibility, RollbackExecutor,
    RollbackRequest, StepId, TargetId, execute_rollback,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let anchor_id = AnchorId::parse("a_1")?;
let pre = Fingerprint::parse(
    "sha256:0000000000000000000000000000000000000000000000000000000000000000",
)?;
let post = Fingerprint::parse(
    "sha256:1111111111111111111111111111111111111111111111111111111111111111",
)?;
let digest = ContentDigest::parse(
    "2222222222222222222222222222222222222222222222222222222222222222",
)?;

let mut anchor = Anchor::new(
    anchor_id.clone(),
    StepId::parse("s_1")?,
    TargetId::parse("document_1")?,
    Reversibility::L1Snapshot,
    pre.clone(),
    AnchorKind::ContentSnapshot { content_digest: digest },
)?;
anchor.record_post_fingerprint(post.clone())?;

# struct NoopExecutor;
# impl RollbackExecutor for NoopExecutor {
#     fn execute(
#         &self,
#         action: &assistant_undo::RollbackAction,
#     ) -> Result<assistant_undo::ActionOutcome, assistant_undo::ExecutorFailure> {
#         let expected = match action {
#             assistant_undo::RollbackAction::RestoreContentSnapshot { .. } => pre.clone(),
#             _ => pre.clone(),
#         };
#         Ok(assistant_undo::ActionOutcome::new(expected))
#     }
# }
# let executor = NoopExecutor;
let request = RollbackRequest::new(
    &anchor,
    None,
    assistant_undo::ConflictResolution::FailClosed,
    &IncidentId::parse("i_1")?,
);
let _ = execute_rollback(&request, &executor);
# Ok(())
# }
```

## Known Limitations

- The crate verifies rollback against fingerprints, not content diffs. A fingerprint can prove
  that the expected state was reached, but it does not explain a mismatch to the user.
- Shadow-copy existence, integrity, and retention are checked by the executor that owns storage.
  This crate validates the path and digest shape only.
- `CompensatingAction.arguments_json` is carried as opaque text. The executor or tool bus must
  validate it against the target tool schema before execution.
- Incident delivery is synchronous and caller-supplied. The current code does not enqueue or
  retry notifications.
- There is no rollback-specific `ErrorCode`. Conflict and fingerprint failures map to the
  existing `VerifyFailed` category, while reporter failure maps to `Fatal`; adding a new protocol
  category requires an ADR.

## Related Documents

- Architecture v2 section 9, section 7.3, and section 12.
- `docs/spec/naming.md` section 7.
- `tasks/TASK-024-undo-four-level-rollback-anchor.md`.
