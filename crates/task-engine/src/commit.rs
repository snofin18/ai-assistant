//! The verified-commit input value object.
//!
//! Responsibilities:
//! - carry the opaque [`VerificationReceipt`] that authorizes one commit;
//! - keep the commit metadata (`post_fingerprint`, `had_warning`) in one place.
//!
//! Boundaries:
//! - does not transition state, persist checkpoints, or run verification;
//! - cannot be constructed without a receipt minted by `assistant-verify`.
//!
//! Invariants:
//! - the receipt is owned, so one proof authorizes exactly one commit;
//! - there is no constructor that omits verification.

use assistant_verify::VerificationReceipt;

/// Inputs required to commit exactly one verified step.
///
/// The commit consumes the [`VerificationReceipt`] by value, so the same proof
/// cannot be replayed for a second commit and there is no constructor path that
/// omits verification. The receipt is intentionally unconstructible outside
/// `assistant-verify`.
#[derive(Debug)]
pub struct StepCommit {
    verification: VerificationReceipt,
    post_fingerprint: Option<String>,
    had_warning: bool,
}

impl StepCommit {
    /// Builds a commit from a verified receipt plus post-fingerprint metadata.
    ///
    /// `had_warning` records a non-fatal warning observed during the step so the
    /// task can settle as `CompletedWithWarnings` instead of silently dropping
    /// it.
    #[must_use]
    pub const fn new(
        verification: VerificationReceipt,
        post_fingerprint: Option<String>,
        had_warning: bool,
    ) -> Self {
        Self {
            verification,
            post_fingerprint,
            had_warning,
        }
    }

    /// Returns the verification proof carried by this commit.
    #[must_use]
    pub const fn verification(&self) -> &VerificationReceipt {
        &self.verification
    }

    /// Consumes the commit into its parts for the engine's state transition.
    pub(crate) fn into_parts(self) -> (VerificationReceipt, Option<String>, bool) {
        (self.verification, self.post_fingerprint, self.had_warning)
    }
}
