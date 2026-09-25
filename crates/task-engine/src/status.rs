//! Task and step states plus their explicit legal transitions.
//!
//! The engine deliberately uses one closed transition table per level.
//! Recovery-only transitions are still explicit events; there is no wildcard
//! fallback and no state coercion.

use serde::{Deserialize, Serialize};

use crate::error::{TaskEngineError, TaskEngineResult};

/// The 12 task-level states from architecture v2 section 8.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskStatus {
    /// The plan is being generated or validated.
    Draft,
    /// The plan is ready for explicit user approval.
    AwaitingApproval,
    /// The task is executing.
    Running,
    /// Execution is paused at a step boundary.
    Paused,
    /// A user has taken control of the target application.
    TakenOver,
    /// The engine is recovering from a crash or restart.
    Resuming,
    /// The user cancelled the task.
    Cancelled,
    /// Every step completed successfully.
    Completed,
    /// The task completed with explicit warnings.
    CompletedWithWarnings,
    /// The task failed and is no longer progressing.
    Failed,
    /// An external condition blocks progress.
    Blocked,
    /// Automation cannot safely decide and requires a human.
    NeedsHuman,
}

impl TaskStatus {
    /// All 12 task states in canonical order.
    pub const ALL: [Self; 12] = [
        Self::Draft,
        Self::AwaitingApproval,
        Self::Running,
        Self::Paused,
        Self::TakenOver,
        Self::Resuming,
        Self::Cancelled,
        Self::Completed,
        Self::CompletedWithWarnings,
        Self::Failed,
        Self::Blocked,
        Self::NeedsHuman,
    ];

    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::TakenOver => "taken_over",
            Self::Resuming => "resuming",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
            Self::CompletedWithWarnings => "completed_with_warnings",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::NeedsHuman => "needs_human",
        }
    }

    /// Returns whether no further task transition is expected.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Completed | Self::CompletedWithWarnings | Self::Failed
        )
    }
}

/// Events that can change a task-level state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskEvent {
    /// Submit a draft plan for approval.
    SubmitForApproval,
    /// Approve the current draft plan.
    ApprovePlan,
    /// Pause at the next safe step boundary.
    Pause,
    /// Resume after an explicit pause.
    Resume,
    /// Resume after an external blocker was cleared.
    Unblock,
    /// Mark that a user took control.
    TakeOver,
    /// Return control to the agent and require recovery.
    ReturnControl,
    /// Begin crash/restart recovery.
    StartRecovery,
    /// Recovery finished and normal execution may continue.
    FinishRecovery,
    /// Cancel remaining work.
    Cancel,
    /// Complete the task successfully.
    Complete,
    /// Complete the task while retaining warnings.
    CompleteWithWarnings,
    /// Fail the task.
    Fail,
    /// Block the task on an external condition.
    Block,
    /// Escalate to a human decision.
    RequireHuman,
}

/// Step-level states from architecture v2 section 8.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StepStatus {
    /// Waiting for dependencies.
    Pending,
    /// Resolving target and preconditions.
    Prechecking,
    /// Approved to execute.
    Approved,
    /// Executing the tool.
    Executing,
    /// Running postconditions and fingerprint checks.
    Verifying,
    /// The verified step is committed.
    Committed,
    /// Policy denied the step.
    PolicyDenied,
    /// The step failed.
    Failed,
    /// A retry is scheduled.
    Retrying,
    /// A rollback completed.
    RolledBack,
    /// The step was deliberately skipped.
    Skipped,
}

impl StepStatus {
    /// All step states in canonical order.
    pub const ALL: [Self; 11] = [
        Self::Pending,
        Self::Prechecking,
        Self::Approved,
        Self::Executing,
        Self::Verifying,
        Self::Committed,
        Self::PolicyDenied,
        Self::Failed,
        Self::Retrying,
        Self::RolledBack,
        Self::Skipped,
    ];

    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Prechecking => "prechecking",
            Self::Approved => "approved",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::Committed => "committed",
            Self::PolicyDenied => "policy_denied",
            Self::Failed => "failed",
            Self::Retrying => "retrying",
            Self::RolledBack => "rolled_back",
            Self::Skipped => "skipped",
        }
    }

    /// Returns whether the state satisfies a DAG dependency.
    #[must_use]
    pub const fn satisfies_dependency(self) -> bool {
        matches!(self, Self::Committed | Self::Skipped)
    }

    /// Returns whether the state prevents automatic execution.
    #[must_use]
    pub const fn blocks_progress(self) -> bool {
        matches!(self, Self::PolicyDenied | Self::Failed | Self::RolledBack)
    }
}

/// Events that can change a step-level state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StepEvent {
    /// Begin target resolution and prechecks.
    BeginPrecheck,
    /// Approve the step for execution.
    Approve,
    /// Start tool execution.
    BeginExecute,
    /// Start postcondition verification.
    BeginVerify,
    /// Commit a verified step.
    Commit,
    /// Deny the step by policy.
    DenyPolicy,
    /// Fail the step.
    Fail,
    /// Schedule a retry.
    ScheduleRetry,
    /// Move from retry scheduling back to pending.
    BeginRetry,
    /// Roll back the step.
    Rollback,
    /// Skip the step.
    Skip,
    /// Recovery proved the step completed.
    RecoverCompleted,
    /// Recovery proved the step did not complete and must be redone.
    RecoverNotCompleted,
    /// Recovery proved the step never crossed the execution boundary.
    RecoverSafeReset,
}

/// The currently watched execution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StepPhase {
    /// Target resolution and prechecks.
    Resolve,
    /// Tool execution.
    Execute,
    /// Postcondition and fingerprint verification.
    Verify,
}

/// Computes the next task state for an event.
///
/// # Errors
///
/// Returns [`TaskEngineError::InvalidTaskTransition`] when the event is not
/// legal from `current`.
pub fn transition_task(current: TaskStatus, event: TaskEvent) -> TaskEngineResult<TaskStatus> {
    use TaskEvent as Event;
    use TaskStatus as State;

    if event == Event::Cancel && !current.is_terminal() {
        return Ok(State::Cancelled);
    }
    if event == Event::Fail && !current.is_terminal() {
        return Ok(State::Failed);
    }
    if event == Event::RequireHuman
        && matches!(
            current,
            State::Draft
                | State::AwaitingApproval
                | State::Running
                | State::Paused
                | State::TakenOver
                | State::Resuming
                | State::Blocked
        )
    {
        return Ok(State::NeedsHuman);
    }
    if event == Event::Block
        && matches!(
            current,
            State::AwaitingApproval
                | State::Running
                | State::Paused
                | State::TakenOver
                | State::Resuming
        )
    {
        return Ok(State::Blocked);
    }
    if event == Event::Complete && matches!(current, State::Running | State::Resuming) {
        return Ok(State::Completed);
    }
    if event == Event::CompleteWithWarnings && matches!(current, State::Running | State::Resuming) {
        return Ok(State::CompletedWithWarnings);
    }

    let next = match (current, event) {
        (State::Draft, Event::SubmitForApproval) => State::AwaitingApproval,
        (State::AwaitingApproval, Event::ApprovePlan)
        | (State::Paused, Event::Resume)
        | (State::Resuming, Event::FinishRecovery) => State::Running,
        (State::Running, Event::Pause) => State::Paused,
        (State::Running | State::Paused, Event::TakeOver) => State::TakenOver,
        (State::Running, Event::StartRecovery)
        | (State::TakenOver, Event::ReturnControl)
        | (State::Blocked, Event::Unblock)
        | (State::NeedsHuman, Event::Resume) => State::Resuming,
        (_, _) => {
            return Err(TaskEngineError::InvalidTaskTransition {
                from: current,
                event,
            });
        }
    };
    Ok(next)
}

/// Computes the next step state for an event.
///
/// # Errors
///
/// Returns [`TaskEngineError::InvalidStepTransition`] when the event is not
/// legal from `current`.
pub fn transition_step(current: StepStatus, event: StepEvent) -> TaskEngineResult<StepStatus> {
    use StepEvent as Event;
    use StepStatus as State;

    if event == Event::Fail
        && matches!(
            current,
            State::Prechecking | State::Approved | State::Executing | State::Verifying
        )
    {
        return Ok(State::Failed);
    }
    if event == Event::Rollback
        && matches!(current, State::Executing | State::Verifying | State::Failed)
    {
        return Ok(State::RolledBack);
    }
    if event == Event::DenyPolicy && matches!(current, State::Prechecking | State::Approved) {
        return Ok(State::PolicyDenied);
    }
    if event == Event::RecoverCompleted && matches!(current, State::Executing | State::Verifying) {
        return Ok(State::Committed);
    }
    if event == Event::RecoverNotCompleted && matches!(current, State::Executing | State::Verifying)
    {
        return Ok(State::Pending);
    }
    if event == Event::RecoverSafeReset
        && matches!(
            current,
            State::Prechecking | State::Approved | State::Retrying
        )
    {
        return Ok(State::Pending);
    }

    let next = match (current, event) {
        (State::Pending, Event::BeginPrecheck) => State::Prechecking,
        (State::Pending, Event::Skip) => State::Skipped,
        (State::Prechecking, Event::Approve) => State::Approved,
        (State::Approved, Event::BeginExecute) => State::Executing,
        (State::Executing, Event::BeginVerify) => State::Verifying,
        (State::Verifying, Event::Commit) => State::Committed,
        (State::Failed, Event::ScheduleRetry) => State::Retrying,
        (State::Retrying, Event::BeginRetry) => State::Pending,
        (_, _) => {
            return Err(TaskEngineError::InvalidStepTransition {
                from: current,
                event,
            });
        }
    };
    Ok(next)
}
