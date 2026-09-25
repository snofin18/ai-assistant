//! Crash recovery assessment.
//!
//! Recovery never guesses whether a write happened. A missing or explicit
//! unknown answer always becomes `NeedsHuman`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{TaskEngineError, TaskEngineResult};
use crate::identifiers::StepId;
use crate::snapshot::{TaskHoldReason, TaskSnapshot};
use crate::status::{StepEvent, StepStatus, TaskStatus, transition_step};

/// Externally supplied proof for a step that was active during a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecoveryEvidence {
    /// The target state proves that the step completed.
    Completed,
    /// The target state proves that the step did not complete.
    NotCompleted,
    /// Evidence cannot distinguish completed from not completed.
    Unknown,
}

/// Recovery outcome for one step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecoveryAction {
    /// The step was marked committed.
    Commit,
    /// The step returned to pending and must be executed again.
    Redo,
    /// A human must determine the actual outcome.
    NeedsHuman,
}

/// One step's recovery assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StepRecoveryAssessment {
    /// Assessed step.
    pub step_id: StepId,
    /// Recovery action.
    pub action: RecoveryAction,
}

/// Complete recovery assessment and candidate snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RecoveryAssessment {
    /// Snapshot with recovery decisions applied.
    pub snapshot: TaskSnapshot,
    /// Per-step decisions.
    pub steps: Vec<StepRecoveryAssessment>,
}

/// Applies crash evidence to a loaded checkpoint.
///
/// # Errors
///
/// Returns [`TaskEngineError::InvalidCheckpoint`] when an active step is not
/// present in the plan, or [`TaskEngineError::ClockWentBackwards`] when the
/// injected clock moved backwards.
pub fn assess_recovery(
    mut snapshot: TaskSnapshot,
    evidence: &BTreeMap<StepId, RecoveryEvidence>,
    now_ms: i64,
) -> TaskEngineResult<RecoveryAssessment> {
    snapshot.validate()?;
    if now_ms < snapshot.updated_at_ms {
        return Err(TaskEngineError::ClockWentBackwards {
            previous_ms: snapshot.updated_at_ms,
            current_ms: now_ms,
        });
    }

    let active_ids = active_recovery_step_ids(&snapshot);

    let mut actions = Vec::new();
    let mut unknown_step = None;
    let mut missing_evidence = None;

    for step_id in active_ids {
        let (action, issue) = apply_recovery_step(&mut snapshot, &step_id, evidence, now_ms)?;
        match issue {
            RecoveryIssue::None => {}
            RecoveryIssue::MissingEvidence => missing_evidence = Some(step_id.clone()),
            RecoveryIssue::UnknownOutcome => unknown_step = Some(step_id.clone()),
        }
        actions.push(StepRecoveryAssessment { step_id, action });
    }

    let hold_reason = missing_evidence
        .map(|step_id| TaskHoldReason::MissingRecoveryEvidence { step_id })
        .or_else(|| unknown_step.map(|step_id| TaskHoldReason::UnknownStepOutcome { step_id }));
    snapshot.status = derive_recovery_status(&mut snapshot, hold_reason);
    snapshot.current_step = snapshot.current_step.clone().filter(|step_id| {
        snapshot.step(step_id).is_some_and(|step| {
            matches!(step.status, StepStatus::Executing | StepStatus::Verifying)
        })
    });
    snapshot.updated_at_ms = now_ms;
    Ok(RecoveryAssessment {
        snapshot,
        steps: actions,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryIssue {
    None,
    MissingEvidence,
    UnknownOutcome,
}

fn active_recovery_step_ids(snapshot: &TaskSnapshot) -> Vec<StepId> {
    snapshot
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step.status,
                StepStatus::Prechecking
                    | StepStatus::Approved
                    | StepStatus::Executing
                    | StepStatus::Verifying
                    | StepStatus::Retrying
            )
        })
        .map(|step| step.id.clone())
        .collect()
}

fn apply_recovery_step(
    snapshot: &mut TaskSnapshot,
    step_id: &StepId,
    evidence: &BTreeMap<StepId, RecoveryEvidence>,
    now_ms: i64,
) -> TaskEngineResult<(RecoveryAction, RecoveryIssue)> {
    let status = snapshot
        .step(step_id)
        .map_or(StepStatus::Pending, |step| step.status);
    let result = match status {
        StepStatus::Prechecking | StepStatus::Approved | StepStatus::Retrying => {
            apply_step_event(snapshot, step_id, StepEvent::RecoverSafeReset)?;
            (RecoveryAction::Redo, RecoveryIssue::None)
        }
        StepStatus::Executing | StepStatus::Verifying => match evidence.get(step_id) {
            Some(RecoveryEvidence::Completed) => {
                apply_step_event(snapshot, step_id, StepEvent::RecoverCompleted)?;
                finish_step_phase(snapshot, step_id, now_ms)?;
                (RecoveryAction::Commit, RecoveryIssue::None)
            }
            Some(RecoveryEvidence::NotCompleted) => {
                apply_step_event(snapshot, step_id, StepEvent::RecoverNotCompleted)?;
                clear_step_phase(snapshot, step_id)?;
                (RecoveryAction::Redo, RecoveryIssue::None)
            }
            Some(RecoveryEvidence::Unknown) => {
                (RecoveryAction::NeedsHuman, RecoveryIssue::UnknownOutcome)
            }
            None => (RecoveryAction::NeedsHuman, RecoveryIssue::MissingEvidence),
        },
        _ => (RecoveryAction::NeedsHuman, RecoveryIssue::UnknownOutcome),
    };
    Ok(result)
}

fn derive_recovery_status(
    snapshot: &mut TaskSnapshot,
    hold_reason: Option<TaskHoldReason>,
) -> TaskStatus {
    if let Some(reason) = hold_reason {
        snapshot.hold_reason = Some(reason);
        return TaskStatus::NeedsHuman;
    }
    if snapshot.cancel_requested {
        snapshot.hold_reason = None;
        return TaskStatus::Cancelled;
    }
    if let Some(blocking) = snapshot
        .steps
        .iter()
        .find(|step| step.status.blocks_progress())
    {
        snapshot.hold_reason = Some(TaskHoldReason::RecoveryBlockedByStep {
            step_id: blocking.id.clone(),
            status: blocking.status,
        });
        return TaskStatus::NeedsHuman;
    }
    snapshot.hold_reason = None;
    if snapshot.all_steps_settled() {
        if snapshot.warning_count == 0 {
            return TaskStatus::Completed;
        }
        return TaskStatus::CompletedWithWarnings;
    }
    TaskStatus::Resuming
}

fn apply_step_event(
    snapshot: &mut TaskSnapshot,
    step_id: &StepId,
    event: StepEvent,
) -> TaskEngineResult<()> {
    let current = snapshot
        .step(step_id)
        .ok_or_else(|| TaskEngineError::StepNotFound {
            task_id: snapshot.task_id.to_string(),
            step_id: step_id.to_string(),
        })?
        .status;
    let next = transition_step(current, event)?;
    let task_id = snapshot.task_id.to_string();
    let step = snapshot
        .step_mut(step_id)
        .ok_or_else(|| TaskEngineError::StepNotFound {
            task_id,
            step_id: step_id.to_string(),
        })?;
    step.status = next;
    Ok(())
}

fn finish_step_phase(
    snapshot: &mut TaskSnapshot,
    step_id: &StepId,
    now_ms: i64,
) -> TaskEngineResult<()> {
    let task_id = snapshot.task_id.to_string();
    let step = snapshot
        .step_mut(step_id)
        .ok_or_else(|| TaskEngineError::StepNotFound {
            task_id,
            step_id: step_id.to_string(),
        })?;
    step.phase_started_at_ms = None;
    step.ended_at_ms = Some(now_ms);
    Ok(())
}

fn clear_step_phase(snapshot: &mut TaskSnapshot, step_id: &StepId) -> TaskEngineResult<()> {
    let task_id = snapshot.task_id.to_string();
    let step = snapshot
        .step_mut(step_id)
        .ok_or_else(|| TaskEngineError::StepNotFound {
            task_id,
            step_id: step_id.to_string(),
        })?;
    step.phase_started_at_ms = None;
    step.ended_at_ms = None;
    Ok(())
}
