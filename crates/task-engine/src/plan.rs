//! Plan and Step DAG types plus structural validation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use assistant_protocol::serde_json::Value;
use serde::{Deserialize, Serialize};

use crate::budget::Budget;
use crate::error::{TaskEngineError, TaskEngineResult};
use crate::identifiers::{PlanId, StepId, TaskId};

/// Whether a step reads or writes state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StepEffect {
    /// The step only reads.
    Read,
    /// The step changes external state.
    Write,
}

/// The four-level reversibility model from architecture v2 section 9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reversibility {
    /// Application undo stack.
    L0UndoStack,
    /// Snapshot can restore the state.
    L1Snapshot,
    /// A semantic compensating action exists.
    L2Compensation,
    /// The action cannot be reversed.
    L3Irreversible,
}

impl Reversibility {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L0UndoStack => "l0_undo_stack",
            Self::L1Snapshot => "l1_snapshot",
            Self::L2Compensation => "l2_compensation",
            Self::L3Irreversible => "l3_irreversible",
        }
    }
}

/// Phase-specific timeouts in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StepTimeouts {
    /// Target resolution and precheck timeout.
    pub resolve_ms: u64,
    /// Tool execution timeout.
    pub execute_ms: u64,
    /// Postcondition verification timeout.
    pub verify_ms: u64,
}

impl StepTimeouts {
    /// Creates validated phase timeouts.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::InvalidPlan`] when any timeout is zero.
    pub fn new(resolve_ms: u64, execute_ms: u64, verify_ms: u64) -> TaskEngineResult<Self> {
        if resolve_ms == 0 || execute_ms == 0 || verify_ms == 0 {
            return Err(TaskEngineError::InvalidPlan {
                reason: "step timeouts must all be greater than zero".to_owned(),
            });
        }
        Ok(Self {
            resolve_ms,
            execute_ms,
            verify_ms,
        })
    }
}

/// Checkpoint persistence policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CheckpointPolicy {
    /// Persist before returning every successful state transition.
    AfterEachTransition,
}

/// One step in a validated Plan DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step identifier unique inside the plan.
    pub id: StepId,
    /// Stable ordering key, unique inside the plan.
    pub sequence: u32,
    /// Registered tool name in `<app>.<domain>.<action>` form.
    pub tool: String,
    /// Tool arguments captured at planning time.
    pub args: Value,
    /// Steps that must commit or skip before this step is ready.
    pub depends_on: Vec<StepId>,
    /// Required postconditions for write steps.
    pub postconditions: Vec<Value>,
    /// Read or write effect.
    pub effect: StepEffect,
    /// Reversibility classification.
    pub reversibility: Reversibility,
    /// Whether crossing this step prevents rollback.
    pub point_of_no_return: bool,
    /// Phase-specific watchdog timeouts.
    pub timeouts: StepTimeouts,
}

/// A complete plan for one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Stable plan identifier.
    pub plan_id: PlanId,
    /// Task this plan belongs to.
    pub task_id: TaskId,
    /// User-visible goal.
    pub goal: String,
    /// Steps in the DAG.
    pub steps: Vec<PlanStep>,
    /// Task resource budget.
    pub budget: Budget,
    /// Checkpoint persistence policy.
    pub checkpoint_policy: CheckpointPolicy,
}

impl Plan {
    /// Validates identifiers, postconditions, tool names, and the DAG.
    ///
    /// # Errors
    ///
    /// Returns a structured error for an empty goal or step list, duplicate
    /// ids or sequences, missing dependencies, cycles, malformed tool names,
    /// write steps without postconditions, or an L3 step not marked as a
    /// point of no return.
    pub fn validate(&self) -> TaskEngineResult<()> {
        if self.goal.trim().is_empty() {
            return Err(TaskEngineError::InvalidPlan {
                reason: "goal must not be empty".to_owned(),
            });
        }
        if self.steps.is_empty() {
            return Err(TaskEngineError::InvalidPlan {
                reason: "plan must contain at least one step".to_owned(),
            });
        }

        let mut ids = BTreeSet::new();
        let mut sequences = BTreeSet::new();
        for step in &self.steps {
            if !ids.insert(step.id.clone()) {
                return Err(TaskEngineError::DuplicateStepId {
                    step_id: step.id.to_string(),
                });
            }
            if !sequences.insert(step.sequence) {
                return Err(TaskEngineError::DuplicateStepSequence {
                    sequence: step.sequence,
                });
            }
            validate_tool_name(step)?;
            if step.effect == StepEffect::Write && step.postconditions.is_empty() {
                return Err(TaskEngineError::MissingPostconditions {
                    step_id: step.id.to_string(),
                });
            }
            if step.reversibility == Reversibility::L3Irreversible && !step.point_of_no_return {
                return Err(TaskEngineError::InvalidPlan {
                    reason: format!(
                        "L3Irreversible step {} must set point_of_no_return",
                        step.id
                    ),
                });
            }
        }

        for step in &self.steps {
            for dependency in &step.depends_on {
                if !ids.contains(dependency) {
                    return Err(TaskEngineError::MissingDependency {
                        step_id: step.id.to_string(),
                        dependency_id: dependency.to_string(),
                    });
                }
            }
        }

        if has_cycle(&self.steps, &ids) {
            return Err(TaskEngineError::DependencyCycle);
        }
        Ok(())
    }

    /// Returns steps ordered by sequence and then id.
    #[must_use]
    pub fn ordered_steps(&self) -> Vec<&PlanStep> {
        let mut steps: Vec<&PlanStep> = self.steps.iter().collect();
        steps.sort_by(|left, right| {
            left.sequence
                .cmp(&right.sequence)
                .then_with(|| left.id.cmp(&right.id))
        });
        steps
    }

    /// Returns the worst reversibility in the plan.
    #[must_use]
    pub fn worst_reversibility(&self) -> Reversibility {
        self.steps
            .iter()
            .map(|step| step.reversibility)
            .max()
            .unwrap_or(Reversibility::L0UndoStack)
    }
}

fn validate_tool_name(step: &PlanStep) -> TaskEngineResult<()> {
    let segments: Vec<&str> = step.tool.split('.').collect();
    let is_valid = segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        });
    if is_valid {
        return Ok(());
    }
    Err(TaskEngineError::InvalidToolName {
        step_id: step.id.to_string(),
        tool: step.tool.clone(),
    })
}

fn has_cycle(steps: &[PlanStep], ids: &BTreeSet<StepId>) -> bool {
    let mut indegree: BTreeMap<StepId, usize> = ids.iter().cloned().map(|id| (id, 0)).collect();
    let mut dependents: BTreeMap<StepId, Vec<StepId>> = BTreeMap::new();

    for step in steps {
        for dependency in &step.depends_on {
            if let Some(value) = indegree.get_mut(&step.id) {
                *value += 1;
            }
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(step.id.clone());
        }
    }

    let mut ready: VecDeque<StepId> = indegree
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect();
    let mut visited = 0_usize;

    while let Some(id) = ready.pop_front() {
        visited += 1;
        if let Some(children) = dependents.get(&id) {
            for child in children {
                if let Some(count) = indegree.get_mut(child) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push_back(child.clone());
                    }
                }
            }
        }
    }

    visited != ids.len()
}
