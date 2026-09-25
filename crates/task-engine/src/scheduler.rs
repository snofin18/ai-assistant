//! Deterministic DAG frontier and Step snapshot mutation helpers.

use std::collections::BTreeMap;

use crate::error::{TaskEngineError, TaskEngineResult};
use crate::identifiers::StepId;
use crate::snapshot::{StepSnapshot, TaskSnapshot};
use crate::status::{StepEvent, StepStatus, transition_step};

/// Computes the deterministic ready frontier for a snapshot.
#[must_use]
pub fn ready_step_ids(snapshot: &TaskSnapshot) -> Vec<StepId> {
    let statuses: BTreeMap<StepId, StepStatus> = snapshot
        .steps
        .iter()
        .map(|step| (step.id.clone(), step.status))
        .collect();
    snapshot
        .plan
        .ordered_steps()
        .into_iter()
        .filter(|plan_step| statuses.get(&plan_step.id) == Some(&StepStatus::Pending))
        .filter(|plan_step| {
            plan_step.depends_on.iter().all(|dependency| {
                statuses
                    .get(dependency)
                    .is_some_and(|status| status.satisfies_dependency())
            })
        })
        .map(|plan_step| plan_step.id.clone())
        .collect()
}

/// Applies one legal step event to a mutable snapshot.
///
/// # Errors
///
/// Returns an invalid-transition or missing-step error.
pub fn apply_step_event(
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
    let step = mutable_step(snapshot, step_id)?;
    step.status = next;
    Ok(())
}

/// Returns a mutable step snapshot or a structured missing-step error.
///
/// # Errors
///
/// Returns [`TaskEngineError::StepNotFound`] when the id is absent.
pub fn mutable_step<'snapshot>(
    snapshot: &'snapshot mut TaskSnapshot,
    step_id: &StepId,
) -> TaskEngineResult<&'snapshot mut StepSnapshot> {
    let task_id = snapshot.task_id.to_string();
    snapshot
        .step_mut(step_id)
        .ok_or_else(|| TaskEngineError::StepNotFound {
            task_id,
            step_id: step_id.to_string(),
        })
}
