//! Crash-recovery evidence coverage.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;

use assistant_task_engine::{
    MemoryCheckpointStore, RecoveryAction, RecoveryEvidence, StepEvent, StepStatus, TaskEngine,
    TaskEvent, TaskHoldReason, TaskId, TaskStatus, assess_recovery, transition_step,
};
use common::single_read_plan;

fn executing_snapshot() -> (TaskEngine<MemoryCheckpointStore>, TaskId) {
    let task_id = TaskId::new("t_recover").expect("task id");
    let plan = single_read_plan(task_id.as_str(), "s_1");
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine.create_task(plan, 1_000).expect("create");
    engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit");
    engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve");
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step");
    engine.begin_step(&task_id, &step_id, 1_030).expect("begin");
    engine
        .approve_step(&task_id, &step_id, 1_040)
        .expect("approve step");
    engine
        .begin_execute(&task_id, &step_id, 1_050)
        .expect("execute");
    (engine, task_id)
}

#[test]
fn test_completed_evidence_commits_active_step() {
    let (engine, task_id) = executing_snapshot();
    let snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step");
    let evidence = BTreeMap::from([(step_id.clone(), RecoveryEvidence::Completed)]);
    let assessment = assess_recovery(snapshot, &evidence, 2_000).expect("assess");
    assert_eq!(
        assessment.steps.first().map(|step| step.action.clone()),
        Some(RecoveryAction::Commit)
    );
    assert_eq!(
        assessment.snapshot.step(&step_id).map(|step| step.status),
        Some(StepStatus::Committed)
    );
    assert_eq!(assessment.snapshot.status, TaskStatus::Completed);
}

#[test]
fn test_not_completed_evidence_reopens_step() {
    let (engine, task_id) = executing_snapshot();
    let snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step");
    let evidence = BTreeMap::from([(step_id.clone(), RecoveryEvidence::NotCompleted)]);
    let assessment = assess_recovery(snapshot, &evidence, 2_000).expect("assess");
    assert_eq!(
        assessment.steps.first().map(|step| step.action.clone()),
        Some(RecoveryAction::Redo)
    );
    assert_eq!(
        assessment.snapshot.step(&step_id).map(|step| step.status),
        Some(StepStatus::Pending)
    );
    assert_eq!(
        assessment
            .snapshot
            .step(&step_id)
            .and_then(|step| step.ended_at_ms),
        None
    );
    assert_eq!(assessment.snapshot.status, TaskStatus::Resuming);
}

#[test]
fn test_unknown_and_missing_evidence_require_human() {
    let (engine, task_id) = executing_snapshot();
    let snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step");
    let unknown = BTreeMap::from([(step_id, RecoveryEvidence::Unknown)]);
    let assessment = assess_recovery(snapshot.clone(), &unknown, 2_000).expect("assess");
    assert_eq!(assessment.snapshot.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        assessment.snapshot.hold_reason,
        Some(TaskHoldReason::UnknownStepOutcome { .. })
    ));

    let assessment = assess_recovery(snapshot, &BTreeMap::new(), 2_000).expect("assess");
    assert_eq!(assessment.snapshot.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        assessment.snapshot.hold_reason,
        Some(TaskHoldReason::MissingRecoveryEvidence { .. })
    ));
}

#[test]
fn test_safe_prechecking_reset_does_not_guess_execution() {
    assert_eq!(
        transition_step(StepStatus::Prechecking, StepEvent::RecoverSafeReset).expect("safe reset"),
        StepStatus::Pending
    );
}
