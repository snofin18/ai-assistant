//! Explicit per-step approvals for the private browser task runner.
//!
//! Responsibilities:
//! - validate operator-supplied step ids against the generated Plan and catalog;
//! - grant one bounded, single-use approval for each eligible write step;
//! - let the browser step policy consume that approval only after Policy itself
//!   returns `AllowWithConfirmation`.
//!
//! Boundaries:
//! - Plan approval never creates step approval;
//! - approvals never override `Decision::Deny` or authorize read-only steps;
//! - no UI, persistence, task-wide scope, or public API is introduced.
//!
//! Invariants:
//! 1. unknown, read-only, already-allowed, duplicate, and over-plan approvals fail;
//! 2. each grant is `Once`, has a positive TTL, and covers exactly one use;
//! 3. policy evaluation remains the allow/deny authority.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use assistant_agent_core::{ApprovalGrants, GrantRequest, RuntimeExecutionError, StepPolicy};
use assistant_core::InstructionAttribution;
use assistant_hitl::{ApprovalScope, DiffPreview, FieldDiff};
use assistant_policy::{Decision, EgressDestination, EvaluationContext, Reversibility, RuleSet};
use assistant_protocol::{ToolEffect, ToolEnvelope, ToolReversibility, ToolSchema};
use assistant_task_engine::{PlanStep, StepEffect, StepId, TaskId};
use assistant_tool_bus::{Clock, SystemClock};
use serde::Deserialize;
use thiserror::Error;

use super::browser_task_support::{BrowserTaskError, argument_error};

const MAX_FORM_FIELDS: usize = 100;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_VALUE_BYTES: usize = 16 * 1024;
const MAX_TOTAL_VALUE_BYTES: usize = 64 * 1024;
const MAX_URL_BYTES: usize = 8 * 1024;

/// One validated `form_read` snapshot retained only for the next form fill.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct FormReadSnapshot {
    form_origin: String,
    form_action: String,
    fields: Vec<FormReadField>,
}

#[derive(Clone, PartialEq, Eq)]
struct FormReadField {
    selector: String,
    value: String,
}

/// Redacted field diff and validated attribution prepared for a future approval card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FormApprovalPreview {
    form_origin: String,
    form_action: String,
    diff: DiffPreview,
    attribution: InstructionAttribution,
}

/// Fail-closed preview construction errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum FormApprovalPreviewError {
    /// The model-visible `form_read` result is malformed.
    #[error("form_read snapshot is invalid: {reason}")]
    InvalidReadSnapshot { reason: String },
    /// The successful `form_fill` result omitted approval evidence.
    #[error("form_fill approval readback evidence is missing")]
    MissingEvidence,
    /// The approval evidence is malformed or exceeds a fixed bound.
    #[error("form_fill approval readback evidence is invalid: {reason}")]
    InvalidEvidence { reason: String },
    /// The evidence belongs to a different form state than the preceding read.
    #[error("form_fill approval readback evidence does not match form_read: {reason}")]
    EvidenceMismatch { reason: String },
    /// The readback contained no changed fields.
    #[error("form_fill produced no changed fields for an approval preview")]
    EmptyDiff,
}

impl FormReadSnapshot {
    /// Parses a successful `form_read` envelope into bounded runtime state.
    pub(super) fn from_envelope(envelope: &ToolEnvelope) -> Result<Self, FormApprovalPreviewError> {
        if !envelope.ok {
            return Err(invalid_read_snapshot(
                "form_read envelope is not successful",
            ));
        }
        let data = envelope
            .data
            .as_ref()
            .ok_or_else(|| invalid_read_snapshot("form_read envelope has no data payload"))?;
        let payload: FormReadPayload = serde_json::from_value(data.0.clone()).map_err(|error| {
            invalid_read_snapshot(format!("payload is not valid JSON: {error}"))
        })?;
        if !is_http_url(&payload.form_origin)
            || http_origin_prefix(&payload.form_origin) != Some(payload.form_origin.as_str())
        {
            return Err(invalid_read_snapshot(
                "form_origin is not an absolute HTTP(S) origin",
            ));
        }
        if !is_http_url(&payload.form_action) {
            return Err(invalid_read_snapshot(
                "form_action is not an absolute HTTP(S) URL",
            ));
        }
        validate_read_fields(&payload.fields)?;
        Ok(Self {
            form_origin: payload.form_origin,
            form_action: payload.form_action,
            fields: payload
                .fields
                .into_iter()
                .map(|field| FormReadField {
                    selector: field.selector,
                    value: field.value,
                })
                .collect(),
        })
    }
}

impl FormApprovalPreview {
    /// Builds a redacted preview from one matching before/readback pair.
    pub(super) fn from_envelopes(
        before: &FormReadSnapshot,
        fill: &ToolEnvelope,
        attribution: InstructionAttribution,
    ) -> Result<Self, FormApprovalPreviewError> {
        if !fill.ok {
            return Err(FormApprovalPreviewError::MissingEvidence);
        }
        let evidence = fill
            .evidence
            .as_ref()
            .ok_or(FormApprovalPreviewError::MissingEvidence)?;
        let extra = evidence
            .extra
            .as_ref()
            .ok_or(FormApprovalPreviewError::MissingEvidence)?;
        let evidence: ApprovalEvidenceEnvelope =
            serde_json::from_value(extra.clone()).map_err(|error| {
                FormApprovalPreviewError::InvalidEvidence {
                    reason: format!("evidence.extra is not the fixed v1 shape: {error}"),
                }
            })?;
        let readback = evidence.approval_readback;
        validate_evidence(&readback)?;
        if before.form_origin != readback.form_origin {
            return Err(evidence_mismatch("form_origin changed"));
        }
        if before.form_action != readback.form_action {
            return Err(evidence_mismatch("form_action changed"));
        }
        if before.fields.len() != readback.fields.len() {
            return Err(evidence_mismatch("field count changed"));
        }

        let mut diffs = Vec::new();
        for (before_field, after_field) in before.fields.iter().zip(&readback.fields) {
            if before_field.selector != after_field.selector {
                return Err(evidence_mismatch("field selector order changed"));
            }
            if before_field.value != after_field.before {
                return Err(evidence_mismatch("field before value changed"));
            }
            if before_field.value == after_field.after {
                continue;
            }
            diffs.push(FieldDiff {
                field: after_field.selector.clone(),
                before: redact_value(&after_field.before),
                after: redact_value(&after_field.after),
            });
        }
        if diffs.is_empty() {
            return Err(FormApprovalPreviewError::EmptyDiff);
        }
        Ok(Self {
            form_origin: readback.form_origin,
            form_action: readback.form_action,
            diff: DiffPreview::Fields(diffs),
            attribution,
        })
    }
}

#[derive(Deserialize)]
struct FormReadPayload {
    #[serde(rename = "origin")]
    form_origin: String,
    #[serde(rename = "action")]
    form_action: String,
    fields: Vec<FormReadFieldPayload>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormReadFieldPayload {
    selector: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalEvidenceEnvelope {
    #[serde(rename = "assistant.browser_form_fill.approval_readback.v1")]
    approval_readback: ApprovalReadback,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalReadback {
    form_origin: String,
    form_action: String,
    fields: Vec<ApprovalReadbackField>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalReadbackField {
    selector: String,
    before: String,
    after: String,
}

fn validate_read_fields(fields: &[FormReadFieldPayload]) -> Result<(), FormApprovalPreviewError> {
    if fields.is_empty() || fields.len() > MAX_FORM_FIELDS {
        return Err(invalid_read_snapshot("field count is outside 1..=100"));
    }
    let mut selectors = BTreeSet::new();
    let mut total = 0usize;
    for field in fields {
        validate_selector(&field.selector)
            .map_err(|reason| invalid_read_snapshot(format!("field selector {reason}")))?;
        validate_value(&field.value)
            .map_err(|reason| invalid_read_snapshot(format!("field value {reason}")))?;
        if !selectors.insert(field.selector.as_str()) {
            return Err(invalid_read_snapshot("field selector is duplicated"));
        }
        total = total.saturating_add(field.value.len());
        if total > MAX_TOTAL_VALUE_BYTES {
            return Err(invalid_read_snapshot("field values exceed 64 KiB"));
        }
    }
    Ok(())
}

fn validate_evidence(readback: &ApprovalReadback) -> Result<(), FormApprovalPreviewError> {
    if !is_http_url(&readback.form_origin)
        || http_origin_prefix(&readback.form_origin) != Some(readback.form_origin.as_str())
    {
        return Err(invalid_evidence(
            "form_origin is not an absolute HTTP(S) origin",
        ));
    }
    if !is_http_url(&readback.form_action) {
        return Err(invalid_evidence(
            "form_action is not an absolute HTTP(S) URL",
        ));
    }
    if readback.fields.is_empty() || readback.fields.len() > MAX_FORM_FIELDS {
        return Err(invalid_evidence("field count is outside 1..=100"));
    }
    let mut selectors = BTreeSet::new();
    let mut total = 0usize;
    for field in &readback.fields {
        validate_selector(&field.selector)
            .map_err(|reason| invalid_evidence(format!("field selector {reason}")))?;
        validate_value(&field.before)
            .map_err(|reason| invalid_evidence(format!("before value {reason}")))?;
        validate_value(&field.after)
            .map_err(|reason| invalid_evidence(format!("after value {reason}")))?;
        if !selectors.insert(field.selector.as_str()) {
            return Err(invalid_evidence("field selector is duplicated"));
        }
        total = total
            .saturating_add(field.before.len())
            .saturating_add(field.after.len());
        if total > MAX_TOTAL_VALUE_BYTES {
            return Err(invalid_evidence("field values exceed 64 KiB"));
        }
    }
    Ok(())
}

fn validate_selector(value: &str) -> Result<(), &'static str> {
    if value.trim().is_empty() {
        return Err("must not be empty");
    }
    if value.len() > MAX_SELECTOR_BYTES {
        return Err("exceeds 4 KiB");
    }
    if has_forbidden_control(value) {
        return Err("contains an unsupported control character");
    }
    Ok(())
}

fn validate_value(value: &str) -> Result<(), &'static str> {
    if value.len() > MAX_VALUE_BYTES {
        return Err("exceeds 16 KiB");
    }
    if has_forbidden_control(value) {
        return Err("contains an unsupported control character");
    }
    Ok(())
}

fn has_forbidden_control(value: &str) -> bool {
    value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    })
}

fn redact_value(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("[redacted:{}]", value.len())
    }
}

fn is_http_url(url: &str) -> bool {
    !url.is_empty()
        && url.len() <= MAX_URL_BYTES
        && !url.contains('#')
        && !url.contains('\\')
        && !url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && http_origin_prefix(url).is_some()
}

fn http_origin_prefix(url: &str) -> Option<&str> {
    let scheme_end = url.find("://")?;
    let scheme = url.get(..scheme_end)?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority_start = scheme_end.checked_add(3)?;
    let remainder = url.get(authority_start..)?;
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = remainder.get(..authority_end)?;
    if authority.is_empty() || authority.contains('@') || authority.contains('\\') {
        return None;
    }
    url.get(..authority_start.checked_add(authority_end)?)
}

fn invalid_read_snapshot(reason: impl Into<String>) -> FormApprovalPreviewError {
    FormApprovalPreviewError::InvalidReadSnapshot {
        reason: reason.into(),
    }
}

fn invalid_evidence(reason: impl Into<String>) -> FormApprovalPreviewError {
    FormApprovalPreviewError::InvalidEvidence {
        reason: reason.into(),
    }
}

fn evidence_mismatch(reason: impl Into<String>) -> FormApprovalPreviewError {
    FormApprovalPreviewError::EvidenceMismatch {
        reason: reason.into(),
    }
}

pub(super) struct BrowserTaskPolicy {
    pub(super) rules: RuleSet,
    pub(super) catalog: BTreeMap<String, ToolSchema>,
    pub(super) target_app: String,
    pub(super) grants: Arc<ApprovalGrants>,
    pub(super) task_id: TaskId,
}

impl BrowserTaskPolicy {
    pub(super) const fn new(
        rules: RuleSet,
        catalog: BTreeMap<String, ToolSchema>,
        target_app: String,
        grants: Arc<ApprovalGrants>,
        task_id: TaskId,
    ) -> Self {
        Self {
            rules,
            catalog,
            target_app,
            grants,
            task_id,
        }
    }

    fn evaluate_without_approval(
        &self,
        step: &PlanStep,
    ) -> Result<Decision, RuntimeExecutionError> {
        let schema = self
            .catalog
            .get(&step.tool)
            .ok_or_else(|| RuntimeExecutionError::Policy {
                reason: format!("browser tool `{}` is absent from the catalog", step.tool),
            })?;
        self.rules
            .evaluate(&EvaluationContext {
                effect: policy_effect(schema.effect),
                risk_level: schema.risk_level,
                reversibility: policy_reversibility(schema.reversibility),
                unattended: false,
                tainted: false,
                target_app: self.target_app.clone(),
                egress: EgressDestination::None,
            })
            .map_err(|error| RuntimeExecutionError::Policy {
                reason: error.to_string(),
            })
    }
}

impl StepPolicy for BrowserTaskPolicy {
    fn decide(&self, step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        let decision = self.evaluate_without_approval(step)?;
        if step.effect != StepEffect::Write
            || !matches!(decision, Decision::AllowWithConfirmation { .. })
        {
            return Ok(decision);
        }
        let approval = self
            .grants
            .consume(
                self.task_id.as_str(),
                step.id.as_str(),
                SystemClock.now_unix_ms(),
            )
            .map_err(|error| RuntimeExecutionError::Policy {
                reason: format!("explicit step approval is unavailable: {error}"),
            })?;
        if approval.is_some() {
            Ok(Decision::Allow {
                rule_id: "explicit_step_approval".to_owned(),
            })
        } else {
            Ok(decision)
        }
    }
}

pub(super) fn validate_step_approvals(
    ordered: &[PlanStep],
    approved_step_ids: &[String],
    policy: &BrowserTaskPolicy,
    catalog: &BTreeMap<String, ToolSchema>,
) -> Result<Vec<StepId>, BrowserTaskError> {
    let plan_step_limit = usize::try_from(super::PLAN_BUDGET_STEPS).unwrap_or(usize::MAX);
    let approval_limit = ordered.len().min(plan_step_limit);
    if approved_step_ids.len() > approval_limit {
        return Err(argument_error(format!(
            "explicit step approvals exceed the plan step limit: approved={} limit={approval_limit}",
            approved_step_ids.len()
        )));
    }
    let mut approved = Vec::with_capacity(approved_step_ids.len());
    let mut seen = BTreeSet::new();
    for step_id in approved_step_ids {
        let parsed = StepId::new(step_id.clone()).map_err(|error| {
            argument_error(format!("--approve-step `{step_id}` is invalid: {error}"))
        })?;
        if !seen.insert(parsed.clone()) {
            return Err(argument_error(format!(
                "--approve-step `{step_id}` was provided more than once"
            )));
        }
        let step = ordered
            .iter()
            .find(|candidate| candidate.id == parsed)
            .ok_or_else(|| {
                argument_error(format!(
                    "--approve-step `{step_id}` is not present in the generated plan"
                ))
            })?;
        validate_approval_target(step_id, step, policy, catalog)?;
        approved.push(parsed);
    }
    Ok(approved)
}

fn validate_approval_target(
    step_id: &str,
    step: &PlanStep,
    policy: &BrowserTaskPolicy,
    catalog: &BTreeMap<String, ToolSchema>,
) -> Result<(), BrowserTaskError> {
    if step.effect != StepEffect::Write {
        return Err(argument_error(format!(
            "--approve-step `{step_id}` targets a read-only step"
        )));
    }
    let schema = catalog.get(&step.tool).ok_or_else(|| {
        argument_error(format!(
            "--approve-step `{step_id}` targets an unknown tool `{}`",
            step.tool
        ))
    })?;
    if schema.effect != ToolEffect::Write {
        return Err(argument_error(format!(
            "--approve-step `{step_id}` targets a tool that is authoritative read-only"
        )));
    }
    match policy.evaluate_without_approval(step)? {
        Decision::AllowWithConfirmation { .. } => Ok(()),
        Decision::Allow { rule_id } => Err(argument_error(format!(
            "--approve-step `{step_id}` targets a step that policy allows without confirmation ({rule_id})"
        ))),
        Decision::Deny { rule_id, reason } => Err(argument_error(format!(
            "--approve-step `{step_id}` targets a step denied by policy {rule_id}: {reason}"
        ))),
    }
}

pub(super) fn grant_step_approvals(
    grants: &ApprovalGrants,
    task_id: &TaskId,
    approved_steps: &[StepId],
    now_ms: i64,
) -> Result<(), BrowserTaskError> {
    let ttl_ms = i64::try_from(super::PLAN_BUDGET_MS).map_err(|error| {
        argument_error(format!(
            "browser plan budget cannot be an approval TTL: {error}"
        ))
    })?;
    for step_id in approved_steps {
        grants
            .grant(&GrantRequest {
                task_id: task_id.as_str(),
                step_id: step_id.as_str(),
                scope: ApprovalScope::Once,
                now_ms,
                ttl_ms,
                uses: 1,
            })
            .map_err(|error| {
                argument_error(format!(
                    "cannot grant explicit step approval for `{step_id}`: {error}"
                ))
            })?;
    }
    Ok(())
}

const fn policy_effect(effect: ToolEffect) -> assistant_policy::Effect {
    match effect {
        ToolEffect::Read => assistant_policy::Effect::Read,
        _ => assistant_policy::Effect::Write,
    }
}

const fn policy_reversibility(reversibility: ToolReversibility) -> Reversibility {
    match reversibility {
        ToolReversibility::L0UndoStack => Reversibility::L0UndoStack,
        ToolReversibility::L1Snapshot => Reversibility::L1Snapshot,
        ToolReversibility::L2Compensation => Reversibility::L2Compensating,
        _ => Reversibility::L3Irreversible,
    }
}

#[cfg(test)]
#[path = "browser_task_approval_tests.rs"]
mod tests;
