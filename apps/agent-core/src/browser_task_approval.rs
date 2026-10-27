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
use assistant_hitl::ApprovalScope;
use assistant_policy::{Decision, EgressDestination, EvaluationContext, Reversibility, RuleSet};
use assistant_protocol::{ToolEffect, ToolReversibility, ToolSchema};
use assistant_task_engine::{PlanStep, StepEffect, StepId, TaskId};
use assistant_tool_bus::{Clock, SystemClock};

use super::browser_task_support::{BrowserTaskError, argument_error};

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
