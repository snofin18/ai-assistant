//! Structured failures for Core session and context operations.
//!
//! Every public failure maps to a stable `assistant-protocol` error category. Core does not
//! invent protocol categories, and it never turns a failed operation into an empty success.

use std::fmt;

use assistant_model_gateway::ModelGatewayError;
use assistant_protocol::ErrorCode;

/// Result alias for Core operations.
pub type CoreResult<T> = Result<T, CoreError>;

/// A compressor failure reported by an injected compression strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionError {
    reason_code: &'static str,
    error_code: ErrorCode,
    detail: String,
}

impl CompressionError {
    /// Creates a structured compressor failure.
    #[must_use]
    pub fn new(
        reason_code: &'static str,
        error_code: ErrorCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            reason_code,
            error_code,
            detail: detail.into(),
        }
    }

    /// Returns the stable machine-readable reason code.
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        self.reason_code
    }

    /// Returns the protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        self.error_code
    }

    /// Returns the human-readable failure detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for CompressionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "context compression failed ({}): {}",
            self.reason_code, self.detail
        )
    }
}

impl std::error::Error for CompressionError {}

/// A session store failure reported by an injected adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStoreError {
    reason_code: &'static str,
    detail: String,
}

impl SessionStoreError {
    /// Creates a structured session-store failure.
    #[must_use]
    pub fn new(reason_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            reason_code,
            detail: detail.into(),
        }
    }

    /// Returns the stable machine-readable reason code.
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        self.reason_code
    }

    /// Returns the human-readable failure detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for SessionStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "session store failed ({}): {}",
            self.reason_code, self.detail
        )
    }
}

impl std::error::Error for SessionStoreError {}

/// A deterministic session or context failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreError {
    /// An identifier is empty, too long, or contains unsupported characters.
    InvalidIdentifier {
        /// Identifier domain.
        kind: &'static str,
        /// Rejected value.
        value: String,
    },
    /// Message or goal content failed input validation.
    InvalidContent {
        /// Field that failed validation.
        field: &'static str,
        /// Human-readable rejection reason.
        reason: String,
    },
    /// A session id already exists.
    SessionAlreadyExists {
        /// Duplicated session id.
        session_id: String,
    },
    /// The requested session does not exist.
    SessionNotFound {
        /// Missing session id.
        session_id: String,
    },
    /// The session is ended and cannot accept mutations.
    SessionEnded {
        /// Ended session id.
        session_id: String,
    },
    /// A message does not exist in the session.
    MessageNotFound {
        /// Session id.
        session_id: String,
        /// Missing message id.
        message_id: String,
    },
    /// A parent message does not exist in the session.
    ParentMessageNotFound {
        /// Session id.
        session_id: String,
        /// Missing parent message id.
        parent_message_id: String,
    },
    /// A non-leaf message cannot be deleted.
    MessageHasChildren {
        /// Message id that still has children.
        message_id: String,
    },
    /// A restored snapshot violates the session-tree contract.
    InvalidSessionSnapshot {
        /// Human-readable validation failure.
        reason: String,
    },
    /// The context budget is malformed.
    InvalidBudget {
        /// Budget field.
        field: &'static str,
        /// Human-readable rejection reason.
        reason: String,
    },
    /// Required context cannot fit in the supplied budget.
    RequiredContextExceedsBudget {
        /// Tokens required by non-trimmable messages.
        required_tokens: u64,
        /// Tokens available for input.
        available_tokens: u64,
    },
    /// An injected history compressor failed.
    CompressionFailed(CompressionError),
    /// An injected session store failed.
    SessionStore(SessionStoreError),
    /// A checked counter overflowed.
    NumericOverflow {
        /// Counter field.
        field: &'static str,
    },
    /// The injected model provider failed while producing a plan.
    PlannerModel(ModelGatewayError),
    /// The parsed plan failed task-engine structural validation.
    InvalidPlan {
        /// Human-readable validation failure.
        reason: String,
    },
    /// Model output was not a valid planner JSON envelope.
    InvalidPlannerOutput {
        /// Human-readable rejection reason.
        reason: String,
    },
    /// A plan step referenced a tool absent from the supplied catalog.
    UnknownPlannerTool {
        /// Rejecting step.
        step_id: String,
        /// Tool name requested by the model.
        tool: String,
    },
}

impl CoreError {
    /// Maps the failure to the stable protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidIdentifier { .. }
            | Self::InvalidContent { .. }
            | Self::SessionAlreadyExists { .. }
            | Self::InvalidSessionSnapshot { .. }
            | Self::InvalidBudget { .. }
            | Self::MessageHasChildren { .. } => ErrorCode::ToolInvalidArgs,
            Self::SessionNotFound { .. }
            | Self::MessageNotFound { .. }
            | Self::ParentMessageNotFound { .. } => ErrorCode::TargetNotFound,
            Self::RequiredContextExceedsBudget { .. } => ErrorCode::PolicyDenied,
            Self::CompressionFailed(error) => error.error_code(),
            Self::InvalidPlan { .. } | Self::UnknownPlannerTool { .. } => {
                ErrorCode::ToolInvalidArgs
            }
            Self::PlannerModel(error) => error.error_code(),
            Self::InvalidPlannerOutput { .. } => ErrorCode::ModelInvalidOutput,
            Self::SessionEnded { .. } | Self::SessionStore(_) | Self::NumericOverflow { .. } => {
                ErrorCode::Fatal
            }
        }
    }

    /// Returns a stable machine-readable reason code.
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier { .. } => "invalid_identifier",
            Self::InvalidContent { .. } => "invalid_content",
            Self::SessionAlreadyExists { .. } => "session_already_exists",
            Self::SessionNotFound { .. } => "session_not_found",
            Self::SessionEnded { .. } => "session_ended",
            Self::MessageNotFound { .. } => "message_not_found",
            Self::ParentMessageNotFound { .. } => "parent_message_not_found",
            Self::MessageHasChildren { .. } => "message_has_children",
            Self::InvalidSessionSnapshot { .. } => "invalid_session_snapshot",
            Self::InvalidBudget { .. } => "invalid_budget",
            Self::RequiredContextExceedsBudget { .. } => "required_context_exceeds_budget",
            Self::CompressionFailed(error) => error.reason_code(),
            Self::SessionStore(error) => error.reason_code(),
            Self::NumericOverflow { .. } => "numeric_overflow",
            Self::PlannerModel(_) => "planner_model_failure",
            Self::InvalidPlan { .. } => "invalid_plan",
            Self::InvalidPlannerOutput { .. } => "invalid_planner_output",
            Self::UnknownPlannerTool { .. } => "unknown_planner_tool",
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { kind, value } => {
                write!(formatter, "invalid {kind} identifier: {value:?}")
            }
            Self::InvalidContent { field, reason } => {
                write!(formatter, "invalid {field}: {reason}")
            }
            Self::SessionAlreadyExists { session_id } => {
                write!(formatter, "session {session_id} already exists")
            }
            Self::SessionNotFound { session_id } => {
                write!(formatter, "session {session_id} was not found")
            }
            Self::SessionEnded { session_id } => {
                write!(formatter, "session {session_id} has ended")
            }
            Self::MessageNotFound {
                session_id,
                message_id,
            } => write!(
                formatter,
                "message {message_id} was not found in session {session_id}"
            ),
            Self::ParentMessageNotFound {
                session_id,
                parent_message_id,
            } => write!(
                formatter,
                "parent message {parent_message_id} was not found in session {session_id}"
            ),
            Self::MessageHasChildren { message_id } => {
                write!(formatter, "message {message_id} still has children")
            }
            Self::InvalidSessionSnapshot { reason } => {
                write!(formatter, "invalid session snapshot: {reason}")
            }
            Self::InvalidBudget { field, reason } => {
                write!(formatter, "invalid context budget field {field}: {reason}")
            }
            Self::RequiredContextExceedsBudget {
                required_tokens,
                available_tokens,
            } => write!(
                formatter,
                "required context needs {required_tokens} tokens but only {available_tokens} are available"
            ),
            Self::CompressionFailed(error) => error.fmt(formatter),
            Self::SessionStore(error) => error.fmt(formatter),
            Self::NumericOverflow { field } => {
                write!(formatter, "numeric overflow while updating {field}")
            }
            Self::PlannerModel(error) => error.fmt(formatter),
            Self::InvalidPlan { reason } => write!(formatter, "invalid plan: {reason}"),
            Self::InvalidPlannerOutput { reason } => {
                write!(formatter, "invalid planner output: {reason}")
            }
            Self::UnknownPlannerTool { step_id, tool } => {
                write!(formatter, "step {step_id} uses unknown tool {tool:?}")
            }
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompressionFailed(error) => Some(error),
            Self::SessionStore(error) => Some(error),
            Self::PlannerModel(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompressionError> for CoreError {
    fn from(value: CompressionError) -> Self {
        Self::CompressionFailed(value)
    }
}

impl From<SessionStoreError> for CoreError {
    fn from(value: SessionStoreError) -> Self {
        Self::SessionStore(value)
    }
}
