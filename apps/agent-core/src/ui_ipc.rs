//! UI-facing command and event contract for the binary layer.
//!
//! Responsibilities:
//! - define the versioned command envelope the desktop UI may send to Core;
//! - reject unknown fields, unknown kinds, and version drift before dispatch;
//! - project task/step state into the structured events the UI timeline renders.
//!
//! Boundaries:
//! - does not decide policy, approval, or authorization semantics;
//! - does not touch the platform layer, IPC transport, or the database;
//! - the UI stays an untrusted input source: nothing is dispatched before
//!   [`parse_ui_command`] accepts it.
//!
//! Invariants:
//! - a rejected command never reaches a [`UiCommandHandler`];
//! - every rejection carries a stable [`ErrorCode`];
//! - the wire shape is a flat, versioned object so both sides can validate it.

use assistant_protocol::ErrorCode;
use assistant_task_engine::{StepPhase, TaskSnapshot, TaskStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

/// Contract version understood by both the UI client and Core.
pub const UI_IPC_VERSION: &str = "1.0";

/// Authorization scope a user grants for one approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAuthorizationScope {
    /// One single execution.
    Once,
    /// Every matching step of the current plan.
    ThisStepPattern,
    /// Every step of the current task.
    ThisTask,
    /// Every task against the current application session.
    ThisAppSession,
    /// Persisted until the user revokes it.
    Persistent,
}

impl UiAuthorizationScope {
    /// Returns the stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::ThisStepPattern => "this_step_pattern",
            Self::ThisTask => "this_task",
            Self::ThisAppSession => "this_app_session",
            Self::Persistent => "persistent",
        }
    }
}

/// Command the desktop UI may send to Core.
///
/// The wire shape is the envelope documented by [`parse_ui_command`]:
/// `{"version": "1.0", "command": {"kind": "approve_request", ...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiCommand {
    /// Records a user intent to start work toward a goal.
    SubmitIntent {
        /// Stable identifier for this intent.
        intent_id: String,
        /// Natural-language goal; Core, not the UI, turns it into a plan.
        goal: String,
    },
    /// Approves one pending approval request.
    ApproveRequest {
        /// Approval request identifier shown on the card.
        request_id: String,
        /// Scope the user selected; must be one Core offered.
        scope: UiAuthorizationScope,
    },
    /// Denies one pending approval request and stops its step.
    DenyRequest {
        /// Approval request identifier shown on the card.
        request_id: String,
        /// Human-readable reason; required so denials are auditable.
        reason: String,
    },
    /// Pauses a running task at the next step boundary.
    PauseTask {
        /// Task identifier.
        task_id: String,
    },
    /// Cancels a task.
    CancelTask {
        /// Task identifier.
        task_id: String,
    },
    /// Takes control away from automation.
    TakeOverTask {
        /// Task identifier.
        task_id: String,
    },
}

/// Structured rejection of a UI command.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UiCommandError {
    /// The payload was not a JSON object.
    #[error("UI command must be a JSON object")]
    NotAnObject,
    /// The envelope omitted `version`.
    #[error("UI command is missing the contract version")]
    MissingVersion,
    /// The envelope declared an unsupported version.
    #[error("unsupported UI IPC version {found:?} (expected {UI_IPC_VERSION})")]
    UnsupportedVersion {
        /// Version found on the wire.
        found: String,
    },
    /// The envelope omitted `command`.
    #[error("UI command is missing the `command` object")]
    MissingCommand,
    /// The command omitted its `kind` discriminator.
    #[error("UI command is missing the `kind` discriminator")]
    MissingKind,
    /// The command used an unknown kind.
    #[error("unknown UI command kind {kind:?}")]
    UnknownKind {
        /// Offending kind.
        kind: String,
    },
    /// The command carried a field that is not part of the contract.
    #[error("unknown UI command field {field:?}")]
    UnknownField {
        /// Offending field name.
        field: String,
    },
    /// A required field had the wrong type or shape.
    #[error("UI command payload is invalid: {reason}")]
    InvalidPayload {
        /// Parser detail.
        reason: String,
    },
    /// A required text field was empty or whitespace.
    #[error("UI command field {field} must be a non-empty string")]
    EmptyField {
        /// Offending field name.
        field: &'static str,
    },
    /// The command referenced a task Core does not know.
    #[error("task {task_id} is not known to Core")]
    UnknownTask {
        /// Offending task identifier.
        task_id: String,
    },
    /// The command referenced an approval that is not pending.
    #[error("approval request {request_id} is not pending")]
    UnknownApproval {
        /// Offending request identifier.
        request_id: String,
    },
    /// The approval did not offer the requested scope.
    #[error("approval request {request_id} does not offer scope {scope:?}")]
    ScopeNotOffered {
        /// Approval request identifier.
        request_id: String,
        /// Scope the UI asked for.
        scope: UiAuthorizationScope,
    },
    /// The task engine rejected the state transition.
    #[error("task engine rejected the UI command: {reason}")]
    TaskEngine {
        /// Engine detail.
        reason: String,
    },
    /// The injected handler failed.
    #[error("UI command handler failed: {reason}")]
    Handler {
        /// Handler detail.
        reason: String,
    },
}

impl UiCommandError {
    /// Maps the rejection onto a stable protocol error code.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::NotAnObject
            | Self::MissingVersion
            | Self::UnsupportedVersion { .. }
            | Self::MissingCommand
            | Self::MissingKind
            | Self::UnknownKind { .. }
            | Self::UnknownField { .. }
            | Self::InvalidPayload { .. }
            | Self::EmptyField { .. } => ErrorCode::ToolInvalidArgs,
            Self::UnknownTask { .. } => ErrorCode::TargetNotFound,
            Self::UnknownApproval { .. } | Self::ScopeNotOffered { .. } => {
                ErrorCode::UserInteraction
            }
            Self::TaskEngine { .. } | Self::Handler { .. } => ErrorCode::Fatal,
        }
    }
}

/// Outcome of one accepted UI command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UiCommandOutcome {
    /// The intent was recorded; planning stays in Core.
    IntentAccepted {
        /// Intent identifier.
        intent_id: String,
    },
    /// The approval was granted with the requested scope.
    ApprovalGranted {
        /// Approval request identifier.
        request_id: String,
        /// Scope granted.
        scope: UiAuthorizationScope,
    },
    /// The approval was denied and its step was stopped.
    ApprovalDenied {
        /// Approval request identifier.
        request_id: String,
        /// Reason recorded for the audit trail.
        reason: String,
        /// Task status after the denial.
        task_status: String,
    },
    /// The task is now paused or pause-requested.
    TaskPaused {
        /// Task identifier.
        task_id: String,
        /// Task status after the command.
        task_status: String,
    },
    /// The task is now cancelled or cancel-requested.
    TaskCancelled {
        /// Task identifier.
        task_id: String,
        /// Task status after the command.
        task_status: String,
    },
    /// The task is now under user control.
    TaskTakenOver {
        /// Task identifier.
        task_id: String,
        /// Task status after the command.
        task_status: String,
    },
}

/// Event projected from Core state for the UI timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiEvent {
    /// The task-level status changed.
    TaskStateChanged {
        /// Task identifier.
        task_id: String,
        /// Wire name of the task status.
        status: String,
        /// Present when Core is holding for a human decision.
        hold_reason: Option<String>,
    },
    /// One step's status changed; committed steps carry their verification
    /// fingerprint so the timeline shows a real result instead of a guess.
    StepStateChanged {
        /// Task identifier.
        task_id: String,
        /// Step identifier.
        step_id: String,
        /// Wire name of the step status.
        status: String,
        /// Watched phase while the step is in flight.
        phase: Option<String>,
        /// Post-step fingerprint recorded after verification.
        post_fingerprint: Option<String>,
    },
}

/// Boundary the binary assembly implements to apply one validated command.
pub trait UiCommandHandler {
    /// Applies one already-validated command.
    ///
    /// # Errors
    ///
    /// Returns a structured [`UiCommandError`] when the command cannot be
    /// applied; the caller must surface it instead of reporting success.
    fn handle(&mut self, command: UiCommand) -> Result<UiCommandOutcome, UiCommandError>;
}

/// Validates a raw UI payload and dispatches it to the handler.
///
/// Parsing happens first and in full, so an illegal payload can never reach the
/// handler and therefore cannot cause a side effect.
///
/// # Errors
///
/// Returns [`UiCommandError`] for malformed envelopes, unknown kinds, unknown
/// fields, version drift, or handler rejection.
pub fn dispatch_ui_command<H: UiCommandHandler>(
    handler: &mut H,
    value: &Value,
) -> Result<UiCommandOutcome, UiCommandError> {
    let command = parse_ui_command(value)?;
    handler.handle(command)
}

/// Parses and validates one raw UI command payload.
///
/// # Errors
///
/// Returns [`UiCommandError`] when the envelope is not an object, omits
/// `version` / `command` / `kind`, declares an unknown version or kind, carries
/// an unknown field, or fails shape validation.
pub fn parse_ui_command(value: &Value) -> Result<UiCommand, UiCommandError> {
    let envelope = value.as_object().ok_or(UiCommandError::NotAnObject)?;
    reject_unknown_fields(envelope, &["version", "command"])?;
    let version = envelope
        .get("version")
        .and_then(Value::as_str)
        .ok_or(UiCommandError::MissingVersion)?;
    if version != UI_IPC_VERSION {
        return Err(UiCommandError::UnsupportedVersion {
            found: version.to_owned(),
        });
    }
    let command_value = envelope
        .get("command")
        .ok_or(UiCommandError::MissingCommand)?;
    let command_object = command_value
        .as_object()
        .ok_or(UiCommandError::MissingCommand)?;
    let kind = command_object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(UiCommandError::MissingKind)?;
    let allowed = allowed_command_fields(kind).ok_or_else(|| UiCommandError::UnknownKind {
        kind: kind.to_owned(),
    })?;
    reject_unknown_fields(command_object, allowed)?;
    let command: UiCommand = serde_json::from_value(command_value.clone()).map_err(|error| {
        UiCommandError::InvalidPayload {
            reason: error.to_string(),
        }
    })?;
    validate_command(&command)?;
    Ok(command)
}

/// Projects the latest task snapshot into UI timeline events.
#[must_use]
pub fn project_snapshot_events(snapshot: &TaskSnapshot) -> Vec<UiEvent> {
    let task_id = snapshot.task_id.to_string();
    let mut events = vec![UiEvent::TaskStateChanged {
        task_id: task_id.clone(),
        status: snapshot.status.as_str().to_owned(),
        hold_reason: snapshot.hold_reason.as_ref().map(describe_hold_reason),
    }];
    for step in &snapshot.steps {
        events.push(UiEvent::StepStateChanged {
            task_id: task_id.clone(),
            step_id: step.id.to_string(),
            status: step.status.as_str().to_owned(),
            phase: step.watched_phase().map(step_phase_name),
            post_fingerprint: step.post_fingerprint.clone(),
        });
    }
    events
}

/// Returns the field allow-list for one command kind, `kind` included.
fn allowed_command_fields(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "submit_intent" => Some(&["kind", "intent_id", "goal"]),
        "approve_request" => Some(&["kind", "request_id", "scope"]),
        "deny_request" => Some(&["kind", "request_id", "reason"]),
        "pause_task" | "cancel_task" | "take_over_task" => Some(&["kind", "task_id"]),
        _ => None,
    }
}

/// Rejects any key that is not on the allow-list.
fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), UiCommandError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(UiCommandError::UnknownField { field: key.clone() });
        }
    }
    Ok(())
}

/// Rejects empty or whitespace-only identifiers and text.
fn validate_command(command: &UiCommand) -> Result<(), UiCommandError> {
    match command {
        UiCommand::SubmitIntent { intent_id, goal } => {
            require_text(intent_id, "intent_id")?;
            require_text(goal, "goal")
        }
        UiCommand::ApproveRequest { request_id, .. } => require_text(request_id, "request_id"),
        UiCommand::DenyRequest { request_id, reason } => {
            require_text(request_id, "request_id")?;
            require_text(reason, "reason")
        }
        UiCommand::PauseTask { task_id }
        | UiCommand::CancelTask { task_id }
        | UiCommand::TakeOverTask { task_id } => require_text(task_id, "task_id"),
    }
}

fn require_text(value: &str, field: &'static str) -> Result<(), UiCommandError> {
    if value.trim().is_empty() {
        return Err(UiCommandError::EmptyField { field });
    }
    Ok(())
}

fn describe_hold_reason(reason: &assistant_task_engine::TaskHoldReason) -> String {
    match reason {
        assistant_task_engine::TaskHoldReason::ExternalBlocked { message } => message.clone(),
        other => format!("{other:?}"),
    }
}

/// Returns the wire name of a task status for event projection.
#[must_use]
pub const fn task_status_name(status: TaskStatus) -> &'static str {
    status.as_str()
}

fn step_phase_name(phase: StepPhase) -> String {
    match phase {
        StepPhase::Resolve => "resolve",
        StepPhase::Execute => "execute",
        StepPhase::Verify => "verify",
        // `StepPhase` is non-exhaustive: a future phase must not be silently
        // reported as one of today's phases.
        _ => "unknown",
    }
    .to_owned()
}
