# assistant-capture

Platform-agnostic window screenshot pipeline (ADR-0071). This is a **boundary
skeleton** created by TASK-238: the responsibilities below are frozen, the
behavior is implemented by TASK-041.

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
2. Privacy mode never persists image bytes.
3. An unsupported or failed capture is an explicit error, never a blank image.
4. Any long-lived state (if added later) has a hard bound and an eviction or
   rejection policy (ADR-0063).
