//! Formal approval-session bridge for paused production runs.
//!
//! Responsibilities:
//! - keep a paused run's engine, pending registry, and grant table together;
//! - hand the transport-specific session to a caller-supplied callback;
//! - apply one validated approve or deny outcome to the same run.
//!
//! Boundaries:
//! - does not parse UI commands or own an IPC transport;
//! - does not auto-approve, persist grants, or replay committed steps;
//! - does not expose the private pause state outside the production root.

use std::sync::{Arc, Mutex};

use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_protocol::ErrorCode;
use assistant_storage::Clock;
use assistant_task_engine::{MemoryCheckpointStore, TaskEngine, TaskSnapshot, TaskStatus};

use crate::production_policy::ApprovalWindows;
use crate::production_run::{PendingRuntimeApproval, ProductionRun};
use crate::runtime_binding::RuntimeBindingState;
use crate::ui_control::TaskControlHandler;
use crate::ui_events::{CombinedEventSource, PendingApprovalEventSource};
use crate::ui_ipc::UiCommandOutcome;
use crate::ui_server::{UiEventSource, UiServerConfig};

use super::{PRODUCTION_PLAN_BUDGET_STEPS, ProductionError, ProductionHost};

struct FloorClock {
    inner: Arc<dyn Clock>,
    floor_ms: i64,
}

impl Clock for FloorClock {
    fn now_unix_ms(&self) -> i64 {
        self.inner.now_unix_ms().max(self.floor_ms)
    }
}

struct UiApprovalRunParts {
    snapshots: Vec<TaskSnapshot>,
    final_snapshot: TaskSnapshot,
    awaiting: PendingRuntimeApproval,
    binding_state: Arc<Mutex<RuntimeBindingState>>,
    approval_windows: ApprovalWindows,
}

fn require_matching_ui_request(
    outcome: &str,
    observed: &str,
    expected: &str,
) -> Result<(), ProductionError> {
    if observed == expected {
        return Ok(());
    }
    Err(ProductionError::InvalidConfiguration {
        field: "ui_approval.request_id",
        reason: format!(
            "{outcome} result `{observed}` does not match pending request `{expected}`"
        ),
    })
}

impl<P> ProductionHost<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    /// Drives a paused run through one or more bounded human approval sessions.
    ///
    /// The callback owns only the transport/session boundary. This method keeps
    /// the paused task's engine, pending registry and grant table together, so
    /// approve resumes the original Prechecking step and deny mutates the same
    /// task instead of a placeholder engine.
    ///
    /// # Errors
    ///
    /// Fails closed when the UI session disconnects without a decision, returns
    /// a non-decision outcome, references a different request, fails to apply a
    /// grant, pauses again on the same request, or exceeds the plan budget.
    pub async fn drive_ui_approvals<F>(
        &self,
        mut run: ProductionRun,
        mut serve: F,
    ) -> Result<ProductionRun, ProductionError>
    where
        F: FnMut(
            &UiServerConfig,
            &PendingRuntimeApproval,
            &mut TaskControlHandler<MemoryCheckpointStore>,
            &mut dyn UiEventSource,
        ) -> Result<UiCommandOutcome, ProductionError>,
    {
        let mut approval_count = 0_u64;
        while let Some(pending) = run.pending_approval().cloned() {
            approval_count = approval_count.saturating_add(1);
            if approval_count > PRODUCTION_PLAN_BUDGET_STEPS {
                return Err(ProductionError::InvalidConfiguration {
                    field: "ui_approval.count",
                    reason: "approval count exceeded the plan step budget".to_owned(),
                });
            }
            let (snapshots, final_snapshot, awaiting, engine, binding_state, approval_windows) =
                run.into_resume_parts();
            let decision_now = self
                .clock
                .now_unix_ms()
                .max(final_snapshot.updated_at_ms)
                .saturating_add(4);
            let awaiting = awaiting.ok_or_else(|| ProductionError::InvalidConfiguration {
                field: "ui_approval.pending",
                reason: "a paused run did not expose pending approval metadata".to_owned(),
            })?;
            let approval_clock: Arc<dyn Clock> = Arc::new(FloorClock {
                inner: Arc::clone(&self.clock),
                floor_ms: decision_now,
            });
            let mut handler = TaskControlHandler::new(engine, approval_clock)
                .with_pending(Arc::clone(&self.pending_approvals))
                .with_approvals(Arc::clone(&self.approvals));
            let mut events = CombinedEventSource::new(
                self.snapshot_event_source(),
                PendingApprovalEventSource::new(Arc::clone(&self.pending_approvals)),
            );
            let outcome = serve(
                &self.ui_config,
                &pending,
                &mut handler,
                &mut events as &mut dyn UiEventSource,
            )?;
            let engine = handler.into_engine();
            run = self
                .apply_ui_approval_outcome(
                    outcome,
                    engine,
                    UiApprovalRunParts {
                        snapshots,
                        final_snapshot,
                        awaiting,
                        binding_state,
                        approval_windows,
                    },
                    pending,
                    decision_now,
                )
                .await?;
        }
        Ok(run)
    }

    async fn apply_ui_approval_outcome(
        &self,
        outcome: UiCommandOutcome,
        engine: TaskEngine<MemoryCheckpointStore>,
        parts: UiApprovalRunParts,
        pending: PendingRuntimeApproval,
        decision_now: i64,
    ) -> Result<ProductionRun, ProductionError> {
        match outcome {
            UiCommandOutcome::ApprovalGranted { request_id, .. } => {
                require_matching_ui_request("approval", &request_id, &pending.request_id)?;
                let expected_request = pending.request_id;
                let run = ProductionRun::new(
                    parts.snapshots,
                    parts.final_snapshot,
                    engine,
                    Some(parts.awaiting),
                    parts.binding_state,
                    parts.approval_windows,
                );
                let run = self.resume_plan(run, decision_now).await?;
                if run
                    .pending_approval()
                    .is_some_and(|next| next.request_id == expected_request)
                {
                    return Err(ProductionError::InvalidConfiguration {
                        field: "ui_approval.resume",
                        reason: format!(
                            "approved request `{expected_request}` paused again instead of consuming its grant"
                        ),
                    });
                }
                Ok(run)
            }
            UiCommandOutcome::ApprovalDenied { request_id, .. } => {
                require_matching_ui_request("denial", &request_id, &pending.request_id)?;
                let final_snapshot = engine.load_snapshot(&pending.task_id)?;
                if final_snapshot.status != TaskStatus::Failed
                    || final_snapshot.last_error_code != Some(ErrorCode::PolicyDenied)
                {
                    return Err(ProductionError::InvalidConfiguration {
                        field: "ui_approval.denial",
                        reason: format!(
                            "denial result for `{}` was not applied by the run engine",
                            pending.request_id
                        ),
                    });
                }
                Ok(ProductionRun::new(
                    parts.snapshots,
                    final_snapshot,
                    engine,
                    None,
                    parts.binding_state,
                    parts.approval_windows,
                ))
            }
            other => Err(ProductionError::InvalidConfiguration {
                field: "ui_approval.outcome",
                reason: format!("expected approve or deny outcome, got {other:?}"),
            }),
        }
    }
}
