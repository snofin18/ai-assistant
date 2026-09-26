//! Pause, resume, takeover, and return-control contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_hitl::{HitlCoordinator, HitlError};
use assistant_protocol::serde_json::json;
use assistant_task_engine::{
    Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep, Reversibility,
    StepEffect, StepId, StepTimeouts, TaskEngine, TaskEvent, TaskId, TaskStatus,
};

fn running_engine() -> (HitlCoordinator<MemoryCheckpointStore>, TaskId) {
    let task_id = TaskId::new("t_takeover").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let plan = Plan {
        plan_id: PlanId::new("p_takeover").expect("plan"),
        task_id: task_id.clone(),
        goal: "write a value".to_owned(),
        steps: vec![PlanStep {
            id: step_id,
            sequence: 1,
            tool: "notepad.text.write".to_owned(),
            args: json!({"text": "hello"}),
            depends_on: Vec::new(),
            postconditions: vec![json!({"kind": "element_exists"})],
            effect: StepEffect::Write,
            reversibility: Reversibility::L0UndoStack,
            point_of_no_return: false,
            timeouts: StepTimeouts::new(500, 2_000, 1_000).expect("timeouts"),
        }],
        budget: Budget::new(10, 60_000, 1_000, 0.5).expect("budget"),
        checkpoint_policy: CheckpointPolicy::AfterEachTransition,
    };
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine.create_task(plan, 1_000).expect("create");
    engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit");
    engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve");
    (HitlCoordinator::new(engine), task_id)
}

#[test]
fn test_pause_and_resume_use_task_engine_transitions() {
    let (mut coordinator, task_id) = running_engine();
    let paused = coordinator.request_pause(&task_id, 2_000).expect("pause");
    assert_eq!(paused.status, TaskStatus::Paused);
    let running = coordinator
        .resume_after_pause(&task_id, 2_100)
        .expect("resume");
    assert_eq!(running.status, TaskStatus::Running);
}

#[test]
fn test_takeover_return_always_requires_resynchronization() {
    let (mut coordinator, task_id) = running_engine();
    let taken_over = coordinator
        .begin_takeover(&task_id, "before", 2_000)
        .expect("take over");
    assert_eq!(taken_over.status, TaskStatus::TakenOver);

    let assessment = coordinator
        .return_control(&task_id, "before", 2_100)
        .expect("return control");
    assert!(!assessment.has_changed);
    assert!(assessment.requires_target_resolution);
    assert!(!assessment.requires_plan_review);
    let resuming = coordinator
        .task_engine()
        .load_snapshot(&task_id)
        .expect("snapshot");
    assert_eq!(resuming.status, TaskStatus::Resuming);
    assert_eq!(
        coordinator
            .finish_resynchronization(&task_id, 2_200)
            .expect("finish")
            .status,
        TaskStatus::Running
    );
}

#[test]
fn test_changed_fingerprint_requires_plan_review() {
    let (mut coordinator, task_id) = running_engine();
    coordinator
        .begin_takeover(&task_id, "before", 2_000)
        .expect("take over");
    let assessment = coordinator
        .return_control(&task_id, "after", 2_100)
        .expect("return control");
    assert!(assessment.has_changed);
    assert!(assessment.requires_plan_review);
}

#[test]
fn test_invalid_takeover_paths_are_explicit() {
    let (mut coordinator, task_id) = running_engine();
    assert!(matches!(
        coordinator.return_control(&task_id, "fingerprint", 2_000),
        Err(HitlError::TakeoverNotActive { .. })
    ));
    coordinator
        .begin_takeover(&task_id, "before", 2_000)
        .expect("take over");
    assert!(matches!(
        coordinator.begin_takeover(&task_id, "other", 2_100),
        Err(HitlError::TakeoverAlreadyActive { .. })
    ));
    assert!(matches!(
        coordinator.return_control(&task_id, "current", 1_999),
        Err(HitlError::ClockWentBackwards { .. })
    ));
}
