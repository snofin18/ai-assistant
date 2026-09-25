//! Persisted task snapshots and step snapshots.

use std::collections::BTreeSet;

use assistant_protocol::ErrorCode;
use serde::{Deserialize, Serialize};

use crate::budget::{BudgetLimit, BudgetUsage};
use crate::error::{TaskEngineError, TaskEngineResult};
use crate::identifiers::{StepId, TaskId};
use crate::plan::Plan;
use crate::status::{StepPhase, StepStatus, TaskStatus};

/// Current snapshot schema version.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Reason the engine stopped automatic progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskHoldReason {
    /// A task budget was exhausted.
    BudgetExceeded {
        /// Exhausted dimension.
        limit: BudgetLimit,
    },
    /// A phase-specific watchdog expired.
    WatchdogExpired {
        /// Step whose watchdog expired.
        step_id: StepId,
        /// Watched phase.
        phase: StepPhase,
        /// Configured timeout.
        timeout_ms: u64,
        /// Observed elapsed time.
        elapsed_ms: u64,
    },
    /// Recovery did not receive evidence for an active step.
    MissingRecoveryEvidence {
        /// Step lacking evidence.
        step_id: StepId,
    },
    /// Recovery evidence explicitly reported an unknown outcome.
    UnknownStepOutcome {
        /// Step with unknown outcome.
        step_id: StepId,
    },
    /// Recovery found a failed or rolled-back step that blocks automatic resume.
    RecoveryBlockedByStep {
        /// Blocking step.
        step_id: StepId,
        /// Blocking state.
        status: StepStatus,
    },
    /// A target application or external service is unavailable.
    ExternalBlocked {
        /// Human-readable explanation.
        message: String,
    },
}

/// One step's persisted progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StepSnapshot {
    /// Stable plan step identifier.
    pub id: StepId,
    /// Stable sequence from the plan.
    pub sequence: u32,
    /// Current step state.
    pub status: StepStatus,
    /// Number of times prechecking started.
    pub attempts: u32,
    /// First attempt timestamp.
    pub started_at_ms: Option<i64>,
    /// Start of the currently watched phase.
    pub phase_started_at_ms: Option<i64>,
    /// Completion timestamp once the step reaches a terminal disposition.
    pub ended_at_ms: Option<i64>,
    /// Optional postcondition fingerprint captured after verification.
    pub post_fingerprint: Option<String>,
}

impl StepSnapshot {
    /// Returns the phase watched for the current status.
    #[must_use]
    pub const fn watched_phase(&self) -> Option<StepPhase> {
        match self.status {
            StepStatus::Prechecking => Some(StepPhase::Resolve),
            StepStatus::Executing => Some(StepPhase::Execute),
            StepStatus::Verifying => Some(StepPhase::Verify),
            _ => None,
        }
    }
}

/// Complete recoverable state for one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TaskSnapshot {
    /// Schema version used to reject incompatible checkpoints.
    pub schema_version: u32,
    /// Monotonic checkpoint revision.
    pub revision: u64,
    /// Task identifier.
    pub task_id: TaskId,
    /// Validated plan.
    pub plan: Plan,
    /// Current task state.
    pub status: TaskStatus,
    /// Step snapshots in plan order.
    pub steps: Vec<StepSnapshot>,
    /// Accumulated resource usage.
    pub budget_usage: BudgetUsage,
    /// User requested cancellation at the next step boundary.
    pub cancel_requested: bool,
    /// User requested pause at the next step boundary.
    pub pause_requested: bool,
    /// Number of successful steps that used a warning path.
    pub warning_count: u32,
    /// Current active step, if any.
    pub current_step: Option<StepId>,
    /// Task creation timestamp.
    pub started_at_ms: i64,
    /// Last persisted update timestamp.
    pub updated_at_ms: i64,
    /// Current human-decision or stop reason.
    pub hold_reason: Option<TaskHoldReason>,
    /// Last protocol error category associated with the snapshot.
    pub last_error_code: Option<ErrorCode>,
}

impl TaskSnapshot {
    /// Validates schema, identifiers, plan structure, and step alignment.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::InvalidCheckpoint`] for incompatible schema
    /// or mismatched step/plan data, and plan validation errors for an invalid
    /// embedded plan.
    pub fn validate(&self) -> TaskEngineResult<()> {
        if self.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(TaskEngineError::InvalidCheckpoint {
                reason: format!(
                    "unsupported snapshot schema version {}; expected {}",
                    self.schema_version, SNAPSHOT_SCHEMA_VERSION
                ),
            });
        }
        // Deserialization bypasses newtype constructors, so re-run validation.
        let task_id = TaskId::new(self.task_id.as_str())?;
        let _plan_id = crate::identifiers::PlanId::new(self.plan.plan_id.as_str())?;
        if task_id != self.plan.task_id {
            return Err(TaskEngineError::InvalidCheckpoint {
                reason: "snapshot task id does not match plan task id".to_owned(),
            });
        }
        self.plan.validate()?;

        let plan_steps: Vec<(StepId, u32)> = self
            .plan
            .ordered_steps()
            .iter()
            .map(|step| (step.id.clone(), step.sequence))
            .collect();
        let snapshot_steps: Vec<(StepId, u32)> = self
            .steps
            .iter()
            .map(|step| (step.id.clone(), step.sequence))
            .collect();
        if plan_steps != snapshot_steps {
            return Err(TaskEngineError::InvalidCheckpoint {
                reason: "snapshot steps do not align with plan steps".to_owned(),
            });
        }

        let ids: BTreeSet<&StepId> = self.steps.iter().map(|step| &step.id).collect();
        if ids.len() != self.steps.len() {
            return Err(TaskEngineError::InvalidCheckpoint {
                reason: "snapshot contains duplicate step ids".to_owned(),
            });
        }
        if let Some(current) = &self.current_step
            && !ids.contains(current)
        {
            return Err(TaskEngineError::InvalidCheckpoint {
                reason: format!("current step {current} is absent from the snapshot"),
            });
        }
        Ok(())
    }

    /// Returns one step snapshot.
    #[must_use]
    pub fn step(&self, step_id: &StepId) -> Option<&StepSnapshot> {
        self.steps.iter().find(|step| &step.id == step_id)
    }

    pub(crate) fn step_mut(&mut self, step_id: &StepId) -> Option<&mut StepSnapshot> {
        self.steps.iter_mut().find(|step| &step.id == step_id)
    }

    /// Returns the highest committed or skipped plan sequence.
    #[must_use]
    pub fn last_step_sequence(&self) -> u32 {
        self.steps
            .iter()
            .filter(|step| step.status.satisfies_dependency())
            .map(|step| step.sequence)
            .max()
            .unwrap_or(0)
    }

    /// Returns whether every step has a dependency-satisfying state.
    #[must_use]
    pub fn all_steps_settled(&self) -> bool {
        self.steps
            .iter()
            .all(|step| step.status.satisfies_dependency())
    }
}
