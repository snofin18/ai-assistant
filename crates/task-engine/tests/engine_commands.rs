//! Command-level coverage for persistence-first engine behavior.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;

use assistant_protocol::ErrorCode;
use assistant_task_engine::{
    Budget, BudgetCheck, MemoryCheckpointStore, Plan, RecoveryEvidence, StepId, TaskEngine,
    TaskEvent, TaskHoldReason, TaskId, TaskStatus, UsageDelta, WatchdogDecision,
};
use common::{read_step, single_read_plan};

fn running_engine(plan: Plan) -> (TaskEngine<MemoryCheckpointStore>, TaskId) {
    let task_id = plan.task_id.clone();
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine.create_task(plan, 1_000).expect("create");
    engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit");
    engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve");
    (engine, task_id)
}

fn complete_step(
    engine: &mut TaskEngine<MemoryCheckpointStore>,
    task_id: &TaskId,
    step_id: &StepId,
    warning: bool,
    now_ms: i64,
) -> assistant_task_engine::TaskSnapshot {
    engine.begin_step(task_id, step_id, now_ms).expect("begin");
    finish_active_step(engine, task_id, step_id, warning, now_ms + 10)
}

fn finish_active_step(
    engine: &mut TaskEngine<MemoryCheckpointStore>,
    task_id: &TaskId,
    step_id: &StepId,
    warning: bool,
    now_ms: i64,
) -> assistant_task_engine::TaskSnapshot {
    engine
        .approve_step(task_id, step_id, now_ms)
        .expect("approve step");
    engine
        .begin_execute(task_id, step_id, now_ms + 10)
        .expect("execute");
    engine
        .begin_verify(task_id, step_id, now_ms + 20)
        .expect("verify");
    engine
        .commit_step(
            task_id,
            step_id,
            Some("fp_after".to_owned()),
            warning,
            now_ms + 30,
        )
        .expect("commit")
}

#[test]
fn test_fail_step_path() {
    let task_id = TaskId::new("t_fail").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    let failed = engine
        .fail_step(&task_id, &step_id, ErrorCode::TargetNotFound, 2_100)
        .expect("fail");
    assert_eq!(failed.status, TaskStatus::Failed);
    assert_eq!(failed.last_error_code, Some(ErrorCode::TargetNotFound));
}

#[test]
fn test_deny_step_path() {
    let task_id = TaskId::new("t_deny").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    let denied = engine.deny_step(&task_id, &step_id, 2_100).expect("deny");
    assert_eq!(denied.status, TaskStatus::Failed);
    assert_eq!(denied.last_error_code, Some(ErrorCode::PolicyDenied));
}

#[test]
fn test_pause_takeover_and_resume_paths() {
    let task_id = TaskId::new("t_pause").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    let pending_pause = engine.request_pause(&task_id, 2_010).expect("pause");
    assert!(pending_pause.pause_requested);
    let paused = finish_active_step(&mut engine, &task_id, &step_id, false, 2_100);
    assert_eq!(paused.status, TaskStatus::Paused);
    let running = engine.resume(&task_id, 2_200).expect("resume");
    assert_eq!(running.status, TaskStatus::Running);

    let taken_over = engine.take_over(&task_id, 2_300).expect("take over");
    assert_eq!(taken_over.status, TaskStatus::TakenOver);
    let resuming = engine.resume(&task_id, 2_400).expect("return control");
    assert_eq!(resuming.status, TaskStatus::Resuming);
    let running = engine
        .apply_task_event(&task_id, TaskEvent::FinishRecovery, 2_500)
        .expect("finish recovery");
    assert_eq!(running.status, TaskStatus::Running);
}

#[test]
fn test_cancel_before_and_during_step() {
    let task_id = TaskId::new("t_cancel_idle").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    let cancelled = engine.request_cancel(&task_id, 2_000).expect("cancel idle");
    assert_eq!(cancelled.status, TaskStatus::Cancelled);

    let active_task = TaskId::new("t_cancel_active").expect("task");
    let active_step = StepId::new("s_1").expect("step");
    let (mut active_engine, active_task) =
        running_engine(single_read_plan(active_task.as_str(), active_step.as_str()));
    active_engine
        .begin_step(&active_task, &active_step, 2_000)
        .expect("begin");
    let requested = active_engine
        .request_cancel(&active_task, 2_010)
        .expect("cancel active");
    assert!(requested.cancel_requested);
    let cancelled =
        finish_active_step(&mut active_engine, &active_task, &active_step, false, 2_100);
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
}

#[test]
fn test_completion_warning_and_human_gate_paths() {
    let task_id = TaskId::new("t_warning").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    let completed = complete_step(&mut engine, &task_id, &step_id, true, 2_000);
    assert_eq!(completed.status, TaskStatus::CompletedWithWarnings);

    let human_task = TaskId::new("t_human").expect("task");
    let human_step = StepId::new("s_1").expect("step");
    let (mut human_engine, human_task) =
        running_engine(single_read_plan(human_task.as_str(), human_step.as_str()));
    let human = human_engine
        .apply_task_event(&human_task, TaskEvent::RequireHuman, 2_000)
        .expect("require human");
    assert_eq!(human.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        human.hold_reason,
        Some(TaskHoldReason::ExternalBlocked { .. })
    ));
}

#[test]
fn test_invalid_command_paths_are_explicit() {
    let draft_task = TaskId::new("t_draft").expect("task");
    let draft_step = StepId::new("s_1").expect("step");
    let mut draft_engine = TaskEngine::new(MemoryCheckpointStore::new());
    draft_engine
        .create_task(
            single_read_plan(draft_task.as_str(), draft_step.as_str()),
            1_000,
        )
        .expect("create");
    assert!(
        draft_engine
            .begin_step(&draft_task, &draft_step, 1_100)
            .is_err()
    );
    assert!(matches!(
        draft_engine
            .check_watchdog(&draft_task, 1_100)
            .expect("watchdog"),
        WatchdogDecision::Idle
    ));

    let missing_task = TaskId::new("t_missing").expect("task");
    assert!(draft_engine.load_snapshot(&missing_task).is_err());
    assert!(
        draft_engine
            .record_usage(
                &draft_task,
                &StepId::new("s_missing").expect("step"),
                UsageDelta {
                    tokens_in: 0,
                    tokens_out: 0,
                    cost_usd: 0.0,
                },
                1_100,
            )
            .is_err()
    );
}

#[test]
fn test_unready_and_active_step_guards() {
    let task_id = TaskId::new("t_guard").expect("task");
    let plan = Plan {
        plan_id: assistant_task_engine::PlanId::new("p_guard").expect("plan"),
        task_id,
        goal: "guard test".to_owned(),
        steps: vec![
            read_step("s_1", 1, Vec::new()),
            read_step("s_2", 2, vec!["s_1"]),
            read_step("s_3", 3, Vec::new()),
        ],
        budget: Budget::new(10, 60_000, 1_000, 1.0).expect("budget"),
        checkpoint_policy: assistant_task_engine::CheckpointPolicy::AfterEachTransition,
    };
    let (mut engine, task_id) = running_engine(plan);
    let first = StepId::new("s_1").expect("step");
    let dependent = StepId::new("s_2").expect("step");
    let independent = StepId::new("s_3").expect("step");
    engine.begin_step(&task_id, &first, 2_000).expect("begin");
    assert!(engine.begin_step(&task_id, &dependent, 2_010).is_err());
    assert!(engine.begin_step(&task_id, &independent, 2_020).is_err());
}

#[test]
fn test_record_usage_requires_active_step() {
    let task_id = TaskId::new("t_usage_state").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    assert!(
        engine
            .record_usage(
                &task_id,
                &step_id,
                UsageDelta {
                    tokens_in: 1,
                    tokens_out: 1,
                    cost_usd: 0.0,
                },
                2_010,
            )
            .is_err()
    );
}

#[test]
fn test_budget_and_watchdog_within_paths() {
    let task_id = TaskId::new("t_within").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    assert_eq!(
        engine.check_budget(&task_id, 2_100).expect("budget"),
        BudgetCheck::Within
    );
    assert_eq!(
        engine
            .enforce_budget(&task_id, 2_100)
            .expect("enforce")
            .status,
        TaskStatus::Running
    );
    assert_eq!(
        engine
            .enforce_watchdog(&task_id, 2_100)
            .expect("watchdog")
            .status,
        TaskStatus::Running
    );
}

#[test]
fn test_enforce_budget_escalates() {
    let task_id = TaskId::new("t_budget_enforce").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    let escalated = engine.enforce_budget(&task_id, 61_100).expect("enforce");
    assert_eq!(escalated.status, TaskStatus::NeedsHuman);
}

#[test]
fn test_engine_recovery_uses_external_evidence() {
    let task_id = TaskId::new("t_recover_engine").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    engine
        .approve_step(&task_id, &step_id, 2_010)
        .expect("approve");
    engine
        .begin_execute(&task_id, &step_id, 2_020)
        .expect("execute");
    let evidence = BTreeMap::from([(step_id.clone(), RecoveryEvidence::NotCompleted)]);
    let assessment = engine.recover(&task_id, &evidence, 2_100).expect("recover");
    assert_eq!(assessment.snapshot.status, TaskStatus::Resuming);
    assert_eq!(
        engine.load_snapshot(&task_id).expect("reload").status,
        TaskStatus::Resuming
    );
}

#[test]
fn test_clock_and_completion_guards() {
    let task_id = TaskId::new("t_guards").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    assert!(
        engine
            .apply_task_event(&task_id, TaskEvent::Pause, 1_000)
            .is_err()
    );
    assert!(
        engine
            .apply_task_event(&task_id, TaskEvent::Complete, 2_000)
            .is_err()
    );
}

#[test]
fn test_usage_overflow_is_rejected() {
    let task_id = TaskId::new("t_overflow").expect("task");
    let step_id = StepId::new("s_1").expect("step");
    let (mut engine, task_id) =
        running_engine(single_read_plan(task_id.as_str(), step_id.as_str()));
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    engine
        .approve_step(&task_id, &step_id, 2_010)
        .expect("approve");
    engine
        .begin_execute(&task_id, &step_id, 2_020)
        .expect("execute");
    let exhausted = engine
        .record_usage(
            &task_id,
            &step_id,
            UsageDelta {
                tokens_in: u64::MAX,
                tokens_out: 0,
                cost_usd: 0.0,
            },
            2_030,
        )
        .expect("single max value fits");
    assert_eq!(exhausted.status, TaskStatus::NeedsHuman);
    assert!(
        engine
            .record_usage(
                &task_id,
                &step_id,
                UsageDelta {
                    tokens_in: 1,
                    tokens_out: 0,
                    cost_usd: 0.0,
                },
                2_040,
            )
            .is_err()
    );
}
