//! Core-side application of validated UI commands to the task engine.
//!
//! Responsibilities:
//! - apply approve / deny / pause / cancel / take-over commands to the single
//!   task engine owned by the binary layer;
//! - keep the pending-approval registry the runtime executor publishes into;
//! - translate engine failures into stable [`UiCommandError`] values.
//!
//! Boundaries:
//! - does not parse or trust raw payloads (that is [`crate::ui_ipc`]);
//! - does not run tools, resolve targets, or decide policy;
//! - does not write SQL; checkpoints go through the engine's store.
//!
//! Invariants:
//! - a denial always stops the referenced step before reporting success;
//! - an approval is only granted for a scope Core actually offered;
//! - timestamps come from the injected clock, never from wall-clock calls here.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use assistant_storage::Clock;
use assistant_task_engine::{CheckpointStore, StepId, TaskEngine, TaskEngineError, TaskId};

use crate::ui_ipc::{
    UiApprovalDiff, UiApprovalReversibility, UiApprovalRisk, UiAuthorizationScope, UiCommand,
    UiCommandError, UiCommandHandler, UiCommandOutcome, UiEvent,
};

use assistant_hitl::ApprovalScope;

use crate::approval_grants::{ApprovalGrants, GrantRequest};

/// One approval the runtime is waiting on.
#[derive(Debug, Clone)]
pub struct PendingApproval {
    task_id: TaskId,
    step_id: StepId,
    scopes: Vec<UiAuthorizationScope>,
    expires_at_ms: Option<i64>,
}

impl PendingApproval {
    /// Task that owns the approval request.
    #[must_use]
    pub const fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Step that must wait for the decision.
    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    /// Scopes the runtime actually offered to the user.
    #[must_use]
    pub fn scopes(&self) -> &[UiAuthorizationScope] {
        &self.scopes
    }

    /// Returns the event expiry when this request came from a UI approval event.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }

    fn is_expired(&self, now_ms: i64) -> bool {
        self.expires_at_ms.is_some_and(|expires| now_ms >= expires)
    }

    fn grant_ttl_ms(&self, now_ms: i64) -> i64 {
        self.expires_at_ms
            .map_or(APPROVAL_TTL_MS, |expires| expires.saturating_sub(now_ms))
    }
}

/// Shared registry of approval requests the runtime is waiting on.
#[derive(Debug, Default)]
pub struct PendingApprovals {
    state: Mutex<PendingState>,
}

#[derive(Debug, Default)]
struct PendingState {
    entries: BTreeMap<String, PendingApproval>,
    events: VecDeque<UiEvent>,
}

/// Hard cap for pending decisions and queued approval events.
///
/// Reaching the cap rejects a new request instead of silently dropping a
/// decision that a human may still be reviewing.
pub const MAX_PENDING_APPROVALS: usize = 1024;

impl PendingApprovals {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one pending approval.
    ///
    /// # Errors
    ///
    /// Returns [`UiCommandError::EmptyField`] for a blank request id and
    /// [`UiCommandError::Handler`] when no scope is offered, because an
    /// approval with no selectable scope can never be answered.
    pub fn register(
        &self,
        request_id: impl Into<String>,
        task_id: TaskId,
        step_id: StepId,
        scopes: Vec<UiAuthorizationScope>,
    ) -> Result<(), UiCommandError> {
        self.register_internal(request_id.into(), task_id, step_id, scopes, None)
    }

    /// Registers a validated browser-form approval event.
    ///
    /// The event is accepted only when its request identity, risk,
    /// reversibility, scope and expiry match the closed T5.2 contract.
    ///
    /// # Errors
    ///
    /// Returns a handler error when the event is not a valid
    /// `step_approval_requested`, has expired, duplicates an existing request,
    /// or either bounded queue is full.
    pub fn register_event(&self, event: &UiEvent, now_ms: i64) -> Result<(), UiCommandError> {
        let (request_id, task, step, scopes) = validate_approval_event(event, now_ms)?;
        self.register_internal(request_id, task, step, scopes, Some(event.clone()))
    }

    fn register_internal(
        &self,
        request_id: String,
        task_id: TaskId,
        step_id: StepId,
        scopes: Vec<UiAuthorizationScope>,
        event: Option<UiEvent>,
    ) -> Result<(), UiCommandError> {
        if request_id.trim().is_empty() {
            return Err(UiCommandError::EmptyField {
                field: "request_id",
            });
        }
        if scopes.is_empty() {
            return Err(UiCommandError::Handler {
                reason: "a pending approval must offer at least one scope".to_owned(),
            });
        }
        let mut state = self.state.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        if state.entries.contains_key(&request_id) {
            return Err(UiCommandError::Handler {
                reason: format!("approval request `{request_id}` is already pending"),
            });
        }
        if state.entries.len() >= MAX_PENDING_APPROVALS {
            return Err(UiCommandError::Handler {
                reason: "the pending approval registry is full".to_owned(),
            });
        }
        if event.is_some() && state.events.len() >= MAX_PENDING_APPROVALS {
            return Err(UiCommandError::Handler {
                reason: "the pending approval event queue is full".to_owned(),
            });
        }
        state.entries.insert(
            request_id,
            PendingApproval {
                task_id,
                step_id,
                scopes,
                expires_at_ms: event.as_ref().and_then(event_expiry),
            },
        );
        if let Some(event) = event {
            state.events.push_back(event);
        }
        drop(state);
        Ok(())
    }

    fn get(&self, request_id: &str) -> Result<Option<PendingApproval>, UiCommandError> {
        let state = self.state.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        Ok(state.entries.get(request_id).cloned())
    }

    fn remove(&self, request_id: &str) -> Result<Option<PendingApproval>, UiCommandError> {
        let mut state = self.state.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        let entry = state.entries.remove(request_id);
        state.events.retain(|event| match event {
            UiEvent::StepApprovalRequested {
                request_id: queued, ..
            } => queued != request_id,
            _ => true,
        });
        drop(state);
        Ok(entry)
    }

    /// Drains queued approval events without removing their pending decisions.
    ///
    /// # Errors
    ///
    /// Returns a handler error when the registry lock is unavailable.
    pub fn drain_events(&self) -> Result<Vec<UiEvent>, UiCommandError> {
        let mut state = self.state.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        Ok(state.events.drain(..).collect())
    }

    pub(crate) fn drain_events_recovering_poison(&self) -> Vec<UiEvent> {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.events.drain(..).collect()
    }
}

fn validate_approval_event(
    event: &UiEvent,
    now_ms: i64,
) -> Result<(String, TaskId, StepId, Vec<UiAuthorizationScope>), UiCommandError> {
    let UiEvent::StepApprovalRequested {
        request_id,
        task_id,
        step_id,
        action,
        target,
        summary,
        impact,
        diff,
        instruction_attribution,
        risk,
        reversibility,
        scope_options,
        evidence,
        expires_at_ms,
    } = event
    else {
        return Err(handler_error(
            "only step_approval_requested events can be registered",
        ));
    };
    let required = [
        request_id.as_str(),
        task_id.as_str(),
        step_id.as_str(),
        action.as_str(),
        target.as_str(),
        summary.as_str(),
        impact.as_str(),
        instruction_attribution.origin.as_str(),
        evidence.form_origin.as_str(),
        evidence.form_action.as_str(),
    ];
    if required.iter().any(|value| value.trim().is_empty()) {
        return Err(handler_error(
            "approval event contains an empty required field",
        ));
    }
    let task = TaskId::new(task_id.clone())
        .map_err(|error| handler_error(format!("approval event task id is invalid: {error}")))?;
    let step = StepId::new(step_id.clone())
        .map_err(|error| handler_error(format!("approval event step id is invalid: {error}")))?;
    let expected_request_id = format!("runtime:{}:{}", task.as_str(), step.as_str());
    if request_id != &expected_request_id {
        return Err(handler_error(
            "approval event request id does not match task and step",
        ));
    }
    validate_approval_policy(
        *risk,
        *reversibility,
        scope_options,
        diff,
        *expires_at_ms,
        now_ms,
    )?;
    Ok((request_id.clone(), task, step, scope_options.clone()))
}

fn validate_approval_policy(
    risk: UiApprovalRisk,
    reversibility: UiApprovalReversibility,
    scope_options: &[UiAuthorizationScope],
    diff: &UiApprovalDiff,
    expires_at_ms: i64,
    now_ms: i64,
) -> Result<(), UiCommandError> {
    if risk != UiApprovalRisk::High {
        return Err(handler_error("browser form approval must carry high risk"));
    }
    if reversibility != UiApprovalReversibility::L3Irreversible {
        return Err(handler_error(
            "browser form approval must be L3 irreversible",
        ));
    }
    if scope_options != [UiAuthorizationScope::Once] {
        return Err(handler_error(
            "browser form approval must offer exactly once",
        ));
    }
    if expires_at_ms <= now_ms {
        return Err(handler_error("browser form approval event has expired"));
    }
    let UiApprovalDiff::Fields { entries } = diff;
    if entries.is_empty() || entries.iter().any(|entry| entry.field.trim().is_empty()) {
        return Err(handler_error(
            "browser form approval must contain a non-empty field diff",
        ));
    }
    Ok(())
}

const fn event_expiry(event: &UiEvent) -> Option<i64> {
    match event {
        UiEvent::StepApprovalRequested { expires_at_ms, .. } => Some(*expires_at_ms),
        _ => None,
    }
}

fn handler_error(reason: impl Into<String>) -> UiCommandError {
    UiCommandError::Handler {
        reason: reason.into(),
    }
}

fn pending_approval_target(
    pending_approvals: &PendingApprovals,
    request_id: &str,
    scope: UiAuthorizationScope,
    now_ms: i64,
) -> Result<(TaskId, StepId, i64), UiCommandError> {
    let pending =
        pending_approvals
            .get(request_id)?
            .ok_or_else(|| UiCommandError::UnknownApproval {
                request_id: request_id.to_owned(),
            })?;
    if !pending.scopes.contains(&scope) {
        return Err(UiCommandError::ScopeNotOffered {
            request_id: request_id.to_owned(),
            scope,
        });
    }
    if pending.is_expired(now_ms) {
        let _ = pending_approvals.remove(request_id)?;
        return Err(UiCommandError::UnknownApproval {
            request_id: request_id.to_owned(),
        });
    }
    let ttl_ms = pending.grant_ttl_ms(now_ms);
    Ok((pending.task_id, pending.step_id, ttl_ms))
}

fn approve_pending_request(
    pending_approvals: &PendingApprovals,
    approvals: &ApprovalGrants,
    request_id: &str,
    scope: UiAuthorizationScope,
    now_ms: i64,
) -> Result<(), UiCommandError> {
    let (task_id, step_id, ttl_ms) =
        pending_approval_target(pending_approvals, request_id, scope, now_ms)?;
    let granted = match scope {
        UiAuthorizationScope::Once => ApprovalScope::Once,
        UiAuthorizationScope::ThisStepPattern => ApprovalScope::ThisStepPattern,
        UiAuthorizationScope::ThisTask => ApprovalScope::ThisTask,
        UiAuthorizationScope::ThisAppSession => ApprovalScope::ThisAppSession,
        UiAuthorizationScope::Persistent => {
            return Err(handler_error(
                "a standing approval cannot be granted to a runtime step",
            ));
        }
    };
    approvals
        .grant(&GrantRequest {
            task_id: task_id.as_str(),
            step_id: step_id.as_str(),
            scope: granted,
            now_ms,
            ttl_ms,
            uses: 1,
        })
        .map_err(|error| handler_error(error.to_string()))?;
    let _ = pending_approvals.remove(request_id)?;
    Ok(())
}

/// Applies UI commands to the task engine owned by the binary layer.
pub struct TaskControlHandler<Store> {
    engine: TaskEngine<Store>,
    clock: Arc<dyn Clock>,
    pending: Arc<PendingApprovals>,
    approvals: Arc<ApprovalGrants>,
}

/// How long a UI approval stays usable. Bounded on purpose: an approval the user
/// gave for one step must not linger into an unrelated later action.
const APPROVAL_TTL_MS: i64 = 300_000;

impl<Store> TaskControlHandler<Store> {
    /// Creates a handler over the assembly-owned engine and clock.
    #[must_use]
    pub fn new(engine: TaskEngine<Store>, clock: Arc<dyn Clock>) -> Self {
        Self {
            engine,
            clock,
            pending: Arc::new(PendingApprovals::new()),
            approvals: Arc::new(ApprovalGrants::new()),
        }
    }

    /// Shares the runtime-owned pending registry, so an authorization card is
    /// visible before the human responds.
    #[must_use]
    pub fn with_pending(mut self, pending: Arc<PendingApprovals>) -> Self {
        self.pending = pending;
        self
    }

    /// Shares the assembly-owned approval table, so a UI decision is visible to
    /// the runtime step that is waiting for it.
    #[must_use]
    pub fn with_approvals(mut self, approvals: Arc<ApprovalGrants>) -> Self {
        self.approvals = approvals;
        self
    }

    /// Registers a pending approval published by the runtime executor.
    ///
    /// # Errors
    ///
    /// Returns [`UiCommandError::EmptyField`] for a blank request id and
    /// [`UiCommandError::Handler`] when no scope is offered, because an
    /// approval with no selectable scope can never be answered.
    pub fn register_pending_approval(
        &self,
        request_id: impl Into<String>,
        task_id: TaskId,
        step_id: StepId,
        scopes: Vec<UiAuthorizationScope>,
    ) -> Result<(), UiCommandError> {
        self.pending.register(request_id, task_id, step_id, scopes)
    }

    /// Registers a validated browser-form approval event and its pending decision.
    ///
    /// # Errors
    ///
    /// Returns the same fail-closed errors as [`PendingApprovals::register_event`].
    pub fn register_pending_approval_event(
        &self,
        event: &UiEvent,
        now_ms: i64,
    ) -> Result<(), UiCommandError> {
        self.pending.register_event(event, now_ms)
    }

    /// Records the human decision so the waiting runtime step can proceed **once**.
    ///
    /// A standing scope is refused: the grant table will not hold one, and a
    /// step-level approval must not become a permanent authorization.
    ///
    /// # Errors
    ///
    /// Returns [`UiCommandError::UnknownApproval`] for an id that is not pending,
    /// [`UiCommandError::ScopeNotOffered`] when the scope was never offered, and
    /// [`UiCommandError::Handler`] when a standing scope is requested or the
    /// grant table cannot record the decision.
    fn approve_request(
        &self,
        request_id: String,
        scope: UiAuthorizationScope,
        now_ms: i64,
    ) -> Result<UiCommandOutcome, UiCommandError> {
        approve_pending_request(&self.pending, &self.approvals, &request_id, scope, now_ms)?;
        Ok(UiCommandOutcome::ApprovalGranted { request_id, scope })
    }

    /// Returns the engine so the assembly can hand it back to the executor.
    #[must_use]
    pub fn into_engine(self) -> TaskEngine<Store> {
        self.engine
    }
}

impl<Store: CheckpointStore> UiCommandHandler for TaskControlHandler<Store> {
    fn handle(&mut self, command: UiCommand) -> Result<UiCommandOutcome, UiCommandError> {
        let now_ms = self.clock.now_unix_ms();
        match command {
            UiCommand::SubmitIntent { intent_id, .. } => {
                Ok(UiCommandOutcome::IntentAccepted { intent_id })
            }
            UiCommand::ApproveRequest { request_id, scope } => {
                self.approve_request(request_id, scope, now_ms)
            }
            UiCommand::DenyRequest { request_id, reason } => {
                let pending = self.pending.remove(&request_id)?.ok_or_else(|| {
                    UiCommandError::UnknownApproval {
                        request_id: request_id.clone(),
                    }
                })?;
                let snapshot = self
                    .engine
                    .deny_step(&pending.task_id, &pending.step_id, now_ms)
                    .map_err(task_engine_error)?;
                Ok(UiCommandOutcome::ApprovalDenied {
                    request_id,
                    reason,
                    task_status: snapshot.status.as_str().to_owned(),
                })
            }
            UiCommand::PauseTask { task_id } => {
                let task_id = parse_task_id(&task_id)?;
                let snapshot = self
                    .engine
                    .request_pause(&task_id, now_ms)
                    .map_err(task_engine_error)?;
                Ok(UiCommandOutcome::TaskPaused {
                    task_id: task_id.to_string(),
                    task_status: snapshot.status.as_str().to_owned(),
                })
            }
            UiCommand::CancelTask { task_id } => {
                let task_id = parse_task_id(&task_id)?;
                let snapshot = self
                    .engine
                    .request_cancel(&task_id, now_ms)
                    .map_err(task_engine_error)?;
                Ok(UiCommandOutcome::TaskCancelled {
                    task_id: task_id.to_string(),
                    task_status: snapshot.status.as_str().to_owned(),
                })
            }
            UiCommand::TakeOverTask { task_id } => {
                let task_id = parse_task_id(&task_id)?;
                let snapshot = self
                    .engine
                    .take_over(&task_id, now_ms)
                    .map_err(task_engine_error)?;
                Ok(UiCommandOutcome::TaskTakenOver {
                    task_id: task_id.to_string(),
                    task_status: snapshot.status.as_str().to_owned(),
                })
            }
        }
    }
}

fn parse_task_id(value: &str) -> Result<TaskId, UiCommandError> {
    TaskId::new(value).map_err(|error| UiCommandError::InvalidPayload {
        reason: error.to_string(),
    })
}

fn task_engine_error(error: TaskEngineError) -> UiCommandError {
    match error {
        TaskEngineError::TaskNotFound { task_id } => UiCommandError::UnknownTask { task_id },
        other => UiCommandError::TaskEngine {
            reason: other.to_string(),
        },
    }
}
