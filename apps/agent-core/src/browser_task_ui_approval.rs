//! Browser form approval orchestration for terminal and formal UI sessions.
//!
//! Responsibilities:
//! - keep the paused browser executor's engine, pending registry and grants together;
//! - serve one formal UI decision session when `--ui-approval` is selected;
//! - rebuild the same runtime executor around the resumed engine.
//!
//! Boundaries:
//! - does not decide policy, submit the form, or render desktop UI pages;
//! - does not treat a disconnect or non-decision command as human approval.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use assistant_agent_core::{
    ApprovalGrants, BindingInvoker, CombinedEventSource, EnvelopeObservationCollector,
    GrantRequest, PendingApprovalEventSource, PendingApprovals, RuntimeBindingState,
    RuntimeDataflowPlan, RuntimeExecutionError, RuntimeExecutor, SnapshotEventSource,
    StepExecutionOutcome, TaskControlHandler, ToolBusInvoker, UiAuthorizationScope,
    UiCommandOutcome, UiEvent, UiEventSource, UiServerConfig,
    serve_until_human_decision_with_events,
};
use assistant_hitl::ApprovalScope;
use assistant_protocol::{ErrorCode, ToolSchema};
use assistant_storage::{Clock, SystemClock};
use assistant_task_engine::{PlanStep, StepId, TaskEngine, TaskId, TaskSnapshot, TaskStatus};
use assistant_tool_bus::ToolBus;

use super::browser_task_support::BrowserTaskError;
use super::{
    BrowserExecutor, BrowserInvocationRecorder, BrowserTaskPolicy, RecordingInvoker,
    browser_task_rules,
};

pub(super) type BrowserUiServe = fn(
    &UiServerConfig,
    &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError>;

#[derive(Clone, Copy)]
pub(super) enum BrowserApprovalMode<'a> {
    None,
    Terminal,
    Ui {
        config: &'a UiServerConfig,
        serve: BrowserUiServe,
    },
}

impl BrowserApprovalMode<'_> {
    pub(super) const fn is_interactive(self) -> bool {
        !matches!(self, Self::None)
    }
}

pub(super) struct BrowserDriveState<'state, 'bus> {
    pub(super) bus: &'bus ToolBus,
    pub(super) catalog: &'state BTreeMap<String, ToolSchema>,
    pub(super) dataflow: &'state RuntimeDataflowPlan,
    pub(super) app_id: &'state str,
    pub(super) binding_state: &'state Arc<Mutex<RuntimeBindingState>>,
    pub(super) latest_snapshot: &'state Arc<Mutex<Option<TaskSnapshot>>>,
    pub(super) recorder: &'state Arc<Mutex<BrowserInvocationRecorder>>,
    pub(super) pending_approvals: &'state Arc<PendingApprovals>,
    pub(super) approval_grants: &'state Arc<ApprovalGrants>,
    pub(super) approval_mode: BrowserApprovalMode<'state>,
}

struct PausedApproval<'a> {
    task_id: &'a TaskId,
    step: &'a PlanStep,
    now_ms: i64,
}

pub(super) async fn resolve_awaiting_approval<'bus>(
    executor: BrowserExecutor<'bus>,
    task_id: &TaskId,
    step: &PlanStep,
    state: &BrowserDriveState<'_, 'bus>,
    now_ms: i64,
) -> Result<(BrowserExecutor<'bus>, StepExecutionOutcome), BrowserTaskError> {
    let paused = PausedApproval {
        task_id,
        step,
        now_ms,
    };
    match state.approval_mode {
        BrowserApprovalMode::None => Ok((executor, StepExecutionOutcome::AwaitingApproval)),
        BrowserApprovalMode::Terminal => resolve_terminal_approval(executor, &paused, state).await,
        BrowserApprovalMode::Ui { config, serve } => {
            resolve_ui_approval(executor, &paused, state, config, serve).await
        }
    }
}

async fn resolve_terminal_approval<'bus>(
    mut executor: BrowserExecutor<'bus>,
    paused: &PausedApproval<'_>,
    state: &BrowserDriveState<'_, 'bus>,
) -> Result<(BrowserExecutor<'bus>, StepExecutionOutcome), BrowserTaskError> {
    let event =
        take_runtime_approval_event(state.pending_approvals, paused.task_id, &paused.step.id)?;
    if !prompt_for_human_approval(&event)? {
        return Err(BrowserTaskError::HumanRejected {
            step_id: paused.step.id.to_string(),
        });
    }
    grant_human_approval(&event, state.approval_grants, paused.now_ms)?;
    let outcome = executor
        .advance(
            paused.task_id,
            &paused.step.id,
            paused.now_ms.saturating_add(3),
        )
        .await?;
    Ok((executor, outcome))
}

async fn resolve_ui_approval<'bus>(
    executor: BrowserExecutor<'bus>,
    paused: &PausedApproval<'_>,
    state: &BrowserDriveState<'_, 'bus>,
    config: &UiServerConfig,
    serve: BrowserUiServe,
) -> Result<(BrowserExecutor<'bus>, StepExecutionOutcome), BrowserTaskError> {
    let expected_request = format!(
        "runtime:{}:{}",
        paused.task_id.as_str(),
        paused.step.id.as_str()
    );
    let engine = executor.into_engine();
    let paused_snapshot = engine.load_snapshot(paused.task_id)?;
    set_latest_snapshot(state.latest_snapshot, &paused_snapshot)?;

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let mut handler = TaskControlHandler::new(engine, clock)
        .with_pending(Arc::clone(state.pending_approvals))
        .with_approvals(Arc::clone(state.approval_grants));
    let latest = Arc::clone(state.latest_snapshot);
    let snapshots =
        SnapshotEventSource::new(move || latest.lock().ok().and_then(|value| value.clone()));
    let mut events = CombinedEventSource::new(
        snapshots,
        PendingApprovalEventSource::new(Arc::clone(state.pending_approvals)),
    );
    let outcome = serve(config, &mut handler, &mut events)?;
    let engine = handler.into_engine();
    match outcome {
        UiCommandOutcome::ApprovalGranted { request_id, .. } => {
            require_matching_request("approval", &request_id, &expected_request)?;
            let mut executor = rebuild_executor(state, paused.task_id.clone(), engine);
            let next_outcome = executor
                .advance(
                    paused.task_id,
                    &paused.step.id,
                    paused.now_ms.saturating_add(4),
                )
                .await?;
            Ok((executor, next_outcome))
        }
        UiCommandOutcome::ApprovalDenied { request_id, .. } => {
            require_matching_request("denial", &request_id, &expected_request)?;
            let snapshot = engine.load_snapshot(paused.task_id)?;
            if snapshot.status != TaskStatus::Failed
                || snapshot.last_error_code != Some(ErrorCode::PolicyDenied)
            {
                return Err(BrowserTaskError::Runtime(RuntimeExecutionError::Policy {
                    reason: "UI denial outcome was not applied by the browser task engine"
                        .to_owned(),
                }));
            }
            Err(BrowserTaskError::HumanRejected {
                step_id: paused.step.id.to_string(),
            })
        }
        other => Err(BrowserTaskError::Runtime(RuntimeExecutionError::Policy {
            reason: format!("expected browser UI approve or deny outcome, got {other:?}"),
        })),
    }
}

fn rebuild_executor<'bus>(
    state: &BrowserDriveState<'_, 'bus>,
    task_id: TaskId,
    engine: TaskEngine<assistant_task_engine::MemoryCheckpointStore>,
) -> BrowserExecutor<'bus> {
    let policy = BrowserTaskPolicy::new(
        browser_task_rules(),
        state.catalog.clone(),
        state.app_id.to_owned(),
        Arc::clone(state.approval_grants),
        task_id,
    );
    let invoker = RecordingInvoker::new(ToolBusInvoker::new(state.bus), Arc::clone(state.recorder));
    let binding = BindingInvoker::new(
        invoker,
        Arc::new(state.dataflow.clone()),
        Arc::clone(state.binding_state),
        Arc::clone(state.latest_snapshot),
    );
    RuntimeExecutor::new(engine, policy, binding, EnvelopeObservationCollector)
}

pub(super) fn serve_formal_ui(
    config: &UiServerConfig,
    handler: &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    events: &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError> {
    serve_until_human_decision_with_events(config, handler, events).map_err(|error| {
        BrowserTaskError::UiServer {
            reason: error.to_string(),
        }
    })
}

fn require_matching_request(
    outcome: &str,
    observed: &str,
    expected: &str,
) -> Result<(), BrowserTaskError> {
    if observed == expected {
        return Ok(());
    }
    Err(BrowserTaskError::Runtime(RuntimeExecutionError::Policy {
        reason: format!(
            "{outcome} result `{observed}` does not match pending request `{expected}`"
        ),
    }))
}

fn set_latest_snapshot(
    latest_snapshot: &Arc<Mutex<Option<TaskSnapshot>>>,
    snapshot: &TaskSnapshot,
) -> Result<(), BrowserTaskError> {
    *latest_snapshot.lock().map_err(|_| {
        BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: "latest task snapshot state is unavailable".to_owned(),
        })
    })? = Some(snapshot.clone());
    Ok(())
}

pub(super) fn take_runtime_approval_event(
    pending_approvals: &PendingApprovals,
    task_id: &TaskId,
    step_id: &StepId,
) -> Result<UiEvent, BrowserTaskError> {
    let events = pending_approvals.drain_events().map_err(|error| {
        BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: format!("pending approval event source failed: {error}"),
        })
    })?;
    let mut matching = Vec::new();
    let mut unexpected = Vec::new();
    for event in events {
        match &event {
            UiEvent::StepApprovalRequested {
                task_id: event_task,
                step_id: event_step,
                ..
            } if event_task == task_id.as_str() && event_step == step_id.as_str() => {
                matching.push(event);
            }
            _ => unexpected.push(event),
        }
    }
    if !unexpected.is_empty() {
        return Err(BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: "pending approval queue contained an event for another task or step".to_owned(),
        }));
    }
    if matching.len() != 1 {
        return Err(BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: format!(
                "expected exactly one pending approval event for step `{step_id}`, got {}",
                matching.len()
            ),
        }));
    }
    matching.pop().ok_or_else(|| {
        BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: "pending approval event disappeared after validation".to_owned(),
        })
    })
}

pub(super) fn prompt_for_human_approval(event: &UiEvent) -> Result<bool, BrowserTaskError> {
    let rendered =
        serde_json::to_string_pretty(event).map_err(|error| BrowserTaskError::HumanPrompt {
            reason: format!("approval event serialization failed: {error}"),
        })?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{rendered}").map_err(|error| BrowserTaskError::HumanPrompt {
        reason: format!("cannot write approval event: {error}"),
    })?;
    writeln!(
        stdout,
        "Type `once` to approve this exact form submission; anything else rejects it."
    )
    .map_err(|error| BrowserTaskError::HumanPrompt {
        reason: format!("cannot write approval prompt: {error}"),
    })?;
    stdout
        .flush()
        .map_err(|error| BrowserTaskError::HumanPrompt {
            reason: format!("cannot flush approval prompt: {error}"),
        })?;
    let mut response = String::new();
    io::stdin()
        .read_line(&mut response)
        .map_err(|error| BrowserTaskError::HumanPrompt {
            reason: format!("cannot read approval response: {error}"),
        })?;
    Ok(response.trim() == "once")
}

pub(super) fn grant_human_approval(
    event: &UiEvent,
    approval_grants: &ApprovalGrants,
    now_ms: i64,
) -> Result<(), BrowserTaskError> {
    let UiEvent::StepApprovalRequested {
        request_id,
        task_id,
        step_id,
        scope_options,
        expires_at_ms,
        ..
    } = event
    else {
        return Err(BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: "human approval event has the wrong type".to_owned(),
        }));
    };
    if scope_options.as_slice() != [UiAuthorizationScope::Once] {
        return Err(BrowserTaskError::Runtime(RuntimeExecutionError::Policy {
            reason: "human form approval must offer exactly once".to_owned(),
        }));
    }
    let task = TaskId::new(task_id.clone()).map_err(|error| {
        BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: format!("human approval task id is invalid: {error}"),
        })
    })?;
    let step = StepId::new(step_id.clone()).map_err(|error| {
        BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: format!("human approval step id is invalid: {error}"),
        })
    })?;
    let expected_request_id = format!("runtime:{}:{}", task.as_str(), step.as_str());
    if request_id != &expected_request_id {
        return Err(BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
            reason: "human approval request id does not match task and step".to_owned(),
        }));
    }
    let ttl_ms = expires_at_ms.saturating_sub(now_ms);
    if ttl_ms <= 0 {
        return Err(BrowserTaskError::HumanApprovalExpired {
            step_id: step.to_string(),
        });
    }
    approval_grants
        .grant(&GrantRequest {
            task_id: task.as_str(),
            step_id: step.as_str(),
            scope: ApprovalScope::Once,
            now_ms,
            ttl_ms,
            uses: 1,
        })
        .map_err(|error| {
            BrowserTaskError::Runtime(RuntimeExecutionError::Policy {
                reason: format!("cannot record human approval: {error}"),
            })
        })
}
