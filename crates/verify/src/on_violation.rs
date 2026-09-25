//! `on_violation` parsing and dispatch (architecture v2 section 7.4).
//!
//! Responsibility: turn the `on_violation` string a tool declared into a typed strategy, classify
//! the failure, and decide which action the Host must take.
//!
//! Boundary: it **decides**, it does not act. Retrying, rolling back, prompting the user, and
//! aborting are Host / `undo` / `hitl` concerns. It also does not consult policy: whether an action
//! is allowed at all is `crates/policy`'s single release point (rule 3).
//!
//! ## The one strategy that is not passed through
//!
//! Section 7.4 allows `retry_once` **only for transient errors** ("仅对 transient 错误"). If a tool
//! declares `retry_once` but the failure is not one where a retry can plausibly help, blindly
//! retrying would repeat the same failure and burn budget, while silently dropping the strategy
//! would hide the mismatch. The dispatch therefore returns
//! [`ViolationAction::EscalateToUser`] in that case: the user sees the expected/actual pair and
//! decides. `EscalateToUser` is the safe default because it neither repeats a doomed call nor
//! pretends the violation did not happen.
//!
//! ## What counts as retryable
//!
//! Exactly the two protocol categories where the *same call* has a real chance of succeeding after
//! a short wait: `Transient` (focus jitter, momentary lock) and `TargetNotFound` (the target may
//! appear after a re-poll). `VerifyFailed` is deliberately **not** retryable here even though
//! `ErrorCode::retryable()` says it is: the error category `retryable()` flag answers "may the agent
//! retry the *task*", whereas `retry_once` re-executes the *same tool call*, and re-executing a call
//! that just failed verification is a loop, not a recovery.
//!
//! ## Invariants
//!
//! 1. Parsing is fail-closed: anything outside the five schema values is rejected, never defaulted.
//! 2. Dispatch is total and pure: every (strategy, kind) pair has a defined action.

use assistant_protocol::ErrorCode;
use serde::{Deserialize, Serialize};

use crate::error::{VerifyError, VerifyResult};

/// The five `on_violation` strategies from the tool schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OnViolation {
    /// Retry the same call once (transient failures only).
    RetryOnce,
    /// Retry through the next selector or the next channel (L3 to L4).
    RetryWithAlternative,
    /// Show the user a card with expected versus actual plus evidence.
    EscalateToUser,
    /// Run the rollback recipe (architecture v2 section 9).
    Rollback,
    /// Stop the task and produce a report.
    AbortTask,
}

impl OnViolation {
    /// Every strategy, in schema order. Useful for adapters and for exhaustive tests.
    pub const ALL: [Self; 5] = [
        Self::RetryOnce,
        Self::RetryWithAlternative,
        Self::EscalateToUser,
        Self::Rollback,
        Self::AbortTask,
    ];

    /// The schema string for this strategy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RetryOnce => "retry_once",
            Self::RetryWithAlternative => "retry_with_alternative",
            Self::EscalateToUser => "escalate_to_user",
            Self::Rollback => "rollback",
            Self::AbortTask => "abort_task",
        }
    }
}

/// How a violation should be treated by retry logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ViolationKind {
    /// A temporary condition; the same call may succeed after a short backoff.
    Transient,
    /// The target was not found; a re-poll may find it.
    TargetNotFound,
    /// Anything else: re-issuing the same call would fail the same way.
    Persistent,
}

impl ViolationKind {
    /// Classifies a protocol error code into the retry treatment.
    #[must_use]
    pub const fn from_error_code(error_code: ErrorCode) -> Self {
        match error_code {
            ErrorCode::Transient => Self::Transient,
            ErrorCode::TargetNotFound => Self::TargetNotFound,
            _ => Self::Persistent,
        }
    }

    /// Whether the same call may be retried once for this kind of failure.
    #[must_use]
    pub const fn allows_single_retry(self) -> bool {
        matches!(self, Self::Transient | Self::TargetNotFound)
    }
}

/// The action the Host must take for a violated postcondition.
///
/// Separate from [`OnViolation`] because the decided action can differ from the declared strategy
/// (see the module header on `retry_once`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ViolationAction {
    /// Re-issue the same call once.
    RetryOnce,
    /// Try the next selector or channel.
    RetryWithAlternative,
    /// Ask the user, showing expected versus actual.
    EscalateToUser,
    /// Execute the rollback recipe.
    Rollback,
    /// Abort the task and report.
    AbortTask,
}

/// Parses an `on_violation` string from a tool or step declaration.
///
/// # Errors
///
/// Returns [`VerifyError::UnsupportedOnViolation`] for any value outside the five schema strings.
/// A missing strategy is a caller defect, not a licence to pick a default.
pub fn parse_on_violation(value: &str) -> VerifyResult<OnViolation> {
    match value {
        "retry_once" => Ok(OnViolation::RetryOnce),
        "retry_with_alternative" => Ok(OnViolation::RetryWithAlternative),
        "escalate_to_user" => Ok(OnViolation::EscalateToUser),
        "rollback" => Ok(OnViolation::Rollback),
        "abort_task" => Ok(OnViolation::AbortTask),
        other => Err(VerifyError::UnsupportedOnViolation {
            value: other.to_owned(),
            reason: "the five schema values are retry_once, retry_with_alternative, \
                     escalate_to_user, rollback, abort_task"
                .to_owned(),
        }),
    }
}

/// Decides the action for a violated postcondition.
///
/// `kind` comes from [`ViolationKind::from_error_code`] (or from the caller's own classification
/// when the failure did not come from a protocol error).
#[must_use]
pub const fn dispatch_on_violation(strategy: OnViolation, kind: ViolationKind) -> ViolationAction {
    match strategy {
        OnViolation::RetryOnce => {
            if kind.allows_single_retry() {
                ViolationAction::RetryOnce
            } else {
                // Section 7.4 restricts `retry_once` to transient failures; escalating keeps the
                // declared intent visible to the user instead of looping or hiding the mismatch.
                ViolationAction::EscalateToUser
            }
        }
        OnViolation::RetryWithAlternative => ViolationAction::RetryWithAlternative,
        OnViolation::EscalateToUser => ViolationAction::EscalateToUser,
        OnViolation::Rollback => ViolationAction::Rollback,
        OnViolation::AbortTask => ViolationAction::AbortTask,
    }
}
