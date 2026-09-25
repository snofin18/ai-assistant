//! Policy errors and their stable protocol error-code mapping.

use assistant_protocol::ErrorCode;
use std::fmt;

/// Result alias for policy operations.
pub type PolicyResult<T> = Result<T, PolicyError>;

/// A fail-closed policy failure.
///
/// Evaluation callers must treat this error as a denial; it must never be
/// converted into an implicit allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// The rule set is malformed, empty in a disallowed field, or unsafe to use.
    InvalidRuleSet {
        /// Human-readable explanation of the invalid rule set.
        reason: String,
    },
    /// Two rules use the same rule id.
    RuleSetConflict {
        /// The duplicated rule id.
        rule_id: String,
    },
    /// A path failed lexical, containment, or resolved-containment checks.
    PathRejected {
        /// Human-readable explanation of the rejected path.
        reason: String,
    },
    /// A URL failed scheme, authority, host, or address checks.
    UrlRejected {
        /// Human-readable explanation of the rejected URL.
        reason: String,
    },
    /// Text failed size, line-count, or control-character checks.
    TextRejected {
        /// Human-readable explanation of the rejected text.
        reason: String,
    },
    /// An integer fell outside its declared inclusive bounds.
    IntegerOutOfRange {
        /// Rejected value.
        value: i64,
        /// Inclusive minimum.
        minimum: i64,
        /// Inclusive maximum.
        maximum: i64,
    },
    /// A regular expression uses unsupported or potentially exponential syntax.
    RegexRejected {
        /// Human-readable explanation of the rejected pattern.
        reason: String,
    },
    /// A rich policy decision could not be projected into the protocol shape.
    ProtocolProjection {
        /// Human-readable projection failure.
        reason: String,
    },
}

impl PolicyError {
    /// Returns the stable protocol error category for this failure.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidRuleSet { .. }
            | Self::RuleSetConflict { .. }
            | Self::PathRejected { .. }
            | Self::UrlRejected { .. }
            | Self::TextRejected { .. }
            | Self::IntegerOutOfRange { .. }
            | Self::RegexRejected { .. } => ErrorCode::ToolInvalidArgs,
            Self::ProtocolProjection { .. } => ErrorCode::Fatal,
        }
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuleSet { reason } => {
                write!(formatter, "invalid policy rule set: {reason}")
            }
            Self::RuleSetConflict { rule_id } => {
                write!(formatter, "duplicate policy rule id: {rule_id}")
            }
            Self::PathRejected { reason } => write!(formatter, "path rejected: {reason}"),
            Self::UrlRejected { reason } => write!(formatter, "URL rejected: {reason}"),
            Self::TextRejected { reason } => write!(formatter, "text rejected: {reason}"),
            Self::IntegerOutOfRange {
                value,
                minimum,
                maximum,
            } => write!(
                formatter,
                "integer {value} is outside inclusive range {minimum}..={maximum}"
            ),
            Self::RegexRejected { reason } => {
                write!(formatter, "regular expression rejected: {reason}")
            }
            Self::ProtocolProjection { reason } => {
                write!(formatter, "protocol projection failed: {reason}")
            }
        }
    }
}

impl std::error::Error for PolicyError {}
