//! Plan validation and ready-frontier coverage.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use assistant_task_engine::{
    Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, Reversibility, StepEffect, TaskEngine,
    TaskEvent, TaskId, ready_step_ids,
};
use common::{postcondition, read_step, write_step};

fn base_plan(steps: Vec<assistant_task_engine::PlanStep>) -> Plan {
    let task_id = TaskId::new("t_dag").expect("task id");
    Plan {
        plan_id: assistant_task_engine::PlanId::new("p_dag").expect("plan id"),
        task_id,
        goal: "validate the DAG".to_owned(),
        steps,
        budget: Budget::new(10, 60_000, 1_000, 1.0).expect("budget"),
        checkpoint_policy: CheckpointPolicy::AfterEachTransition,
    }
}

#[test]
fn test_ready_frontier_is_dependency_ordered() {
    let plan = base_plan(vec![
        write_step("s_2", 2, vec!["s_1"]),
        read_step("s_1", 1, Vec::new()),
    ]);
    let task_id = plan.task_id.clone();
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    let snapshot = engine.create_task(plan, 1_000).expect("create");
    assert_eq!(
        ready_step_ids(&snapshot)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["s_1"]
    );
    let approved = engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit");
    let running = engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve");
    assert_eq!(
        approved.status,
        assistant_task_engine::TaskStatus::AwaitingApproval
    );
    assert_eq!(running.status, assistant_task_engine::TaskStatus::Running);
}

#[test]
fn test_duplicate_step_ids_are_rejected() {
    let plan = base_plan(vec![
        read_step("s_1", 1, Vec::new()),
        read_step("s_1", 2, Vec::new()),
    ]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_duplicate_sequences_are_rejected() {
    let plan = base_plan(vec![
        read_step("s_1", 1, Vec::new()),
        read_step("s_2", 1, Vec::new()),
    ]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_missing_dependency_is_rejected() {
    let plan = base_plan(vec![read_step("s_1", 1, vec!["s_missing"])]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_dependency_cycle_is_rejected() {
    let plan = base_plan(vec![
        read_step("s_1", 1, vec!["s_2"]),
        read_step("s_2", 2, vec!["s_1"]),
    ]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_write_without_postcondition_is_rejected() {
    let mut step = write_step("s_1", 1, Vec::new());
    step.postconditions.clear();
    let plan = base_plan(vec![step]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_malformed_tool_name_is_rejected() {
    let mut step = read_step("s_1", 1, Vec::new());
    step.tool = "notepad".to_owned();
    let plan = base_plan(vec![step]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_l3_requires_point_of_no_return() {
    let mut step = write_step("s_1", 1, Vec::new());
    step.reversibility = Reversibility::L3Irreversible;
    step.point_of_no_return = false;
    let plan = base_plan(vec![step]);
    assert!(plan.validate().is_err());
}

#[test]
fn test_valid_write_postcondition_alignment() {
    let mut step = write_step("s_1", 1, Vec::new());
    step.effect = StepEffect::Write;
    step.postconditions = vec![postcondition()];
    let plan = base_plan(vec![step]);
    assert!(plan.validate().is_ok());
}
