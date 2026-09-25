//! Combining per-postcondition outcomes into one verdict for a step.
//!
//! Responsibility: run [`evaluate_postcondition`] over every postcondition of a step and reduce the
//! results to [`VerifyOutcome`], plus the protocol [`ErrorCode`] the caller must report.
//!
//! Boundary: it does not execute anything, does not choose an `on_violation` action (that is
//! [`crate::dispatch_on_violation`]), and does not write audit rows.
//!
//! ## The reduction (invariant 1 of the crate)
//!
//! ```text
//! any Falsified                        -> Violated      (report VerifyFailed)
//! else any NotEvaluable                -> Inconclusive  (report VerifyFailed)
//! else every Satisfied                 -> Verified      (report ok)
//! empty postcondition list             -> Inconclusive  (report VerifyFailed)
//! ```
//!
//! The first line is section 7.4's red line: a falsified postcondition can never produce a success.
//! The second line is the same red line applied to ignorance: a step whose effect we could not
//! observe has not been verified, so it must not be reported as `ok: true` either. Both non-success
//! outcomes carry [`ErrorCode::VerifyFailed`] so that the envelope is honest by construction.
//!
//! ## Why an empty list is not a success
//!
//! `postconditions` has `minItems: 1` in the tool schema (appendix A) and `task-engine` rejects a
//! write step without postconditions, so an empty list means a caller skipped a contract. "Every
//! postcondition holds" is vacuously true for an empty list, and that vacuous truth is exactly how a
//! verification layer ends up rubber-stamping an unchecked action. It is reported as
//! `Inconclusive` instead.
//!
//! ## Invariants
//!
//! 1. `Verified` is returned only when at least one postcondition was evaluated and every one of
//!    them was `Satisfied`.
//! 2. `verifications` has exactly one entry per input postcondition, in input order.
//! 3. `violations` / `unevaluable` are non-empty exactly for their respective outcome.
//! 4. Pure function: no IO, no clock, no randomness.

use assistant_protocol::ErrorCode;
use serde::{Deserialize, Serialize};

use crate::assertion::{AssertionOutcome, evaluate_postcondition};
use crate::observation::Observation;
use crate::postcondition::Postcondition;

/// One postcondition's evaluation result, kept for the audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verification {
    /// Zero-based position of the postcondition inside the step.
    pub index: usize,
    /// The three-valued outcome.
    pub outcome: AssertionOutcome,
}

/// A falsified postcondition, rendered for the model and the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    /// Zero-based position of the postcondition inside the step.
    pub index: usize,
    /// What the postcondition asked for.
    pub expected: String,
    /// What the observation actually showed.
    pub actual: String,
}

/// A postcondition the observation could not decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unevaluable {
    /// Zero-based position of the postcondition inside the step.
    pub index: usize,
    /// What was missing, or why the observation cannot answer this question.
    pub reason: String,
}

/// The verdict for one step's postconditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VerifyOutcome {
    /// Every postcondition was satisfied.
    Verified {
        /// One entry per postcondition, in input order.
        verifications: Vec<Verification>,
    },
    /// At least one postcondition was falsified.
    Violated {
        /// One entry per postcondition, in input order.
        verifications: Vec<Verification>,
        /// The falsified postconditions (non-empty).
        violations: Vec<Violation>,
    },
    /// Nothing was falsified, but the step is not verified.
    Inconclusive {
        /// One entry per postcondition, in input order.
        verifications: Vec<Verification>,
        /// The postconditions that could not be evaluated (may be empty when no postcondition was
        /// declared at all).
        unevaluable: Vec<Unevaluable>,
        /// Why the verdict is not `Verified`.
        reason: String,
    },
}

impl VerifyOutcome {
    /// Whether every postcondition was satisfied.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }

    /// Whether at least one postcondition was falsified.
    #[must_use]
    pub const fn is_violated(&self) -> bool {
        matches!(self, Self::Violated { .. })
    }

    /// The falsified postconditions, or an empty slice when there are none.
    #[must_use]
    pub fn violations(&self) -> &[Violation] {
        match self {
            Self::Violated { violations, .. } => violations,
            _ => &[],
        }
    }

    /// The postconditions that could not be evaluated, or an empty slice when there are none.
    #[must_use]
    pub fn unevaluable(&self) -> &[Unevaluable] {
        match self {
            Self::Inconclusive { unevaluable, .. } => unevaluable,
            _ => &[],
        }
    }

    /// The protocol error category the caller must report, or `None` for a verified step.
    ///
    /// `Inconclusive` maps to [`ErrorCode::VerifyFailed`] on purpose: the 13 protocol categories
    /// (architecture v2 section 8.7) have no "unknown" variant, and reporting an unevaluable step as
    /// anything other than a verification failure would be the silent success section 7.4 forbids.
    #[must_use]
    pub const fn error_code(&self) -> Option<ErrorCode> {
        match self {
            Self::Verified { .. } => None,
            Self::Violated { .. } | Self::Inconclusive { .. } => Some(ErrorCode::VerifyFailed),
        }
    }
}

/// Evaluates every postcondition of a step against one observation.
#[must_use]
pub fn verify_postconditions(
    postconditions: &[Postcondition],
    observation: &Observation,
) -> VerifyOutcome {
    if postconditions.is_empty() {
        return VerifyOutcome::Inconclusive {
            verifications: Vec::new(),
            unevaluable: Vec::new(),
            reason: "no postconditions were declared, so nothing was verified".to_owned(),
        };
    }

    let mut verifications = Vec::with_capacity(postconditions.len());
    let mut violations = Vec::new();
    let mut unevaluable = Vec::new();

    for (index, postcondition) in postconditions.iter().enumerate() {
        let outcome = evaluate_postcondition(postcondition, observation);
        match &outcome {
            AssertionOutcome::Falsified { expected, actual } => violations.push(Violation {
                index,
                expected: expected.clone(),
                actual: actual.clone(),
            }),
            AssertionOutcome::NotEvaluable { reason } => unevaluable.push(Unevaluable {
                index,
                reason: reason.clone(),
            }),
            AssertionOutcome::Satisfied => {}
        }
        verifications.push(Verification { index, outcome });
    }

    if !violations.is_empty() {
        VerifyOutcome::Violated {
            verifications,
            violations,
        }
    } else if unevaluable.is_empty() {
        VerifyOutcome::Verified { verifications }
    } else {
        let reason = format!(
            "{} of {} postcondition(s) could not be evaluated",
            unevaluable.len(),
            postconditions.len()
        );
        VerifyOutcome::Inconclusive {
            verifications,
            unevaluable,
            reason,
        }
    }
}
