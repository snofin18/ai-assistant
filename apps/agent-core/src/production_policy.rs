//! Policy adapter used by the production `RuntimeExecutor`.
//!
//! Responsibilities:
//! - turn one `PlanStep` plus its authoritative tool metadata into the policy
//!   engine's `EvaluationContext` and ask the rule set for a decision.
//!
//! Boundaries:
//! - does not decide anything itself: `crates/policy` remains the single
//!   allow/deny/confirmation point (iron law 3);
//! - does not execute tools, resolve targets, or touch the task engine.
//!
//! Invariants:
//! 1. a tool missing from the catalog fails closed rather than defaulting;
//! 2. step effect and reversibility come from the **authoritative** step metadata
//!    (ADR-0055), never from the model;
//! 3. unattended and tainted are `false` because 1a runs attended only.
//!
//! Split out of `production.rs` so that file stays under the 600-line guideline
//! (gov section 5.4).

use std::collections::BTreeMap;

use assistant_policy::{Decision, Effect, EgressDestination, Reversibility, RuleSet};
use assistant_protocol::ToolSchema;
use assistant_task_engine::PlanStep;

use crate::runtime::{RuntimeExecutionError, StepPolicy};

/// Step policy backed by the adapter tool catalog and the workspace rule set.
pub struct CatalogStepPolicy {
    pub rules: RuleSet,
    pub catalog: BTreeMap<String, ToolSchema>,
    pub target_app: String,
}

impl StepPolicy for CatalogStepPolicy {
    fn decide(&self, step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        let metadata =
            self.catalog
                .get(&step.tool)
                .ok_or_else(|| RuntimeExecutionError::Policy {
                    reason: format!("tool `{}` is absent from the policy catalog", step.tool),
                })?;
        let context = assistant_policy::EvaluationContext {
            effect: policy_effect(step.effect),
            risk_level: metadata.risk_level,
            reversibility: policy_reversibility(step.reversibility),
            unattended: false,
            tainted: false,
            target_app: self.target_app.clone(),
            egress: EgressDestination::None,
        };
        self.rules
            .evaluate(&context)
            .map_err(|error| RuntimeExecutionError::Policy {
                reason: error.to_string(),
            })
    }
}

/// Maps the task engine's step effect onto the policy engine's effect.
const fn policy_effect(effect: assistant_task_engine::StepEffect) -> Effect {
    match effect {
        assistant_task_engine::StepEffect::Read => Effect::Read,
        _ => Effect::Write,
    }
}

/// Maps the task engine's reversibility onto the policy engine's reversibility.
const fn policy_reversibility(
    reversibility: assistant_task_engine::Reversibility,
) -> Reversibility {
    match reversibility {
        assistant_task_engine::Reversibility::L0UndoStack => Reversibility::L0UndoStack,
        assistant_task_engine::Reversibility::L1Snapshot => Reversibility::L1Snapshot,
        assistant_task_engine::Reversibility::L2Compensation => Reversibility::L2Compensating,
        _ => Reversibility::L3Irreversible,
    }
}
