//! Stateful facade over the pure state machines and checkpoint store.

use std::collections::BTreeMap;

use assistant_protocol::ErrorCode;

use crate::budget::{BudgetCheck, UsageDelta};
use crate::checkpoint::{CheckpointStore, TaskCheckpoint};
use crate::error::{TaskEngineError, TaskEngineResult};
use crate::identifiers::{StepId, TaskId};
use crate::plan::Plan;
use crate::recovery::{RecoveryAssessment, RecoveryEvidence, assess_recovery};
use crate::scheduler::{apply_step_event, mutable_step, ready_step_ids};
use crate::snapshot::{SNAPSHOT_SCHEMA_VERSION, StepSnapshot, TaskHoldReason, TaskSnapshot};
use crate::status::{StepEvent, StepStatus, TaskEvent, TaskStatus, transition_task};
use crate::watchdog::{WatchdogDecision, check_watchdog, elapsed_ms};

/// Deterministic orchestration facade.
#[derive(Debug)]
pub struct TaskEngine<Store> {
    store: Store,
}

impl<Store> TaskEngine<Store> {
    /// Creates an engine over a checkpoint store.
    #[must_use]
    pub const fn new(store: Store) -> Self {
        Self { store }
    }
}

impl<Store: CheckpointStore> TaskEngine<Store> {
    /// Validates and creates a new controlled task.
    ///
    /// # Errors
    ///
    /// Returns plan validation errors, clock errors, or a checkpoint-store
    /// failure. No task is visible unless the initial checkpoint commits.
    pub fn create_task(&mut self, plan: Plan, now_ms: i64) -> TaskEngineResult<TaskSnapshot> {
        plan.validate()?;
        let steps = plan
            .ordered_steps()
            .into_iter()
            .map(|step| StepSnapshot {
                id: step.id.clone(),
                sequence: step.sequence,
                status: StepStatus::Pending,
                attempts: 0,
                started_at_ms: None,
                phase_started_at_ms: None,
                ended_at_ms: None,
                post_fingerprint: None,
            })
            .collect();
        let snapshot = TaskSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            revision: 0,
            task_id: plan.task_id.clone(),
            plan,
            status: TaskStatus::Draft,
            steps,
            budget_usage: crate::budget::BudgetUsage::default(),
            cancel_requested: false,
            pause_requested: false,
            warning_count: 0,
            current_step: None,
            started_at_ms: now_ms,
            updated_at_ms: now_ms,
            hold_reason: None,
            last_error_code: None,
        };
        snapshot.validate()?;
        self.store
            .create_task(&TaskCheckpoint::new(snapshot.clone()))?;
        Ok(snapshot)
    }

    /// Loads the latest checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::TaskNotFound`] when no checkpoint exists and
    /// propagates store read or schema failures.
    pub fn load_snapshot(&self, task_id: &TaskId) -> TaskEngineResult<TaskSnapshot> {
        self.store
            .load_latest(task_id)?
            .map(|checkpoint| checkpoint.snapshot)
            .ok_or_else(|| TaskEngineError::TaskNotFound {
                task_id: task_id.to_string(),
            })
    }

    /// Applies a task event after validating event-specific preconditions.
    ///
    /// # Errors
    ///
    /// Returns an invalid-transition error, a task-not-found error, or a
    /// checkpoint-store failure.
    pub fn apply_task_event(
        &mut self,
        task_id: &TaskId,
        event: TaskEvent,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        if matches!(event, TaskEvent::Complete | TaskEvent::CompleteWithWarnings)
            && !snapshot.all_steps_settled()
        {
            return Err(TaskEngineError::InvalidPlan {
                reason: "task cannot complete before all steps settle".to_owned(),
            });
        }
        snapshot.status = transition_task(snapshot.status, event)?;
        if event == TaskEvent::RequireHuman {
            snapshot.hold_reason = Some(TaskHoldReason::ExternalBlocked {
                message: "explicit human decision required".to_owned(),
            });
        }
        self.persist(snapshot, now_ms)
    }

    /// Starts one ready step and updates the step budget.
    ///
    /// # Errors
    ///
    /// Returns an error when the task is not running, another step is active,
    /// the step is not on the ready frontier, or persistence fails.
    pub fn begin_step(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        ensure_running_without_active_step(&snapshot)?;
        if !ready_step_ids(&snapshot).contains(step_id) {
            return Err(TaskEngineError::InvalidPlan {
                reason: format!("step {step_id} is not on the ready frontier"),
            });
        }

        let elapsed = elapsed_ms(snapshot.started_at_ms, now_ms)?;
        snapshot.budget_usage.elapsed_ms = elapsed;
        snapshot.budget_usage.record_step_started()?;
        if let BudgetCheck::Exceeded(limit) =
            snapshot.plan.budget.check(&snapshot.budget_usage, elapsed)
        {
            snapshot.status = TaskStatus::NeedsHuman;
            snapshot.hold_reason = Some(TaskHoldReason::BudgetExceeded { limit });
            snapshot.last_error_code = Some(ErrorCode::Fatal);
            return self.persist(snapshot, now_ms);
        }

        apply_step_event(&mut snapshot, step_id, StepEvent::BeginPrecheck)?;
        let step = mutable_step(&mut snapshot, step_id)?;
        step.attempts = step
            .attempts
            .checked_add(1)
            .ok_or(TaskEngineError::NumericOverflow {
                field: "step.attempts",
            })?;
        step.started_at_ms.get_or_insert(now_ms);
        step.phase_started_at_ms = Some(now_ms);
        snapshot.current_step = Some(step_id.clone());
        self.persist(snapshot, now_ms)
    }

    /// Approves a step after prechecks.
    ///
    /// # Errors
    ///
    /// Returns an invalid step transition or checkpoint-store failure.
    pub fn approve_step(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        self.transition_step(task_id, step_id, StepEvent::Approve, now_ms)
    }

    /// Starts tool execution for an approved step.
    ///
    /// # Errors
    ///
    /// Returns an invalid step transition or checkpoint-store failure.
    pub fn begin_execute(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        self.transition_step(task_id, step_id, StepEvent::BeginExecute, now_ms)
    }

    /// Starts verification for an executing step.
    ///
    /// # Errors
    ///
    /// Returns an invalid step transition or checkpoint-store failure.
    pub fn begin_verify(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        self.transition_step(task_id, step_id, StepEvent::BeginVerify, now_ms)
    }

    /// Commits a verified step and advances the task state machine.
    ///
    /// # Errors
    ///
    /// Returns an invalid step transition, clock error, or checkpoint-store
    /// failure.
    pub fn commit_step(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        post_fingerprint: Option<String>,
        had_warning: bool,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        apply_step_event(&mut snapshot, step_id, StepEvent::Commit)?;
        let step = mutable_step(&mut snapshot, step_id)?;
        step.phase_started_at_ms = None;
        step.ended_at_ms = Some(now_ms);
        step.post_fingerprint = post_fingerprint;
        snapshot.current_step = None;
        if had_warning {
            snapshot.warning_count =
                snapshot
                    .warning_count
                    .checked_add(1)
                    .ok_or(TaskEngineError::NumericOverflow {
                        field: "warning_count",
                    })?;
        }
        snapshot.status = if snapshot.cancel_requested {
            TaskStatus::Cancelled
        } else if snapshot.pause_requested {
            TaskStatus::Paused
        } else if snapshot.all_steps_settled() {
            if snapshot.warning_count == 0 {
                TaskStatus::Completed
            } else {
                TaskStatus::CompletedWithWarnings
            }
        } else {
            TaskStatus::Running
        };
        self.persist(snapshot, now_ms)
    }

    /// Records a failed step and fails the task.
    ///
    /// # Errors
    ///
    /// Returns an invalid step transition or checkpoint-store failure.
    pub fn fail_step(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        error_code: ErrorCode,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        apply_step_event(&mut snapshot, step_id, StepEvent::Fail)?;
        let step = mutable_step(&mut snapshot, step_id)?;
        step.phase_started_at_ms = None;
        step.ended_at_ms = Some(now_ms);
        snapshot.current_step = None;
        snapshot.status = TaskStatus::Failed;
        snapshot.last_error_code = Some(error_code);
        self.persist(snapshot, now_ms)
    }

    /// Records a policy denial and fails the task.
    ///
    /// # Errors
    ///
    /// Returns an invalid step transition or checkpoint-store failure.
    pub fn deny_step(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        apply_step_event(&mut snapshot, step_id, StepEvent::DenyPolicy)?;
        let step = mutable_step(&mut snapshot, step_id)?;
        step.phase_started_at_ms = None;
        step.ended_at_ms = Some(now_ms);
        snapshot.current_step = None;
        snapshot.status = TaskStatus::Failed;
        snapshot.last_error_code = Some(ErrorCode::PolicyDenied);
        self.persist(snapshot, now_ms)
    }

    /// Requests pause at the next step boundary.
    ///
    /// # Errors
    ///
    /// Returns an invalid task transition or checkpoint-store failure.
    pub fn request_pause(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        if snapshot.status != TaskStatus::Running {
            return Err(TaskEngineError::InvalidTaskTransition {
                from: snapshot.status,
                event: TaskEvent::Pause,
            });
        }
        if snapshot.current_step.is_some() {
            snapshot.pause_requested = true;
        } else {
            snapshot.status = TaskStatus::Paused;
        }
        self.persist(snapshot, now_ms)
    }

    /// Requests cancellation at the next step boundary.
    ///
    /// # Errors
    ///
    /// Returns an invalid task transition or checkpoint-store failure.
    pub fn request_cancel(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        if snapshot.status.is_terminal() {
            return Err(TaskEngineError::InvalidTaskTransition {
                from: snapshot.status,
                event: TaskEvent::Cancel,
            });
        }
        if snapshot.current_step.is_some() {
            snapshot.cancel_requested = true;
        } else {
            snapshot.status = TaskStatus::Cancelled;
        }
        self.persist(snapshot, now_ms)
    }

    /// Marks the task as taken over by a user.
    ///
    /// # Errors
    ///
    /// Returns an invalid task transition or checkpoint-store failure.
    pub fn take_over(&mut self, task_id: &TaskId, now_ms: i64) -> TaskEngineResult<TaskSnapshot> {
        self.apply_task_event(task_id, TaskEvent::TakeOver, now_ms)
    }

    /// Resumes after pause, a blocker, or a human decision.
    ///
    /// # Errors
    ///
    /// Returns an invalid task transition or checkpoint-store failure.
    pub fn resume(&mut self, task_id: &TaskId, now_ms: i64) -> TaskEngineResult<TaskSnapshot> {
        let snapshot = self.load_snapshot(task_id)?;
        let event = match snapshot.status {
            TaskStatus::Paused | TaskStatus::NeedsHuman => TaskEvent::Resume,
            TaskStatus::Blocked => TaskEvent::Unblock,
            TaskStatus::TakenOver => TaskEvent::ReturnControl,
            _ => {
                return Err(TaskEngineError::InvalidTaskTransition {
                    from: snapshot.status,
                    event: TaskEvent::Resume,
                });
            }
        };
        self.apply_task_event(task_id, event, now_ms)
    }

    /// Records externally measured token and cost usage.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown step, invalid usage, or checkpoint-store
    /// failure. Budget exhaustion is represented as a persisted
    /// `NeedsHuman` snapshot, not as an unrecorded failure.
    pub fn record_usage(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        delta: UsageDelta,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        let step = snapshot
            .step(step_id)
            .ok_or_else(|| TaskEngineError::StepNotFound {
                task_id: task_id.to_string(),
                step_id: step_id.to_string(),
            })?;
        if !matches!(step.status, StepStatus::Executing | StepStatus::Verifying) {
            return Err(TaskEngineError::InvalidPlan {
                reason: format!("usage can only be recorded for active step {step_id}"),
            });
        }
        snapshot
            .budget_usage
            .record_usage(delta.tokens_in, delta.tokens_out, delta.cost_usd)?;
        snapshot.budget_usage.elapsed_ms = elapsed_ms(snapshot.started_at_ms, now_ms)?;
        if let BudgetCheck::Exceeded(limit) = snapshot
            .plan
            .budget
            .check(&snapshot.budget_usage, snapshot.budget_usage.elapsed_ms)
        {
            snapshot.status = TaskStatus::NeedsHuman;
            snapshot.hold_reason = Some(TaskHoldReason::BudgetExceeded { limit });
            snapshot.last_error_code = Some(ErrorCode::Fatal);
        }
        self.persist(snapshot, now_ms)
    }

    /// Evaluates the budget at `now_ms`.
    ///
    /// # Errors
    ///
    /// Returns clock or checkpoint-store errors.
    pub fn check_budget(&self, task_id: &TaskId, now_ms: i64) -> TaskEngineResult<BudgetCheck> {
        let snapshot = self.load_snapshot(task_id)?;
        let elapsed = elapsed_ms(snapshot.started_at_ms, now_ms)?;
        Ok(snapshot.plan.budget.check(&snapshot.budget_usage, elapsed))
    }

    /// Persists `NeedsHuman` when the budget is already exhausted.
    ///
    /// # Errors
    ///
    /// Returns clock or checkpoint-store errors.
    pub fn enforce_budget(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        let elapsed = elapsed_ms(snapshot.started_at_ms, now_ms)?;
        snapshot.budget_usage.elapsed_ms = elapsed;
        if let BudgetCheck::Exceeded(limit) =
            snapshot.plan.budget.check(&snapshot.budget_usage, elapsed)
        {
            snapshot.status = TaskStatus::NeedsHuman;
            snapshot.hold_reason = Some(TaskHoldReason::BudgetExceeded { limit });
            snapshot.last_error_code = Some(ErrorCode::Fatal);
            return self.persist(snapshot, now_ms);
        }
        Ok(snapshot)
    }

    /// Evaluates the active step watchdog at `now_ms`.
    ///
    /// # Errors
    ///
    /// Returns clock, checkpoint, or store errors.
    pub fn check_watchdog(
        &self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> TaskEngineResult<WatchdogDecision> {
        let snapshot = self.load_snapshot(task_id)?;
        check_watchdog(&snapshot.plan, &snapshot, now_ms)
    }

    /// Persists `NeedsHuman` when an active phase watchdog expired.
    ///
    /// # Errors
    ///
    /// Returns clock, checkpoint, or store errors.
    pub fn enforce_watchdog(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        let decision = check_watchdog(&snapshot.plan, &snapshot, now_ms)?;
        if let WatchdogDecision::Expired {
            phase,
            timeout_ms,
            elapsed_ms,
        } = decision
        {
            let step_id = snapshot.current_step.clone().ok_or_else(|| {
                TaskEngineError::InvalidCheckpoint {
                    reason: "watchdog expired without a current step".to_owned(),
                }
            })?;
            snapshot.status = TaskStatus::NeedsHuman;
            snapshot.hold_reason = Some(TaskHoldReason::WatchdogExpired {
                step_id,
                phase,
                timeout_ms,
                elapsed_ms,
            });
            snapshot.last_error_code = Some(ErrorCode::TargetUnresponsive);
            return self.persist(snapshot, now_ms);
        }
        Ok(snapshot)
    }

    /// Applies crash-recovery evidence to the latest checkpoint.
    ///
    /// # Errors
    ///
    /// Returns checkpoint, clock, recovery, or store errors.
    pub fn recover(
        &mut self,
        task_id: &TaskId,
        evidence: &BTreeMap<StepId, RecoveryEvidence>,
        now_ms: i64,
    ) -> TaskEngineResult<RecoveryAssessment> {
        let snapshot = self.load_snapshot(task_id)?;
        let mut assessment = assess_recovery(snapshot, evidence, now_ms)?;
        assessment.snapshot = self.persist(assessment.snapshot, now_ms)?;
        Ok(assessment)
    }

    fn transition_step(
        &mut self,
        task_id: &TaskId,
        step_id: &StepId,
        event: StepEvent,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        let mut snapshot = self.load_snapshot(task_id)?;
        apply_step_event(&mut snapshot, step_id, event)?;
        let step = mutable_step(&mut snapshot, step_id)?;
        if matches!(event, StepEvent::BeginExecute | StepEvent::BeginVerify) {
            step.phase_started_at_ms = Some(now_ms);
        }
        self.persist(snapshot, now_ms)
    }

    fn persist(
        &mut self,
        mut snapshot: TaskSnapshot,
        now_ms: i64,
    ) -> TaskEngineResult<TaskSnapshot> {
        if now_ms < snapshot.updated_at_ms {
            return Err(TaskEngineError::ClockWentBackwards {
                previous_ms: snapshot.updated_at_ms,
                current_ms: now_ms,
            });
        }
        snapshot.revision =
            snapshot
                .revision
                .checked_add(1)
                .ok_or(TaskEngineError::NumericOverflow {
                    field: "snapshot.revision",
                })?;
        snapshot.updated_at_ms = now_ms;
        snapshot.validate()?;
        self.store
            .save_checkpoint(&TaskCheckpoint::new(snapshot.clone()))?;
        Ok(snapshot)
    }
}

fn ensure_running_without_active_step(snapshot: &TaskSnapshot) -> TaskEngineResult<()> {
    if snapshot.status != TaskStatus::Running {
        return Err(TaskEngineError::InvalidTaskTransition {
            from: snapshot.status,
            event: TaskEvent::ApprovePlan,
        });
    }
    if snapshot.current_step.is_some() {
        return Err(TaskEngineError::InvalidPlan {
            reason: "another step is already active".to_owned(),
        });
    }
    Ok(())
}
