//! Idempotency classification: did the previous step actually happen?
//!
//! Responsibility: answer architecture v2 section 7.3 use 2 ("崩溃恢复时判断「上次那一步到底做没做」",
//! detailed in section 8.5) from the evidence a recovery pass can collect: the fingerprint before the
//! step, the fingerprint after it, and — when the application has an interface that can tell us —
//! the application's own report.
//!
//! Boundary: this module does not re-run the step, does not decide whether re-running is safe, and
//! does not touch storage. `task-engine` turns the verdict into a recovery transition; the Host
//! executes it.
//!
//! ## Invariants
//!
//! 1. **Never guess**: incomplete evidence yields [`ApplicationVerdict::Unknown`]. In particular a
//!    missing before or after fingerprint is never read as "nothing happened" — an unrecorded
//!    fingerprint is a gap in our knowledge, not evidence of stability.
//! 2. **The application's own report wins** (section 7.4: `app_reported` is the L1 channel, "最可信").
//!    When the application states the effect is absent, that beats a fingerprint difference, which
//!    can always be caused by a component the adapter failed to ignore.
//! 3. **Pure function**: no IO, no clock, no randomness.

use assistant_platform_api::Fingerprint;
use serde::{Deserialize, Serialize};

/// What the recovery pass concluded about a previous step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ApplicationVerdict {
    /// The evidence says the step's effect is present, so re-running it would double-apply.
    Applied,
    /// The evidence says the step's effect is absent, so it is safe to run again.
    NotApplied,
    /// The evidence is incomplete or contradictory; a human (or a fuller probe) must decide.
    Unknown,
}

/// The evidence a recovery pass collected about one previous step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ApplicationEvidence {
    /// The state fingerprint recorded before the step ran.
    pub before: Option<Fingerprint>,
    /// The state fingerprint recorded after the step ran (or at recovery time).
    pub after: Option<Fingerprint>,
    /// What the application itself reported about the effect, when it has such an interface.
    ///
    /// `None` means "the application was not asked or cannot answer", which is different from
    /// `Some(false)` ("the application says the effect is not there").
    pub marker_observed: Option<bool>,
}

impl ApplicationEvidence {
    /// Creates evidence from a before/after fingerprint pair and no application report.
    #[must_use]
    pub const fn from_fingerprints(before: Fingerprint, after: Fingerprint) -> Self {
        Self {
            before: Some(before),
            after: Some(after),
            marker_observed: None,
        }
    }

    /// Creates evidence from the application's own report.
    #[must_use]
    pub const fn from_marker(marker_observed: bool) -> Self {
        Self {
            before: None,
            after: None,
            marker_observed: Some(marker_observed),
        }
    }
}

/// Classifies a previous step from the collected evidence.
///
/// The precedence is deliberate and documented in the module header: the application's report
/// first, then the fingerprint transition, then `Unknown`.
#[must_use]
pub fn classify_application(evidence: &ApplicationEvidence) -> ApplicationVerdict {
    if let Some(marker_observed) = evidence.marker_observed {
        return if marker_observed {
            ApplicationVerdict::Applied
        } else {
            ApplicationVerdict::NotApplied
        };
    }

    let (Some(before), Some(after)) = (evidence.before.as_ref(), evidence.after.as_ref()) else {
        // Invariant 1: one fingerprint is not half an answer.
        return ApplicationVerdict::Unknown;
    };

    if before == after {
        ApplicationVerdict::NotApplied
    } else {
        // A fingerprint difference is evidence the step had *an* effect; it cannot prove causation,
        // which is exactly why section 7.3 requires per-adapter ignore lists — a jittering title
        // would otherwise make every step look applied.
        ApplicationVerdict::Applied
    }
}
