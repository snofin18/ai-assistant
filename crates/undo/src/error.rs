//! Error type for rollback construction, execution, conflict handling, and incident delivery.
//!
//! Responsibility: map every failure to one of the existing protocol [`ErrorCode`] categories
//! without inventing a new protocol code.
//!
//! Boundary: no logging, transport, retry, or platform calls.

use std::fmt;

use assistant_platform_api::ErrorCode;

use crate::anchor::AnchorStrategy;
use crate::id::AnchorId;
use crate::reversibility::Reversibility;

/// Errors raised while validating or preparing rollback behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UndoError {
    /// A newtype identifier was empty or contained NUL.
    InvalidIdentifier {
        /// Identifier kind, such as `anchor_id`.
        kind: &'static str,
        /// Why the value was rejected.
        reason: String,
    },
    /// A content digest was not 64 lowercase hexadecimal characters.
    InvalidContentDigest {
        /// Why the digest was rejected.
        reason: String,
    },
    /// A shadow-copy path was empty, contained NUL, or traversed upward.
    InvalidShadowCopyPath {
        /// Why the path was rejected.
        reason: String,
    },
    /// An undo-step count fell outside 1 through 50.
    InvalidUndoStepCount {
        /// The rejected count.
        value: u8,
    },
    /// A tool name did not have the `<app>.<domain>.<action>` shape.
    InvalidToolName {
        /// The rejected tool name.
        value: String,
    },
    /// A compensating action did not contain valid, non-empty argument text.
    InvalidCompensatingArguments {
        /// Why the arguments were rejected.
        reason: String,
    },
    /// A rollback recipe contained no actions.
    EmptyRollbackRecipe {
        /// Anchor that owned the empty recipe.
        anchor_id: AnchorId,
    },
    /// A rollback recipe exceeded the fixed action limit.
    RollbackRecipeTooLong {
        /// Number of declared actions.
        count: usize,
        /// Maximum accepted action count.
        maximum: usize,
    },
    /// A compensating recipe belonged to a different anchor.
    RecipeAnchorMismatch {
        /// Anchor requested by the caller.
        anchor_id: AnchorId,
        /// Anchor embedded in the recipe.
        recipe_anchor_id: AnchorId,
    },
    /// The anchor payload did not match the declared reversibility level.
    AnchorKindMismatch {
        /// Declared reversibility.
        reversibility: Reversibility,
        /// Payload strategy that was supplied.
        strategy: AnchorStrategy,
    },
    /// A required capability or prerequisite was not available.
    CapabilityMissing {
        /// Missing prerequisite.
        capability: &'static str,
        /// Why it is required.
        context: String,
    },
    /// An irreversible step was asked to produce or execute a rollback recipe.
    IrreversibleStep {
        /// Step that cannot be rolled back.
        step_id: String,
    },
    /// A post-step fingerprint was already recorded.
    PostFingerprintAlreadyRecorded {
        /// Anchor whose post-step state was already recorded.
        anchor_id: AnchorId,
    },
    /// Incident delivery failed.
    IncidentDeliveryFailed {
        /// Incident that could not be delivered.
        incident_id: String,
        /// Error category returned by the reporter.
        error_code: ErrorCode,
        /// Human-readable reporter failure.
        reason: String,
    },
}

impl UndoError {
    /// Maps this error to an existing protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::CapabilityMissing { .. } => ErrorCode::CapabilityMissing,
            Self::IncidentDeliveryFailed { .. } => ErrorCode::Fatal,
            Self::InvalidIdentifier { .. }
            | Self::InvalidContentDigest { .. }
            | Self::InvalidShadowCopyPath { .. }
            | Self::InvalidUndoStepCount { .. }
            | Self::InvalidToolName { .. }
            | Self::InvalidCompensatingArguments { .. }
            | Self::EmptyRollbackRecipe { .. }
            | Self::RollbackRecipeTooLong { .. }
            | Self::RecipeAnchorMismatch { .. }
            | Self::AnchorKindMismatch { .. }
            | Self::IrreversibleStep { .. }
            | Self::PostFingerprintAlreadyRecorded { .. } => ErrorCode::ToolInvalidArgs,
        }
    }
}

impl fmt::Display for UndoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&describe_error(self))
    }
}

fn describe_error(error: &UndoError) -> String {
    match error {
        UndoError::InvalidIdentifier { kind, reason } => format!("invalid {kind}: {reason}"),
        UndoError::InvalidContentDigest { reason } => {
            format!("invalid content digest: {reason}")
        }
        UndoError::InvalidShadowCopyPath { reason } => {
            format!("invalid shadow-copy path: {reason}")
        }
        UndoError::InvalidUndoStepCount { value } => {
            format!("undo step count {value} is outside 1..=50")
        }
        UndoError::InvalidToolName { value } => {
            format!("tool name `{value}` is not <app>.<domain>.<action>")
        }
        UndoError::InvalidCompensatingArguments { reason } => {
            format!("invalid compensating-action arguments: {reason}")
        }
        UndoError::EmptyRollbackRecipe { anchor_id } => {
            format!(
                "rollback recipe for anchor `{}` is empty",
                anchor_id.as_str()
            )
        }
        UndoError::RollbackRecipeTooLong { count, maximum } => {
            format!("rollback recipe has {count} actions; maximum is {maximum}")
        }
        UndoError::RecipeAnchorMismatch {
            anchor_id,
            recipe_anchor_id,
        } => format!(
            "recipe anchor `{}` does not match requested anchor `{}`",
            recipe_anchor_id.as_str(),
            anchor_id.as_str()
        ),
        UndoError::AnchorKindMismatch {
            reversibility,
            strategy,
        } => format!(
            "anchor strategy `{}` is not valid for reversibility `{}`",
            strategy.as_str(),
            reversibility.as_str()
        ),
        UndoError::CapabilityMissing {
            capability,
            context,
        } => {
            format!("missing rollback prerequisite `{capability}`: {context}")
        }
        UndoError::IrreversibleStep { step_id } => {
            format!("step `{step_id}` is irreversible and has no rollback recipe")
        }
        UndoError::PostFingerprintAlreadyRecorded { anchor_id } => format!(
            "post-step fingerprint is already recorded for anchor `{}`",
            anchor_id.as_str()
        ),
        UndoError::IncidentDeliveryFailed {
            incident_id,
            error_code,
            reason,
        } => format!("failed to deliver incident `{incident_id}` as {error_code:?}: {reason}"),
    }
}

impl std::error::Error for UndoError {}

/// Result alias for the undo layer.
pub type UndoResult<T> = Result<T, UndoError>;
