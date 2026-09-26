//! Structured errors for the HITL domain.
//!
//! Every public failure maps to an existing `assistant_protocol::ErrorCode`.
//! This crate deliberately does not add protocol error categories.

use assistant_protocol::ErrorCode;
use assistant_task_engine::TaskEngineError;
use thiserror::Error;

/// Result type used by all fallible HITL APIs.
pub type HitlResult<T> = Result<T, HitlError>;

/// Structured HITL failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HitlError {
    /// An identifier is empty, too long, or contains control characters.
    #[error("{kind} identifier is invalid: {reason}")]
    InvalidIdentifier {
        /// Identifier kind.
        kind: &'static str,
        /// Validation failure.
        reason: String,
    },
    /// A tool name does not use `<app>.<domain>.<action>`.
    #[error("tool name is invalid: {tool}")]
    InvalidToolName {
        /// Rejected tool name.
        tool: String,
    },
    /// An approval request or grant is structurally invalid.
    #[error("invalid HITL value: {reason}")]
    InvalidValue {
        /// Human-readable validation failure.
        reason: String,
    },
    /// The policy decision does not require approval.
    #[error("policy decision does not require approval")]
    ApprovalNotRequired,
    /// The policy denied the action and HITL cannot convert it to approval.
    #[error("policy denied the action; approval cannot override a deny")]
    PolicyDenied,
    /// The policy confirmation omitted `show_diff` or supplied an invalid value.
    #[error("policy confirmation is missing required scope or show_diff fields")]
    InvalidPolicyConfirmation,
    /// A scope unsupported for the request risk was supplied.
    #[error("authorization scope '{scope}' is forbidden for this risk level")]
    ForbiddenAuthorizationScope {
        /// Forbidden scope.
        scope: String,
    },
    /// The approval response selected a scope not offered by the request.
    #[error("selected authorization scope was not offered by the approval request")]
    ScopeNotOffered,
    /// The approval or grant expired.
    #[error("approval expired at {expires_at_ms} ms; current time is {now_ms} ms")]
    ApprovalExpired {
        /// Expiry timestamp.
        expires_at_ms: i64,
        /// Evaluation timestamp.
        now_ms: i64,
    },
    /// A grant does not cover the proposed action.
    #[error("authorization grant does not cover the action: {reason}")]
    AuthorizationDenied {
        /// Deterministic denial reason.
        reason: String,
    },
    /// An injected timestamp moved backwards.
    #[error("clock moved backwards: previous={previous_ms} ms current={now_ms} ms")]
    ClockWentBackwards {
        /// Previous timestamp.
        previous_ms: i64,
        /// Current timestamp.
        now_ms: i64,
    },
    /// A takeover operation was requested without an active takeover.
    #[error("task {task_id} does not have an active takeover")]
    TakeoverNotActive {
        /// Task identifier.
        task_id: String,
    },
    /// A takeover operation was requested while another takeover was active.
    #[error("task {task_id} already has an active takeover")]
    TakeoverAlreadyActive {
        /// Task identifier.
        task_id: String,
    },
    /// A diff exceeds the caller-provided cell budget.
    #[error("diff needs {required_cells} cells but the limit is {max_cells}")]
    DiffBudgetExceeded {
        /// Required dynamic-programming cells.
        required_cells: usize,
        /// Caller-provided maximum.
        max_cells: usize,
    },
    /// The task engine rejected a transition or checkpoint operation.
    #[error(transparent)]
    TaskEngine(#[from] TaskEngineError),
}

impl HitlError {
    /// Maps the error to the stable protocol error taxonomy.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidIdentifier { .. }
            | Self::InvalidToolName { .. }
            | Self::InvalidValue { .. }
            | Self::InvalidPolicyConfirmation
            | Self::DiffBudgetExceeded { .. }
            | Self::ApprovalNotRequired => ErrorCode::ToolInvalidArgs,
            Self::PolicyDenied | Self::ForbiddenAuthorizationScope { .. } => {
                ErrorCode::PolicyDenied
            }
            Self::ScopeNotOffered
            | Self::AuthorizationDenied { .. }
            | Self::ApprovalExpired { .. }
            | Self::TakeoverNotActive { .. }
            | Self::TakeoverAlreadyActive { .. } => ErrorCode::UserInteraction,
            Self::ClockWentBackwards { .. } => ErrorCode::Transient,
            Self::TaskEngine(error) => error.error_code(),
        }
    }
}
