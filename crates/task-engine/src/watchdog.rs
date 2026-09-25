//! Phase-specific watchdog evaluation.

use serde::{Deserialize, Serialize};

use crate::error::{TaskEngineError, TaskEngineResult};
use crate::plan::Plan;
use crate::snapshot::TaskSnapshot;
use crate::status::{StepPhase, StepStatus};

/// Deterministic result of a watchdog check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WatchdogDecision {
    /// No active phase is being watched.
    Idle,
    /// The active phase has not reached its timeout.
    Healthy {
        /// Remaining milliseconds.
        remaining_ms: u64,
    },
    /// The active phase reached or exceeded its timeout.
    Expired {
        /// Watched phase.
        phase: StepPhase,
        /// Configured timeout.
        timeout_ms: u64,
        /// Observed elapsed time.
        elapsed_ms: u64,
    },
}

/// Checks the active step phase against its configured timeout.
///
/// # Errors
///
/// Returns [`TaskEngineError::ClockWentBackwards`] if `now_ms` predates the
/// phase start. Returns [`TaskEngineError::InvalidCheckpoint`] if the active
/// step is absent from the plan.
pub fn check_watchdog(
    plan: &Plan,
    snapshot: &TaskSnapshot,
    now_ms: i64,
) -> TaskEngineResult<WatchdogDecision> {
    let Some(active_step_id) = active_step_id(snapshot) else {
        return Ok(WatchdogDecision::Idle);
    };
    let Some(step) = snapshot.steps.iter().find(|step| step.id == active_step_id) else {
        return Ok(WatchdogDecision::Idle);
    };
    let Some(phase) = step.watched_phase() else {
        return Ok(WatchdogDecision::Idle);
    };
    let Some(started_at_ms) = step.phase_started_at_ms else {
        return Err(TaskEngineError::InvalidCheckpoint {
            reason: format!("active phase for step {} has no start timestamp", step.id),
        });
    };
    let elapsed_ms = elapsed_ms(started_at_ms, now_ms)?;
    let plan_step = plan
        .steps
        .iter()
        .find(|candidate| candidate.id == step.id)
        .ok_or_else(|| TaskEngineError::InvalidCheckpoint {
            reason: format!("active step {} is absent from the plan", step.id),
        })?;
    let timeout_ms = match phase {
        StepPhase::Resolve => plan_step.timeouts.resolve_ms,
        StepPhase::Execute => plan_step.timeouts.execute_ms,
        StepPhase::Verify => plan_step.timeouts.verify_ms,
    };
    if elapsed_ms >= timeout_ms {
        return Ok(WatchdogDecision::Expired {
            phase,
            timeout_ms,
            elapsed_ms,
        });
    }
    Ok(WatchdogDecision::Healthy {
        remaining_ms: timeout_ms - elapsed_ms,
    })
}

fn active_step_id(snapshot: &TaskSnapshot) -> Option<crate::identifiers::StepId> {
    snapshot.current_step.clone().or_else(|| {
        snapshot
            .steps
            .iter()
            .find(|step| {
                matches!(
                    step.status,
                    StepStatus::Prechecking | StepStatus::Executing | StepStatus::Verifying
                )
            })
            .map(|step| step.id.clone())
    })
}

/// Computes non-negative elapsed milliseconds without wrapping.
///
/// # Errors
///
/// Returns [`TaskEngineError::ClockWentBackwards`] when `now_ms` predates
/// `started_at_ms`, or [`TaskEngineError::NumericOverflow`] when the result
/// does not fit in `u64`.
pub fn elapsed_ms(started_at_ms: i64, now_ms: i64) -> TaskEngineResult<u64> {
    let elapsed = now_ms
        .checked_sub(started_at_ms)
        .ok_or(TaskEngineError::ClockWentBackwards {
            previous_ms: started_at_ms,
            current_ms: now_ms,
        })?;
    if elapsed < 0 {
        return Err(TaskEngineError::ClockWentBackwards {
            previous_ms: started_at_ms,
            current_ms: now_ms,
        });
    }
    u64::try_from(elapsed).map_err(|_| TaskEngineError::NumericOverflow {
        field: "watchdog.elapsed_ms",
    })
}
