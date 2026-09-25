//! Identifier, ordering, and watchdog edge cases.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use assistant_task_engine::{
    MemoryCheckpointStore, Reversibility, StepId, TaskEngine, TaskId, check_watchdog,
};
use common::{read_step, single_read_plan, write_step};

#[test]
fn test_identifier_rejects_invalid_values() {
    assert!(TaskId::new("").is_err());
    assert!(TaskId::new("bad value").is_err());
    assert!(StepId::new("săn").is_err());
    assert_eq!(TaskId::new("t_ok").expect("valid").as_str(), "t_ok");
}

#[test]
fn test_plan_ordering_and_worst_reversibility() {
    let mut plan = single_read_plan("t_order", "s_1");
    plan.steps = vec![
        write_step("s_2", 2, Vec::new()),
        read_step("s_1", 1, Vec::new()),
    ];
    if let Some(step) = plan.steps.first_mut() {
        step.reversibility = Reversibility::L3Irreversible;
        step.point_of_no_return = true;
    }
    let ordered: Vec<String> = plan
        .ordered_steps()
        .iter()
        .map(|step| step.id.to_string())
        .collect();
    assert_eq!(ordered, ["s_1", "s_2"]);
    assert_eq!(plan.worst_reversibility(), Reversibility::L3Irreversible);
}

#[test]
fn test_watchdog_clock_and_missing_phase_errors() {
    let task_id = TaskId::new("t_watchdog_edges").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine
        .create_task(single_read_plan(task_id.as_str(), step_id.as_str()), 1_000)
        .expect("create");
    engine
        .apply_task_event(
            &task_id,
            assistant_task_engine::TaskEvent::SubmitForApproval,
            1_010,
        )
        .expect("submit");
    engine
        .apply_task_event(
            &task_id,
            assistant_task_engine::TaskEvent::ApprovePlan,
            1_020,
        )
        .expect("approve");
    engine.begin_step(&task_id, &step_id, 1_030).expect("begin");
    assert!(engine.check_watchdog(&task_id, 1_000).is_err());

    let mut snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    if let Some(step) = snapshot.steps.first_mut() {
        step.phase_started_at_ms = None;
    }
    assert!(check_watchdog(&snapshot.plan, &snapshot, 2_000).is_err());
}
