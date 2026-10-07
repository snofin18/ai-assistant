//! Canonical instruction-origin attribution for proposed actions.
//!
//! Responsibility:
//! - define the four architecture v2 section 10.5 origin tokens;
//! - validate origin-specific metadata such as a parent goal or source reference;
//! - expose `app_content` as a high-risk signal for policy and UI consumers.
//!
//! Boundary:
//! - this module does not authorize, deny, persist, audit, or execute an action;
//! - it does not change `PlanStep`, `PolicyDecision`, `ApprovalRequest`, IPC,
//!   or DB schema.
//!
//! Invariants:
//! - unknown origin tokens fail closed;
//! - `plan_derived` requires a non-empty parent goal;
//! - `app_content` and `tool_suggestion` require a non-empty source reference;
//! - `user_request` carries neither parent goal nor source reference.
//!
//! Related documents: architecture v2 sections 10.5 and 12.4, ADR-0082, and
//! `docs/spec/core-orchestration.md`.

use std::fmt;

use assistant_protocol::ErrorCode;

const MAX_PARENT_GOAL_BYTES: usize = 4_096;
const MAX_SOURCE_REF_BYTES: usize = 1_024;

/// Result alias for instruction-origin operations.
pub type OriginResult<T> = Result<T, OriginError>;

/// Canonical origin of an instruction for a proposed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstructionOrigin {
    /// The user explicitly requested the action.
    UserRequest,
    /// The action was derived from the user's request.
    PlanDerived,
    /// The instruction came from content read from an application or document.
    AppContent,
    /// The instruction came from a tool result or suggestion.
    ToolSuggestion,
}

impl InstructionOrigin {
    /// Returns the stable token used by audit, IPC, and UI consumers.
    #[must_use]
    pub const fn stable_token(self) -> &'static str {
        match self {
            Self::UserRequest => "user_request",
            Self::PlanDerived => "plan_derived",
            Self::AppContent => "app_content",
            Self::ToolSuggestion => "tool_suggestion",
        }
    }

    /// Parses one stable origin token.
    ///
    /// # Errors
    ///
    /// Returns [`OriginError::UnknownToken`] for any value outside the four
    /// canonical tokens. This parser never defaults to `user_request`.
    pub fn from_token(token: &str) -> OriginResult<Self> {
        match token {
            "user_request" => Ok(Self::UserRequest),
            "plan_derived" => Ok(Self::PlanDerived),
            "app_content" => Ok(Self::AppContent),
            "tool_suggestion" => Ok(Self::ToolSuggestion),
            _ => Err(OriginError::UnknownToken {
                token: token.to_string(),
            }),
        }
    }

    /// Returns whether this origin is high risk for approval purposes.
    ///
    /// This is only a signal. Policy and HITL remain the only permission
    /// authorities, and the UI still requires an explicit override for
    /// `app_content`.
    #[must_use]
    pub const fn is_high_risk(self) -> bool {
        matches!(self, Self::AppContent)
    }
}

/// Validated attribution metadata for one proposed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionAttribution {
    origin: InstructionOrigin,
    parent_goal: Option<String>,
    source_ref: Option<String>,
}

impl InstructionAttribution {
    /// Creates an attribution from one origin and optional metadata.
    ///
    /// # Errors
    ///
    /// Returns [`OriginError::InvalidAttribution`] when metadata is missing,
    /// forbidden, malformed, or exceeds its byte limit for the selected origin.
    pub fn new(
        origin: InstructionOrigin,
        parent_goal: Option<String>,
        source_ref: Option<String>,
    ) -> OriginResult<Self> {
        let attribution = Self {
            origin,
            parent_goal,
            source_ref,
        };
        attribution.validate()?;
        Ok(attribution)
    }

    /// Creates a `user_request` attribution without extra metadata.
    #[must_use]
    pub const fn user_request() -> Self {
        Self {
            origin: InstructionOrigin::UserRequest,
            parent_goal: None,
            source_ref: None,
        }
    }

    /// Creates a `plan_derived` attribution with its parent goal.
    ///
    /// # Errors
    ///
    /// Returns [`OriginError::InvalidAttribution`] when `parent_goal` is empty,
    /// malformed, or exceeds its byte limit.
    pub fn plan_derived(parent_goal: impl Into<String>) -> OriginResult<Self> {
        Self::new(
            InstructionOrigin::PlanDerived,
            Some(parent_goal.into()),
            None,
        )
    }

    /// Creates an `app_content` attribution with its source reference.
    ///
    /// # Errors
    ///
    /// Returns [`OriginError::InvalidAttribution`] when `source_ref` is empty,
    /// malformed, or exceeds its byte limit.
    pub fn app_content(source_ref: impl Into<String>) -> OriginResult<Self> {
        Self::new(InstructionOrigin::AppContent, None, Some(source_ref.into()))
    }

    /// Creates a `tool_suggestion` attribution with its source reference.
    ///
    /// # Errors
    ///
    /// Returns [`OriginError::InvalidAttribution`] when `source_ref` is empty,
    /// malformed, or exceeds its byte limit.
    pub fn tool_suggestion(source_ref: impl Into<String>) -> OriginResult<Self> {
        Self::new(
            InstructionOrigin::ToolSuggestion,
            None,
            Some(source_ref.into()),
        )
    }

    /// Returns the canonical origin.
    #[must_use]
    pub const fn origin(&self) -> InstructionOrigin {
        self.origin
    }

    /// Returns the parent goal for `plan_derived` attributions.
    #[must_use]
    pub fn parent_goal(&self) -> Option<&str> {
        self.parent_goal.as_deref()
    }

    /// Returns the source reference for `app_content` / `tool_suggestion`.
    #[must_use]
    pub fn source_ref(&self) -> Option<&str> {
        self.source_ref.as_deref()
    }

    /// Returns whether the origin is high risk.
    #[must_use]
    pub const fn is_high_risk(&self) -> bool {
        self.origin.is_high_risk()
    }

    fn validate(&self) -> OriginResult<()> {
        match self.origin {
            InstructionOrigin::UserRequest => {
                if self.parent_goal.is_some() || self.source_ref.is_some() {
                    return Err(OriginError::InvalidAttribution {
                        field: "instruction_attribution",
                        reason: "user_request must not carry parent_goal or source_ref".to_string(),
                    });
                }
            }
            InstructionOrigin::PlanDerived => {
                let parent_goal =
                    self.parent_goal
                        .as_deref()
                        .ok_or_else(|| OriginError::InvalidAttribution {
                            field: "parent_goal",
                            reason: "plan_derived requires a parent_goal".to_string(),
                        })?;
                validate_text("parent_goal", parent_goal, MAX_PARENT_GOAL_BYTES)?;
                if self.source_ref.is_some() {
                    return Err(OriginError::InvalidAttribution {
                        field: "source_ref",
                        reason: "plan_derived must not carry source_ref".to_string(),
                    });
                }
            }
            InstructionOrigin::AppContent | InstructionOrigin::ToolSuggestion => {
                let source_ref =
                    self.source_ref
                        .as_deref()
                        .ok_or_else(|| OriginError::InvalidAttribution {
                            field: "source_ref",
                            reason: format!("{} requires a source_ref", self.origin.stable_token()),
                        })?;
                validate_text("source_ref", source_ref, MAX_SOURCE_REF_BYTES)?;
                if self.parent_goal.is_some() {
                    return Err(OriginError::InvalidAttribution {
                        field: "parent_goal",
                        reason: format!(
                            "{} must not carry parent_goal",
                            self.origin.stable_token()
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Typed instruction-origin failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OriginError {
    /// The caller supplied a token outside the canonical origin set.
    UnknownToken {
        /// Rejected token.
        token: String,
    },
    /// The attribution metadata is missing, forbidden, or malformed.
    InvalidAttribution {
        /// Rejected field.
        field: &'static str,
        /// Human-readable rejection reason.
        reason: String,
    },
}

impl OriginError {
    /// Maps the failure to the stable protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        ErrorCode::ToolInvalidArgs
    }

    /// Returns a stable machine-readable reason code.
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::UnknownToken { .. } => "origin_unknown_token",
            Self::InvalidAttribution { .. } => "origin_invalid_attribution",
        }
    }
}

impl fmt::Display for OriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownToken { token } => {
                write!(formatter, "unknown instruction origin token: {token:?}")
            }
            Self::InvalidAttribution { field, reason } => {
                write!(
                    formatter,
                    "invalid instruction attribution {field}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for OriginError {}

fn validate_text(field: &'static str, value: &str, maximum: usize) -> OriginResult<()> {
    if value.trim().is_empty() {
        return Err(OriginError::InvalidAttribution {
            field,
            reason: "must not be empty".to_string(),
        });
    }
    if value.len() > maximum {
        return Err(OriginError::InvalidAttribution {
            field,
            reason: format!("must not exceed {maximum} bytes"),
        });
    }
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(OriginError::InvalidAttribution {
            field,
            reason: "contains unsupported control characters".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::{InstructionAttribution, InstructionOrigin, OriginError};
    use assistant_protocol::ErrorCode;

    #[test]
    fn test_instruction_origin_tokens_round_trip() {
        for (origin, token) in [
            (InstructionOrigin::UserRequest, "user_request"),
            (InstructionOrigin::PlanDerived, "plan_derived"),
            (InstructionOrigin::AppContent, "app_content"),
            (InstructionOrigin::ToolSuggestion, "tool_suggestion"),
        ] {
            assert_eq!(origin.stable_token(), token);
            assert_eq!(
                InstructionOrigin::from_token(token).expect("known token"),
                origin
            );
        }
    }

    #[test]
    fn test_instruction_origin_unknown_token_fails_closed() {
        let error = InstructionOrigin::from_token("model_invented").expect_err("unknown token");
        assert!(matches!(error, OriginError::UnknownToken { .. }));
        assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
        assert_eq!(error.reason_code(), "origin_unknown_token");
    }

    #[test]
    fn test_instruction_attribution_plan_derived_requires_parent_goal() {
        let error = InstructionAttribution::new(InstructionOrigin::PlanDerived, None, None)
            .expect_err("missing parent goal");
        assert!(matches!(
            error,
            OriginError::InvalidAttribution {
                field: "parent_goal",
                ..
            }
        ));
        let attribution =
            InstructionAttribution::plan_derived("Replace the report title").expect("attribution");
        assert_eq!(attribution.parent_goal(), Some("Replace the report title"));
        assert!(!attribution.is_high_risk());
    }

    #[test]
    fn test_instruction_attribution_app_content_requires_source_and_is_high_risk() {
        let error = InstructionAttribution::new(InstructionOrigin::AppContent, None, None)
            .expect_err("missing source");
        assert!(matches!(
            error,
            OriginError::InvalidAttribution {
                field: "source_ref",
                ..
            }
        ));
        let attribution =
            InstructionAttribution::app_content("evidence://doc/1").expect("attribution");
        assert!(attribution.is_high_risk());
        assert_eq!(attribution.source_ref(), Some("evidence://doc/1"));
    }

    #[test]
    fn test_instruction_attribution_tool_suggestion_requires_source_ref() {
        let error = InstructionAttribution::new(InstructionOrigin::ToolSuggestion, None, None)
            .expect_err("missing source");
        assert!(matches!(
            error,
            OriginError::InvalidAttribution {
                field: "source_ref",
                ..
            }
        ));
        let attribution = InstructionAttribution::tool_suggestion("tool://editor/read_text")
            .expect("attribution");
        assert_eq!(attribution.source_ref(), Some("tool://editor/read_text"));
    }

    #[test]
    fn test_instruction_attribution_user_request_rejects_metadata() {
        let error = InstructionAttribution::new(
            InstructionOrigin::UserRequest,
            Some("parent".to_string()),
            None,
        )
        .expect_err("metadata rejected");
        assert!(matches!(
            error,
            OriginError::InvalidAttribution {
                field: "instruction_attribution",
                ..
            }
        ));
        assert_eq!(
            InstructionAttribution::user_request().origin(),
            InstructionOrigin::UserRequest
        );
    }

    #[test]
    fn test_instruction_attribution_rejects_invalid_text() {
        let error =
            InstructionAttribution::plan_derived("bad\u{0000}goal").expect_err("control rejected");
        assert!(matches!(
            error,
            OriginError::InvalidAttribution {
                field: "parent_goal",
                ..
            }
        ));
        assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
        assert_eq!(error.reason_code(), "origin_invalid_attribution");
    }
}
