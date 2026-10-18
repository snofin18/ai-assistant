# assistant-capture

Platform-agnostic window screenshot pipeline (ADR-0071). This is a **boundary
skeleton** created by TASK-238: the responsibilities below are frozen, the
zero-dependency orchestration is implemented by TASK-041 split A.

## Responsibilities

- Orchestrate one window screenshot end to end: resolve the target window, call
  `WindowProvider::capture`, apply redaction, run scroll cleanup, and decide
  whether image bytes are retained under the active privacy mode.
- Own the capture-side retention policy and its explicit failure modes.

## Boundaries

- **No platform access.** The only screenshot primitive is the
  `WindowProvider::capture` trait from `crates/platform/api`. No Win32, UIA, COM,
  or `std::process::Command` appears here (iron rule 7).
- **No policy decision.** Whether an egress is allowed is decided by `crates/dlp`
  and `crates/policy`, not here.
- **No third-party dependency yet.** Image codecs and perceptual hashing are
  added by their own cards through a registered, ADR-approved dependency.

## Invariants

1. A capture is always scoped to one resolved window, never the whole screen.
2. `NeverPersist` never returns or retains an `ImageRef`.
3. An unsupported or failed capture is an explicit error, never a blank image.
4. Any long-lived state (if added later) has a hard bound and an eviction or
   rejection policy (ADR-0063).

## Implemented slice (TASK-041 A)

- `CapturePipeline` validates already-classified redaction rules before calling
  the injected `WindowProvider::capture` primitive.
- `PrivacyMode::NeverPersist` drops the returned `ImageRef` and exposes only
  dimensions plus validated occlusion decisions.
- `ScrollCleanupPlan` carries an explicit merge direction and rejects zero or
  over-limit step counts.
- Pixel masking, image codecs, and real platform screenshot implementation
  remain outside this crate.

## Known Limitations

- **No image codec or perceptual hash.** Pixel masking, BGRA encoding and
  pHash / dHash are not implemented here; they need a registered,
  ADR-approved dependency and their own card.
- **No platform screenshot.** The only primitive is the injected
  `WindowProvider::capture`; the Windows implementation lives in
  `crates/platform/windows`, so this crate is unusable without a provider.
- **Redaction is applied, not classified.** Rules must already be classified by
  the caller (`crates/dlp` owns the rule model); an unclassified region is never
  discovered here.
- **`ScrollCleanupPlan` is a plan, not a merge.** It validates direction and step
  bounds; performing the actual image merge needs the codec above.
- **Retention is a decision, not storage.** `NeverPersist` drops the returned
  `ImageRef`; blob persistence is injected by the assembly layer
  (`ImageBlobSink`, ADR-0076).
