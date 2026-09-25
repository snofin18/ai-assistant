//! Structured errors for task-engine commands and checkpoint access.

use assistant_protocol::ErrorCode;
use thiserror::Error;

use crate::checkpoint::CheckpointStoreError;
use crate::status::{StepEvent, StepStatus, TaskEvent, TaskStatus};

/// Result alias for task-engine operations.
pub type TaskEngineResult<T> = Result<T, TaskEngineError>;

/// A deterministic task-engine failure.
///
/// Every variant maps to a stable protocol error category. Callers must not
/// treat an error as an implicit success or infer a task outcome from it.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TaskEngineError {
    /// An identifier failed syntax or size validation.
    #[error("invalid {kind} identifier: {value:?}")]
    InvalidIdentifier {
        /// Identifier domain (`task`, `plan`, or `step`).
        kind: &'static str,
        /// Rejected value.
        value: String,
    },

    /// The plan failed structural validation.
    #[error("invalid plan: {reason}")]
    InvalidPlan {
        /// Human-readable validation failure.
        reason: String,
    },

    /// Two plan steps use the same identifier.
    #[error("duplicate step id: {step_id}")]
    DuplicateStepId {
        /// Duplicated identifier.
        step_id: String,
    },

    /// Two plan steps use the same sequence number.
    #[error("duplicate step sequence: {sequence}")]
    DuplicateStepSequence {
        /// Duplicated sequence.
        sequence: u32,
    },

    /// A dependency references no step in the same plan.
    #[error("step {step_id} depends on missing step {dependency_id}")]
    MissingDependency {
        /// Dependent step.
        step_id: String,
        /// Missing dependency.
        dependency_id: String,
    },

    /// The dependency graph contains a cycle.
    #[error("plan step dependency graph contains a cycle")]
    DependencyCycle,

    /// A write step did not declare a postcondition.
    #[error("write step {step_id} must declare at least one postcondition")]
    MissingPostconditions {
        /// Invalid step.
        step_id: String,
    },

    /// A tool name does not use the required `<app>.<domain>.<action>` shape.
    #[error("invalid tool name {tool:?} for step {step_id}")]
    InvalidToolName {
        /// Invalid step.
        step_id: String,
        /// Rejected tool name.
        tool: String,
    },

    /// A task event is not legal from the current state.
    #[error("invalid task transition: {from:?} + {event:?}")]
    InvalidTaskTransition {
        /// Current state.
        from: TaskStatus,
        /// Rejected event.
        event: TaskEvent,
    },

    /// A step event is not legal from the current state.
    #[error("invalid step transition: {from:?} + {event:?}")]
    InvalidStepTransition {
        /// Current state.
        from: StepStatus,
        /// Rejected event.
        event: StepEvent,
    },

    /// A step identifier was not present in the loaded task snapshot.
    #[error("step not found in task {task_id}: {step_id}")]
    StepNotFound {
        /// Task identifier.
        task_id: String,
        /// Missing step.
        step_id: String,
    },

    /// No checkpoint exists for the requested task.
    #[error("no checkpoint exists for task {task_id}")]
    TaskNotFound {
        /// Missing task.
        task_id: String,
    },

    /// The injected clock moved backwards.
    #[error("clock moved backwards: previous {previous_ms}, current {current_ms}")]
    ClockWentBackwards {
        /// Previously persisted timestamp.
        previous_ms: i64,
        /// Rejected newer timestamp.
        current_ms: i64,
    },

    /// A counter would overflow.
    #[error("numeric overflow while updating {field}")]
    NumericOverflow {
        /// Counter field.
        field: &'static str,
    },

    /// A checkpoint is corrupt or incompatible.
    #[error("invalid checkpoint: {reason}")]
    InvalidCheckpoint {
        /// Parse or compatibility failure.
        reason: String,
    },

    /// The persisted store rejected a read or write.
    #[error(transparent)]
    CheckpointStore(#[from] CheckpointStoreError),
}

impl TaskEngineError {
    /// Maps the failure to the stable protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidIdentifier { .. }
            | Self::InvalidPlan { .. }
            | Self::DuplicateStepId { .. }
            | Self::DuplicateStepSequence { .. }
            | Self::MissingDependency { .. }
            | Self::DependencyCycle
            | Self::MissingPostconditions { .. }
            | Self::InvalidToolName { .. }
            | Self::StepNotFound { .. } => ErrorCode::ToolInvalidArgs,
            Self::TaskNotFound { .. } => ErrorCode::TargetNotFound,
            Self::InvalidTaskTransition { .. }
            | Self::InvalidStepTransition { .. }
            | Self::ClockWentBackwards { .. }
            | Self::NumericOverflow { .. }
            | Self::InvalidCheckpoint { .. } => ErrorCode::Fatal,
            Self::CheckpointStore(error) => error.error_code(),
        }
    }
}
