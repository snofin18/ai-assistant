//! Resumable production execution over an approval boundary.
//!
//! Responsibilities: validate the paused snapshot, rebuild the runtime
//! executor, and continue from the same Prechecking step.
//!
//! Boundaries: it does not decide policy, grant approval, or replay committed
//! steps.

use std::sync::{Arc, Mutex};

use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_task_engine::{
    MemoryCheckpointStore, Plan, PlanStep, StepId, StepStatus, TaskId, TaskSnapshot,
};

use crate::production_policy::{ApprovalWindows, CatalogStepPolicy};
use crate::production_run::{PendingRuntimeApproval, ProductionRun};
use crate::runtime::{
    EnvelopeObservationCollector, RuntimeExecutionError, RuntimeExecutor, StepExecutionOutcome,
};
use crate::runtime_binding::{BindingInvoker, RuntimeBindingState};
use crate::ui_ipc::UiAuthorizationScope;

use super::{ProductionError, ProductionHost, StepHandle};

impl<P> ProductionHost<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    /// Resumes a run that paused on an approval boundary.
    ///
    /// # Errors
    ///
    /// Fails closed when the run is not awaiting approval, the snapshot no
    /// longer points at the same Prechecking step, or the resumed execution
    /// encounters the normal runtime failure set.
    pub async fn resume_plan(
        &self,
        run: ProductionRun,
        now_ms: i64,
    ) -> Result<ProductionRun, ProductionError> {
        let (snapshots, snapshot, awaiting, engine, binding_state, approval_windows) =
            run.into_resume_parts();
        let pending = awaiting.ok_or_else(|| ProductionError::InvalidConfiguration {
            field: "production_run.resume",
            reason: "the run is not awaiting approval".to_owned(),
        })?;
        let start_sequence = resume_sequence(&snapshot, &pending)?;
        let steps = snapshot
            .plan
            .ordered_steps()
            .into_iter()
            .filter(|step| step.sequence >= start_sequence)
            .map(step_handle)
            .collect::<Vec<_>>();
        let policy = CatalogStepPolicy {
            rules: self.host.policy().clone(),
            catalog: self.policy_catalog.clone(),
            target_app: self.app_id.clone(),
            approvals: approval_windows.clone(),
        };
        let reserved = crate::reserved_invoker::ReservedRuntimeInvoker::new(
            self.host.tool_bus(),
            Arc::clone(&self.latest_snapshot),
        )
        .with_approvals(Arc::clone(&self.approvals), Arc::clone(&self.clock))
        .with_host_operations(Arc::clone(&self.host_operations));
        let binding_invoker = BindingInvoker::new(
            reserved,
            Arc::new(self.provider.dataflow_plan().clone()),
            Arc::clone(&binding_state),
            Arc::clone(&self.latest_snapshot),
        );
        let executor = RuntimeExecutor::new(
            engine,
            policy,
            binding_invoker,
            EnvelopeObservationCollector,
        );
        self.drive_steps(
            executor,
            snapshot.task_id.clone(),
            steps,
            binding_state,
            approval_windows,
            snapshots,
            now_ms,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn drive_steps(
        &self,
        mut executor: RuntimeExecutor<
            MemoryCheckpointStore,
            CatalogStepPolicy,
            BindingInvoker<crate::reserved_invoker::ReservedRuntimeInvoker<'_>>,
            EnvelopeObservationCollector,
        >,
        task_id: TaskId,
        steps: Vec<StepHandle>,
        binding_state: Arc<Mutex<RuntimeBindingState>>,
        approval_windows: ApprovalWindows,
        mut snapshots: Vec<TaskSnapshot>,
        now_ms: i64,
    ) -> Result<ProductionRun, ProductionError> {
        for (step_id, sequence, tool) in steps {
            let outcome = executor
                .advance(&task_id, &step_id, now_ms.saturating_add(3))
                .await?;
            let committed = matches!(&outcome, StepExecutionOutcome::Committed(_));
            let awaiting = matches!(&outcome, StepExecutionOutcome::AwaitingApproval);
            let mut state = binding_state.lock().map_err(|_| {
                ProductionError::Runtime(RuntimeExecutionError::Tool {
                    reason: "runtime binding state is unavailable".to_owned(),
                })
            })?;
            if committed {
                state.commit(step_id.as_str());
            } else {
                state.discard(step_id.as_str());
            }
            drop(state);
            if committed && tool == crate::runtime_tools::TOOL_REQUEST_APPROVAL {
                approval_windows.record(sequence)?;
            }
            if awaiting {
                let engine = executor.into_engine();
                let snapshot = engine.load_snapshot(&task_id)?;
                self.set_latest_snapshot(&snapshot);
                let pending = self.register_pending_approval(&snapshot, &step_id)?;
                return Ok(ProductionRun::new(
                    snapshots,
                    snapshot,
                    engine,
                    Some(pending),
                    binding_state,
                    approval_windows,
                ));
            }
            self.apply_step_outcome(outcome, &mut snapshots, &step_id, &task_id)?;
        }
        let engine = executor.into_engine();
        let final_snapshot = engine.load_snapshot(&task_id)?;
        self.set_latest_snapshot(&final_snapshot);
        Ok(ProductionRun::new(
            snapshots,
            final_snapshot,
            engine,
            None,
            binding_state,
            approval_windows,
        ))
    }

    fn register_pending_approval(
        &self,
        snapshot: &TaskSnapshot,
        step_id: &StepId,
    ) -> Result<PendingRuntimeApproval, ProductionError> {
        let step = snapshot
            .plan
            .steps
            .iter()
            .find(|step| &step.id == step_id)
            .ok_or_else(|| ProductionError::InvalidConfiguration {
                field: "pending_approval.step",
                reason: format!("step {step_id} is absent from the plan"),
            })?;
        let scopes = approval_scopes(step)?;
        let request_id = pending_request_id(&snapshot.task_id, step_id);
        self.pending_approvals
            .register(
                request_id.clone(),
                snapshot.task_id.clone(),
                step_id.clone(),
                scopes,
            )
            .map_err(|error| ProductionError::InvalidConfiguration {
                field: "pending_approval.registry",
                reason: error.to_string(),
            })?;
        Ok(PendingRuntimeApproval {
            request_id,
            task_id: snapshot.task_id.clone(),
            step_id: step_id.clone(),
        })
    }

    pub(super) fn apply_step_outcome(
        &self,
        outcome: StepExecutionOutcome,
        snapshots: &mut Vec<TaskSnapshot>,
        step_id: &StepId,
        task_id: &TaskId,
    ) -> Result<(), ProductionError> {
        match outcome {
            StepExecutionOutcome::Committed(snapshot) => {
                self.set_latest_snapshot(&snapshot);
                snapshots.push(snapshot);
                Ok(())
            }
            StepExecutionOutcome::PolicyDenied(snapshot) => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(RuntimeExecutionError::Policy {
                    reason: format!(
                        "policy denied step {} in task {}",
                        step_id.as_str(),
                        task_id.as_str()
                    ),
                }))
            }
            StepExecutionOutcome::AwaitingApproval => {
                Err(ProductionError::Runtime(RuntimeExecutionError::Policy {
                    reason: format!(
                        "step {} requires human approval before execution",
                        step_id.as_str()
                    ),
                }))
            }
            StepExecutionOutcome::ToolFailed {
                snapshot,
                code,
                message,
            } => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(RuntimeExecutionError::Tool {
                    reason: format!("step {} failed with {code:?}: {message}", step_id.as_str()),
                }))
            }
            StepExecutionOutcome::NeedsHuman { snapshot, reason } => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(
                    RuntimeExecutionError::Observation { reason },
                ))
            }
            StepExecutionOutcome::VerificationFailed { snapshot, outcome } => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(
                    RuntimeExecutionError::Postconditions {
                        reason: format!(
                            "step {} verification failed: {outcome:?}",
                            step_id.as_str()
                        ),
                    },
                ))
            }
        }
    }
}

fn resume_sequence(
    snapshot: &TaskSnapshot,
    pending: &PendingRuntimeApproval,
) -> Result<u32, ProductionError> {
    if snapshot.current_step.as_ref() != Some(&pending.step_id) {
        return Err(ProductionError::InvalidConfiguration {
            field: "production_run.resume",
            reason: format!(
                "pending approval step {} is not the current step",
                pending.step_id
            ),
        });
    }
    let step = snapshot
        .plan
        .steps
        .iter()
        .find(|step| step.id == pending.step_id)
        .ok_or_else(|| ProductionError::InvalidConfiguration {
            field: "production_run.resume",
            reason: format!(
                "pending approval step {} is absent from the plan",
                pending.step_id
            ),
        })?;
    let step_snapshot =
        snapshot
            .step(&pending.step_id)
            .ok_or_else(|| ProductionError::InvalidConfiguration {
                field: "production_run.resume",
                reason: format!(
                    "pending approval step {} has no task snapshot",
                    pending.step_id
                ),
            })?;
    if step_snapshot.status != StepStatus::Prechecking {
        return Err(ProductionError::InvalidConfiguration {
            field: "production_run.resume",
            reason: format!(
                "pending approval step {} is {:?}, not Prechecking",
                pending.step_id, step_snapshot.status
            ),
        });
    }
    Ok(step.sequence)
}

pub(super) fn ordered_step_handles(plan: &Plan) -> Vec<StepHandle> {
    plan.ordered_steps().into_iter().map(step_handle).collect()
}

fn step_handle(step: &PlanStep) -> StepHandle {
    (step.id.clone(), step.sequence, step.tool.clone())
}

fn pending_request_id(task_id: &TaskId, step_id: &StepId) -> String {
    format!("runtime:{}:{}", task_id.as_str(), step_id.as_str())
}

fn approval_scopes(step: &PlanStep) -> Result<Vec<UiAuthorizationScope>, ProductionError> {
    let options = step
        .args
        .get("scope_options")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ProductionError::InvalidConfiguration {
            field: "approval.scope_options",
            reason: "request_approval requires a non-empty scope_options array".to_owned(),
        })?;
    let mut scopes = Vec::with_capacity(options.len());
    for option in options {
        let scope = match option.as_str() {
            Some("once") => UiAuthorizationScope::Once,
            Some("task") => UiAuthorizationScope::ThisTask,
            Some("session") => UiAuthorizationScope::ThisAppSession,
            Some(other) => {
                return Err(ProductionError::InvalidConfiguration {
                    field: "approval.scope_options",
                    reason: format!("unknown approval scope `{other}`"),
                });
            }
            None => {
                return Err(ProductionError::InvalidConfiguration {
                    field: "approval.scope_options",
                    reason: "scope_options entries must be strings".to_owned(),
                });
            }
        };
        scopes.push(scope);
    }
    if scopes.is_empty() {
        return Err(ProductionError::InvalidConfiguration {
            field: "approval.scope_options",
            reason: "request_approval must offer at least one scope".to_owned(),
        });
    }
    Ok(scopes)
}
