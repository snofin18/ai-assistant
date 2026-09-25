//! Shared task-engine test fixtures.

#![allow(dead_code, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_protocol::serde_json::{Value, json};
use assistant_task_engine::{
    Budget, CheckpointPolicy, Plan, PlanStep, Reversibility, StepEffect, StepId, StepTimeouts,
    TaskId,
};

/// Builds a plan with one read step.
pub fn single_read_plan(task_id: &str, step_id: &str) -> Plan {
    let task_id = TaskId::new(task_id).expect("task id");
    Plan {
        plan_id: assistant_task_engine::PlanId::new(format!("p_{task_id}")).expect("plan id"),
        task_id,
        goal: "read a value".to_owned(),
        steps: vec![read_step(step_id, 1, Vec::new())],
        budget: Budget::new(10, 60_000, 10_000, 1.0).expect("budget"),
        checkpoint_policy: CheckpointPolicy::AfterEachTransition,
    }
}

/// Builds a valid read step.
pub fn read_step(step_id: &str, sequence: u32, depends_on: Vec<&str>) -> PlanStep {
    PlanStep {
        id: StepId::new(step_id).expect("step id"),
        sequence,
        tool: "notepad.text.read".to_owned(),
        args: json!({"path": "sample.txt"}),
        depends_on: depends_on
            .into_iter()
            .map(|id| StepId::new(id).expect("dependency id"))
            .collect(),
        postconditions: Vec::new(),
        effect: StepEffect::Read,
        reversibility: Reversibility::L0UndoStack,
        point_of_no_return: false,
        timeouts: StepTimeouts::new(1_000, 2_000, 1_000).expect("timeouts"),
    }
}

/// Builds a valid write step.
pub fn write_step(step_id: &str, sequence: u32, depends_on: Vec<&str>) -> PlanStep {
    PlanStep {
        id: StepId::new(step_id).expect("step id"),
        sequence,
        tool: "notepad.text.write".to_owned(),
        args: json!({"text": "hello"}),
        depends_on: depends_on
            .into_iter()
            .map(|id| StepId::new(id).expect("dependency id"))
            .collect(),
        postconditions: vec![postcondition()],
        effect: StepEffect::Write,
        reversibility: Reversibility::L0UndoStack,
        point_of_no_return: false,
        timeouts: StepTimeouts::new(1_000, 2_000, 1_000).expect("timeouts"),
    }
}

/// Returns a representative postcondition.
pub fn postcondition() -> Value {
    json!({"kind": "element_exists"})
}
