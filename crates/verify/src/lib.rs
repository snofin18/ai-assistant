//! Postcondition assertion engine (architecture v2 section 7.4), state fingerprints
//! (section 7.3), idempotency classification (section 7.3 use 2), and `on_violation`
//! dispatch (section 7.4).
//!
//! Responsibilities:
//! - parse `PlanStep.postconditions` JSON into the strongly typed [`Postcondition`] (fail-closed);
//! - evaluate every postcondition against a caller-supplied [`Observation`] and produce a
//!   [`VerifyOutcome`];
//! - produce a state fingerprint from a [`FingerprintSubject`] with a configurable
//!   ignore list (section 7.3 "sufficiently sensitive but not over-sensitive");
//! - decide whether the previous step actually happened, from before/after fingerprints
//!   plus business markers ([`classify_application`]);
//! - map a violated postcondition to the action to take ([`dispatch_on_violation`]).
//!
//! Boundaries (what this crate deliberately does not do):
//! - **No platform access**: no UIA, no Win32, no filesystem, no network, no clock. The
//!   caller collects the observation and passes it in.
//! - **No execution**: it does not retry, roll back, or prompt. It only *decides*; execution
//!   belongs to the Host, `undo`, and `hitl`.
//! - **No storage**: no checkpoints, no audit rows.
//! - **No `visual_assert`**: screenshots, perceptual hashing, tolerance, and
//!   `confidence_min` belong to TASK-042 (`crates/verify/src/visual/**`).
//! - **No preconditions**: `target_resolvable` and `capability` are `preconditions`
//!   (architecture section 5.3) and are not part of the postcondition engine.
//!
//! Invariants:
//! 1. **A failed verification never returns ok**: only when *every* postcondition is
//!    `Satisfied` is the outcome [`VerifyOutcome::Verified`]. Any falsified postcondition
//!    gives `Violated`; with no falsification but at least one unevaluable postcondition the
//!    outcome is `Inconclusive` and is never upgraded to success.
//! 2. **Unevaluable is not satisfied**: something the observation does not contain (an
//!    unprobed element, an unrecorded file, a missing previous fingerprint) is always
//!    `NotEvaluable`; it is never treated as "absent", "unchanged", or "satisfied".
//! 3. **Parsing is fail-closed**: an unknown `kind`, an unexpected extra field (usually a
//!    typo), a missing field, or a wrong type is rejected at parse time and never evaluated.
//! 4. **Pure functions**: equal inputs give equal outputs; no IO, clock, randomness, or
//!    global mutable state.
//! 5. **Idempotency never guesses**: incomplete evidence (a missing before or after
//!    fingerprint) yields `Unknown`, never `Applied`.
//!
//! Typical use:
//! ```
//! use assistant_platform_api::Fingerprint;
//! use assistant_protocol::serde_json::json;
//! use assistant_verify::{
//!     Observation, VerifyOutcome, parse_postconditions, verify_postconditions,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let postconditions = parse_postconditions(&[
//!     json!({"kind": "text_contains", "value": "hello"}),
//!     json!({"kind": "state_changed", "within_ms": 2000}),
//! ])?;
//!
//! let before = Fingerprint::parse(
//!     "sha256:0000000000000000000000000000000000000000000000000000000000000000",
//! )?;
//! let after = Fingerprint::parse(
//!     "sha256:1111111111111111111111111111111111111111111111111111111111111111",
//! )?;
//! let observation = Observation {
//!     text: "hello world".to_owned(),
//!     previous_fingerprint: Some(before),
//!     elapsed_since_previous_ms: Some(120),
//!     ..Observation::new("document.body", "untitled", after)
//! };
//!
//! assert!(matches!(
//!     verify_postconditions(&postconditions, &observation),
//!     VerifyOutcome::Verified { .. }
//! ));
//! # Ok(())
//! # }
//! ```
//!
//! Related documents: architecture v2 sections 7.3, 7.4, 8.5, and 9;
//! `docs/spec/tool-schema.md` section 4 invariant 3 (postconditions are mandatory);
//! `docs/spec/naming.md` section 7 (`Postcondition` is a controlled term);
//! `tasks/TASK-023-verify-postcondition-assertion-engine.md`.

#![deny(unsafe_code)]

mod assertion;
mod error;
mod fingerprint;
mod idempotency;
mod json_field;
mod observation;
mod on_violation;
mod postcondition;
mod verdict;

pub use assertion::{AssertionOutcome, evaluate_postcondition, render_text};
pub use error::{VerifyError, VerifyResult};
pub use fingerprint::{
    ControlState, DocumentDigest, FingerprintField, FingerprintIgnore, FingerprintSubject,
    ScrollPosition, canonical_form, state_fingerprint,
};
pub use idempotency::{ApplicationEvidence, ApplicationVerdict, classify_application};
pub use observation::{FileSnapshot, FileTransition, Observation, ObservedElement};
pub use on_violation::{
    OnViolation, ViolationAction, ViolationKind, dispatch_on_violation, parse_on_violation,
};
pub use postcondition::{
    AssertValue, CompareOp, FileChangeKind, Postcondition, StateField, parse_postconditions,
};
pub use verdict::{Unevaluable, Verification, VerifyOutcome, Violation, verify_postconditions};
