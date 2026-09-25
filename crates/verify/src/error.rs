//! Error type for the verification layer.
//!
//! Responsibility: separate "the postcondition itself is malformed" (a caller defect,
//! reported as `ToolInvalidArgs`) from "the postcondition was not satisfied" (a verification
//! result, reported as `VerifyFailed`). The two must not be conflated: a malformed
//! postcondition must never look like a satisfied one, and a falsified one must never look
//! like a caller bug.
//!
//! Boundary: no IO, no logging, no retry. Every variant carries a readable reason
//! (invariant 1: no silent failure).

use assistant_protocol::ErrorCode;
use thiserror::Error;

/// Errors raised while parsing postconditions or fingerprint material.
///
/// Every variant maps to a protocol [`ErrorCode`] via [`VerifyError::error_code`], so a
/// caller can surface a structured failure instead of a string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum VerifyError {
    /// The postcondition JSON value is not an object.
    #[error("postcondition #{index} must be a JSON object, got {actual}")]
    PostconditionNotAnObject {
        /// Zero-based position of the postcondition inside the step.
        index: usize,
        /// The JSON type actually seen (`array`, `string`, ...).
        actual: String,
    },

    /// A required field is missing, has the wrong type, or is out of range.
    #[error("postcondition #{index} is malformed: {reason}")]
    MalformedPostcondition {
        /// Zero-based position of the postcondition inside the step.
        index: usize,
        /// What exactly is wrong, in a form a tool author can act on.
        reason: String,
    },

    /// The postcondition declares a `kind` this crate does not implement.
    ///
    /// This is fail-closed on purpose: a kind we cannot evaluate must never be silently
    /// skipped, because skipping it would turn a real constraint into decoration.
    #[error("postcondition #{index} declares unsupported kind `{kind}`: {reason}")]
    UnsupportedPostconditionKind {
        /// Zero-based position of the postcondition inside the step.
        index: usize,
        /// The `kind` string as written by the tool author.
        kind: String,
        /// Why it is unsupported, and what to use instead.
        reason: String,
    },

    /// The step declares an `on_violation` strategy outside the five schema values.
    ///
    /// Fail-closed for the same reason as an unknown postcondition kind: picking a default would
    /// silently substitute our policy for the tool author's.
    #[error("unsupported on_violation strategy `{value}`: {reason}")]
    UnsupportedOnViolation {
        /// The strategy string as written by the tool author.
        value: String,
        /// Why it is unsupported, and what the five valid values are.
        reason: String,
    },

    /// A fingerprint string is not in the canonical `sha256:<64 lowercase hex>` form.
    #[error("invalid fingerprint: {reason}")]
    InvalidFingerprint {
        /// Why the fingerprint was rejected.
        reason: String,
    },
}

impl VerifyError {
    /// The protocol error category this error belongs to.
    ///
    /// Malformed postconditions and unsupported `on_violation` strategies are caller defects
    /// (`ToolInvalidArgs`); fingerprint problems are verification failures (`VerifyFailed`).
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::PostconditionNotAnObject { .. }
            | Self::MalformedPostcondition { .. }
            | Self::UnsupportedPostconditionKind { .. }
            | Self::UnsupportedOnViolation { .. } => ErrorCode::ToolInvalidArgs,
            Self::InvalidFingerprint { .. } => ErrorCode::VerifyFailed,
        }
    }
}

/// Result alias for the verification layer.
pub type VerifyResult<T> = Result<T, VerifyError>;

/// Builds a [`VerifyError::MalformedPostcondition`].
///
/// Shared by this module's callers so every "field is wrong" rejection has the same shape.
pub fn malformed(index: usize, reason: impl Into<String>) -> VerifyError {
    VerifyError::MalformedPostcondition {
        index,
        reason: reason.into(),
    }
}

/// Builds a [`VerifyError::UnsupportedPostconditionKind`].
pub fn unsupported(index: usize, kind: &str, reason: &str) -> VerifyError {
    VerifyError::UnsupportedPostconditionKind {
        index,
        kind: kind.to_owned(),
        reason: reason.to_owned(),
    }
}
