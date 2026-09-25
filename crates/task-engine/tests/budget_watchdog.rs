//! Budget and watchdog boundary coverage.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use assistant_task_engine::{
    Budget, BudgetCheck, BudgetLimit, MemoryCheckpointStore, Plan, TaskEngine, TaskEvent,
    TaskHoldReason, TaskId, TaskStatus, UsageDelta, WatchdogDecision,
};
use common::{read_step, single_read_plan};

fn runnable_engine(plan: Plan) -> (TaskEngine<MemoryCheckpointStore>, TaskId) {
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

#[test]
fn test_budget_check_reports_each_dimension() {
    let budget = Budget::new(1, 10, 5, 0.1).expect("budget");
    let mut usage = assistant_task_engine::BudgetUsage {
        steps_started: 2,
        ..Default::default()
    };
    assert_eq!(
        budget.check(&usage, 0),
        BudgetCheck::Exceeded(BudgetLimit::Steps)
    );
    usage.steps_started = 1;
    assert_eq!(
        budget.check(&usage, 11),
        BudgetCheck::Exceeded(BudgetLimit::ElapsedTime)
    );
    usage.tokens_in = 6;
    assert_eq!(
        budget.check(&usage, 1),
        BudgetCheck::Exceeded(BudgetLimit::Tokens)
    );
    usage.tokens_in = 0;
    usage.cost_usd = 0.2;
    assert_eq!(
        budget.check(&usage, 1),
        BudgetCheck::Exceeded(BudgetLimit::Cost)
    );
}

#[test]
fn test_max_steps_is_persisted_as_needs_human() {
    let mut plan = single_read_plan("t_steps", "s_1");
    plan.steps = vec![
        read_step("s_1", 1, Vec::new()),
        read_step("s_2", 2, Vec::new()),
    ];
    plan.budget = Budget::new(1, 60_000, 100, 1.0).expect("budget");
    let (mut engine, task_id) = runnable_engine(plan);
    let first_step = assistant_task_engine::StepId::new("s_1").expect("step");
    let second_step = assistant_task_engine::StepId::new("s_2").expect("step");
    let first = engine
        .begin_step(&task_id, &first_step, 2_000)
        .expect("first step");
    assert_eq!(first.status, TaskStatus::Running);
    engine
        .approve_step(&task_id, &first_step, 2_010)
        .expect("approve first");
    engine
        .begin_execute(&task_id, &first_step, 2_020)
        .expect("execute first");
    engine
        .begin_verify(&task_id, &first_step, 2_030)
        .expect("verify first");
    engine
        .commit_step(&task_id, &first_step, None, false, 2_100)
        .expect("commit first");
    let exhausted = engine
        .begin_step(&task_id, &second_step, 2_200)
        .expect("second attempt is a state outcome");
    assert_eq!(exhausted.status, TaskStatus::NeedsHuman);
}

#[test]
fn test_recorded_usage_exhaustion_is_persisted() {
    let mut plan = single_read_plan("t_tokens", "s_1");
    plan.budget = Budget::new(1, 60_000, 5, 1.0).expect("budget");
    let (mut engine, task_id) = runnable_engine(plan);
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step");
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    engine
        .approve_step(&task_id, &step_id, 2_010)
        .expect("approve");
    engine
        .begin_execute(&task_id, &step_id, 2_020)
        .expect("execute");
    let snapshot = engine
        .record_usage(
            &task_id,
            &step_id,
            UsageDelta {
                tokens_in: 3,
                tokens_out: 3,
                cost_usd: 0.0,
            },
            2_030,
        )
        .expect("usage");
    assert_eq!(snapshot.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        snapshot.hold_reason,
        Some(TaskHoldReason::BudgetExceeded {
            limit: BudgetLimit::Tokens
        })
    ));
}

#[test]
fn test_watchdog_health_expiry_and_enforcement() {
    let mut plan = single_read_plan("t_watchdog", "s_1");
    plan.steps = vec![read_step("s_1", 1, Vec::new())];
    let (mut engine, task_id) = runnable_engine(plan);
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step");
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    assert!(matches!(
        engine.check_watchdog(&task_id, 2_500).expect("healthy"),
        WatchdogDecision::Healthy { .. }
    ));
    assert!(matches!(
        engine.check_watchdog(&task_id, 3_000).expect("expired"),
        WatchdogDecision::Expired { .. }
    ));
    let enforced = engine
        .enforce_watchdog(&task_id, 3_000)
        .expect("enforce watchdog");
    assert_eq!(enforced.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        enforced.hold_reason,
        Some(TaskHoldReason::WatchdogExpired { .. })
    ));
}
