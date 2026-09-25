//! Task and step state-machine coverage.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use assistant_task_engine::{
    MemoryCheckpointStore, StepEvent, StepStatus, TaskEngine, TaskEvent, TaskId, TaskStatus,
    transition_step, transition_task,
};
use common::single_read_plan;

#[test]
fn test_catalog_contains_all_documented_states() {
    assert_eq!(TaskStatus::ALL.len(), 12);
    assert_eq!(StepStatus::ALL.len(), 11);
    let unique_task_names: std::collections::BTreeSet<&str> = TaskStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    let unique_step_names: std::collections::BTreeSet<&str> = StepStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    assert_eq!(unique_task_names.len(), 12);
    assert_eq!(unique_step_names.len(), 11);
}

#[test]
fn test_task_lifecycle_covers_non_terminal_states() {
    let running = transition_task(TaskStatus::Draft, TaskEvent::SubmitForApproval).expect("submit");
    assert_eq!(running, TaskStatus::AwaitingApproval);
    let running = transition_task(running, TaskEvent::ApprovePlan).expect("approve");
    assert_eq!(running, TaskStatus::Running);
    let paused = transition_task(running, TaskEvent::Pause).expect("pause");
    assert_eq!(paused, TaskStatus::Paused);
    let resumed = transition_task(paused, TaskEvent::Resume).expect("resume");
    assert_eq!(resumed, TaskStatus::Running);
    let taken_over = transition_task(resumed, TaskEvent::TakeOver).expect("take over");
    assert_eq!(taken_over, TaskStatus::TakenOver);
    let resuming = transition_task(taken_over, TaskEvent::ReturnControl).expect("return");
    assert_eq!(resuming, TaskStatus::Resuming);
    assert_eq!(
        transition_task(resuming, TaskEvent::FinishRecovery).expect("finish recovery"),
        TaskStatus::Running
    );
    assert_eq!(
        transition_task(TaskStatus::Running, TaskEvent::Block).expect("block"),
        TaskStatus::Blocked
    );
    assert_eq!(
        transition_task(TaskStatus::Blocked, TaskEvent::Unblock).expect("unblock"),
        TaskStatus::Resuming
    );
    assert_eq!(
        transition_task(TaskStatus::NeedsHuman, TaskEvent::Resume).expect("human resume"),
        TaskStatus::Resuming
    );
}

#[test]
fn test_terminal_task_states_reject_transitions() {
    for terminal in [
        TaskStatus::Cancelled,
        TaskStatus::Completed,
        TaskStatus::CompletedWithWarnings,
        TaskStatus::Failed,
    ] {
        assert!(
            transition_task(terminal, TaskEvent::Pause).is_err(),
            "{terminal:?} must be terminal"
        );
    }
}

#[test]
fn test_step_lifecycle_and_retry_paths() {
    let status = transition_step(StepStatus::Pending, StepEvent::BeginPrecheck).expect("precheck");
    assert_eq!(status, StepStatus::Prechecking);
    let status = transition_step(status, StepEvent::Approve).expect("approve");
    assert_eq!(status, StepStatus::Approved);
    let status = transition_step(status, StepEvent::BeginExecute).expect("execute");
    assert_eq!(status, StepStatus::Executing);
    let status = transition_step(status, StepEvent::BeginVerify).expect("verify");
    assert_eq!(status, StepStatus::Verifying);
    assert_eq!(
        transition_step(status, StepEvent::Commit).expect("commit"),
        StepStatus::Committed
    );

    let failed = transition_step(StepStatus::Executing, StepEvent::Fail).expect("fail");
    assert_eq!(failed, StepStatus::Failed);
    let retrying = transition_step(failed, StepEvent::ScheduleRetry).expect("retry");
    assert_eq!(retrying, StepStatus::Retrying);
    assert_eq!(
        transition_step(retrying, StepEvent::BeginRetry).expect("begin retry"),
        StepStatus::Pending
    );
    assert_eq!(
        transition_step(StepStatus::Verifying, StepEvent::Rollback).expect("rollback"),
        StepStatus::RolledBack
    );
    assert_eq!(
        transition_step(StepStatus::Prechecking, StepEvent::DenyPolicy).expect("policy"),
        StepStatus::PolicyDenied
    );
    assert_eq!(
        transition_step(StepStatus::Pending, StepEvent::Skip).expect("skip"),
        StepStatus::Skipped
    );
}

#[test]
fn test_engine_persists_each_successful_transition() {
    let task_id = TaskId::new("t_persist").expect("task id");
    let plan = single_read_plan(task_id.as_str(), "s_1");
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    let draft = engine.create_task(plan, 1_000).expect("create");
    assert_eq!(draft.revision, 0);
    let submitted = engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit");
    assert_eq!(submitted.revision, 1);
    let running = engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve");
    assert_eq!(running.revision, 2);
    assert_eq!(engine.load_snapshot(&task_id).expect("reload"), running);
}
