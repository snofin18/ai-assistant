//! Projects a validated browser form preview into the UI approval event.
//!
//! Responsibilities:
//! - turn already-redacted field diffs into the closed `step_approval_requested` shape;
//! - preserve the validated instruction attribution without reinterpreting it;
//! - bound event lifetime and expose only form origin/action evidence.
//!
//! Boundaries:
//! - does not register pending approvals, grant authorization, or submit the form;
//! - does not read raw field values, DOM, title, cookies, or tool evidence.
//!
//! Invariants:
//! 1. the event always offers exactly one `once` scope;
//! 2. the action is a non-empty bounded identifier;
//! 3. expiry is a checked positive Unix-millisecond timestamp.

use std::sync::{Arc, Mutex};

use assistant_agent_core::{
    RuntimeExecutionError, UiApprovalDiff, UiApprovalFieldDiff, UiApprovalReversibility,
    UiApprovalRisk, UiAuthorizationScope, UiEvent, UiFormEvidence, UiInstructionAttribution,
};
use assistant_hitl::DiffPreview;
use assistant_task_engine::{PlanStep, StepId, TaskId};

use super::super::{BrowserInvocationRecorder, BrowserTaskError, preview_runtime_error};
use super::{FormApprovalPreview, FormApprovalPreviewError};

const APPROVAL_TTL_MS: i64 = 300_000;
const MAX_ACTION_BYTES: usize = 512;

impl FormApprovalPreview {
    /// Builds one UI approval event for the following form submit step.
    ///
    /// # Errors
    ///
    /// Returns [`FormApprovalPreviewError::InvalidEvent`] when the action is
    /// blank, oversized, contains unsupported whitespace/control characters, or
    /// the expiry timestamp overflows.
    pub(crate) fn to_ui_event(
        &self,
        task_id: &TaskId,
        submit_step_id: &StepId,
        action: &str,
        now_ms: i64,
    ) -> Result<UiEvent, FormApprovalPreviewError> {
        validate_action(action)?;
        let expires_at_ms = now_ms
            .checked_add(APPROVAL_TTL_MS)
            .ok_or_else(|| invalid_event("approval expiry overflowed"))?;
        if expires_at_ms <= now_ms {
            return Err(invalid_event("approval expiry must be in the future"));
        }
        let DiffPreview::Fields(fields) = &self.diff else {
            return Err(invalid_event(
                "form approval preview must contain a field diff",
            ));
        };
        let entries = fields
            .iter()
            .map(|field| UiApprovalFieldDiff {
                field: field.field.clone(),
                before: field.before.clone(),
                after: field.after.clone(),
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(invalid_event("approval event field diff must not be empty"));
        }
        Ok(UiEvent::StepApprovalRequested {
            request_id: format!("runtime:{}:{}", task_id.as_str(), submit_step_id.as_str()),
            task_id: task_id.to_string(),
            step_id: submit_step_id.to_string(),
            action: action.to_owned(),
            target: self.form_origin.clone(),
            summary: format!(
                "Submit {} changed form field(s) to {}",
                entries.len(),
                self.form_action
            ),
            impact: format!(
                "Submitting to {} cannot be automatically undone.",
                self.form_action
            ),
            diff: Box::new(UiApprovalDiff::Fields { entries }),
            instruction_attribution: Box::new(UiInstructionAttribution {
                origin: self.attribution.origin().stable_token().to_owned(),
                parent_goal: self.attribution.parent_goal().map(str::to_owned),
                source_ref: self.attribution.source_ref().map(str::to_owned),
            }),
            risk: UiApprovalRisk::High,
            reversibility: UiApprovalReversibility::L3Irreversible,
            scope_options: vec![UiAuthorizationScope::Once],
            evidence: UiFormEvidence {
                form_origin: self.form_origin.clone(),
                form_action: self.form_action.clone(),
            },
            expires_at_ms,
        })
    }
}

impl BrowserInvocationRecorder {
    /// Builds the submit approval event before policy evaluates that step.
    pub(crate) fn prepare_form_submit_approval(
        &mut self,
        task_id: &TaskId,
        step: &PlanStep,
        now_ms: i64,
    ) -> Result<(), RuntimeExecutionError> {
        if !is_form_submit_tool(&step.tool) {
            return Ok(());
        }
        let preview = self.pending_form_approval.take().ok_or_else(|| {
            preview_runtime_error(
                step.id.as_str(),
                "form_submit has no preceding form_fill approval preview",
            )
        })?;
        let event = preview
            .to_ui_event(task_id, &step.id, &step.tool, now_ms)
            .map_err(|error| preview_runtime_error(step.id.as_str(), error))?;
        self.approval_events.insert(step.id.to_string(), event);
        Ok(())
    }
}

pub fn recorded_approval_events(
    recorder: &Arc<Mutex<BrowserInvocationRecorder>>,
) -> Result<Vec<UiEvent>, BrowserTaskError> {
    recorder
        .lock()
        .map(|recorder| recorder.approval_events.values().cloned().collect())
        .map_err(|_| BrowserTaskError::ToolBus {
            reason: "browser task invocation recorder is unavailable".to_owned(),
        })
}

fn is_form_submit_tool(tool: &str) -> bool {
    matches!(tool, "edge.cdp.form_submit" | "chrome.cdp.form_submit")
}

fn validate_action(action: &str) -> Result<(), FormApprovalPreviewError> {
    if action.trim().is_empty() {
        return Err(invalid_event("approval action must not be empty"));
    }
    if action.len() > MAX_ACTION_BYTES {
        return Err(invalid_event("approval action exceeds 512 bytes"));
    }
    if action
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(invalid_event(
            "approval action contains unsupported whitespace or a control character",
        ));
    }
    Ok(())
}

fn invalid_event(reason: impl Into<String>) -> FormApprovalPreviewError {
    FormApprovalPreviewError::InvalidEvent {
        reason: reason.into(),
    }
}
