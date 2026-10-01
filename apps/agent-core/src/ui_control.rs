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

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use assistant_storage::Clock;
use assistant_task_engine::{CheckpointStore, StepId, TaskEngine, TaskEngineError, TaskId};

use crate::ui_ipc::{
    UiAuthorizationScope, UiCommand, UiCommandError, UiCommandHandler, UiCommandOutcome,
};

use assistant_hitl::ApprovalScope;

use crate::approval_grants::{ApprovalGrants, GrantRequest};

/// One approval the runtime is waiting on.
#[derive(Debug, Clone)]
pub struct PendingApproval {
    task_id: TaskId,
    step_id: StepId,
    scopes: Vec<UiAuthorizationScope>,
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
}

/// Shared registry of approval requests the runtime is waiting on.
#[derive(Debug, Default)]
pub struct PendingApprovals {
    entries: Mutex<BTreeMap<String, PendingApproval>>,
}

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
        let request_id = request_id.into();
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
        let mut entries = self.entries.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        entries.insert(
            request_id,
            PendingApproval {
                task_id,
                step_id,
                scopes,
            },
        );
        drop(entries);
        Ok(())
    }

    fn get(&self, request_id: &str) -> Result<Option<PendingApproval>, UiCommandError> {
        let entries = self.entries.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        Ok(entries.get(request_id).cloned())
    }

    fn remove(&self, request_id: &str) -> Result<Option<PendingApproval>, UiCommandError> {
        let mut entries = self.entries.lock().map_err(|_| UiCommandError::Handler {
            reason: "the pending approval registry is unavailable".to_owned(),
        })?;
        let entry = entries.remove(request_id);
        drop(entries);
        Ok(entry)
    }
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
        let (task_id, step_id) = {
            let pending =
                self.pending
                    .get(&request_id)?
                    .ok_or_else(|| UiCommandError::UnknownApproval {
                        request_id: request_id.clone(),
                    })?;
            if !pending.scopes.contains(&scope) {
                return Err(UiCommandError::ScopeNotOffered { request_id, scope });
            }
            (pending.task_id, pending.step_id)
        };
        let granted = match scope {
            UiAuthorizationScope::Once => ApprovalScope::Once,
            UiAuthorizationScope::ThisStepPattern => ApprovalScope::ThisStepPattern,
            UiAuthorizationScope::ThisTask => ApprovalScope::ThisTask,
            UiAuthorizationScope::ThisAppSession => ApprovalScope::ThisAppSession,
            UiAuthorizationScope::Persistent => {
                return Err(UiCommandError::Handler {
                    reason: "a standing approval cannot be granted to a runtime step".to_owned(),
                });
            }
        };
        self.approvals
            .grant(&GrantRequest {
                task_id: task_id.as_str(),
                step_id: step_id.as_str(),
                scope: granted,
                now_ms,
                ttl_ms: APPROVAL_TTL_MS,
                uses: 1,
            })
            .map_err(|error| UiCommandError::Handler {
                reason: error.to_string(),
            })?;
        let _ = self.pending.remove(&request_id)?;
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
