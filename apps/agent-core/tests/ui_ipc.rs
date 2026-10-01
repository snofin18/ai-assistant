//! Contract tests for the UI-facing command and event boundary.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use assistant_agent_core::{
    ApprovalGrants, TaskControlHandler, UI_IPC_VERSION, UiAuthorizationScope, UiCommand,
    UiCommandError, UiCommandHandler, UiCommandOutcome, UiEvent, dispatch_ui_command,
    parse_ui_command, project_snapshot_events,
};
use assistant_hitl::ApprovalScope;
use assistant_platform_api::Fingerprint;
use assistant_protocol::{ErrorCode, serde_json::Value, serde_json::json};
use assistant_storage::SystemClock;
use assistant_task_engine::{
    Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep, Reversibility,
    StepCommit, StepEffect, StepId, StepStatus, StepTimeouts, TaskEngine, TaskEvent, TaskId,
    TaskStatus,
};
use assistant_verify::{Observation, parse_postconditions, verify_postconditions_with_receipt};

/// Records what a handler was asked to apply.
#[derive(Default)]
struct RecordingHandler {
    seen: Vec<UiCommand>,
}

impl UiCommandHandler for RecordingHandler {
    fn handle(&mut self, command: UiCommand) -> Result<UiCommandOutcome, UiCommandError> {
        self.seen.push(command);
        Ok(UiCommandOutcome::IntentAccepted {
            intent_id: "recorded".to_owned(),
        })
    }
}

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ui_ipc")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn read_plan() -> Plan {
    let task_id = TaskId::new("t_ui").expect("task id");
    Plan {
        plan_id: PlanId::new("p_ui").expect("plan id"),
        task_id,
        goal: "ui contract".to_owned(),
        steps: vec![PlanStep {
            id: StepId::new("s_1").expect("step id"),
            sequence: 1,
            tool: "notepad.text.write".to_owned(),
            args: json!({"text": "hello"}),
            depends_on: Vec::new(),
            postconditions: vec![json!({"kind": "text_contains", "value": "hello"})],
            effect: StepEffect::Write,
            reversibility: Reversibility::L0UndoStack,
            point_of_no_return: false,
            timeouts: StepTimeouts::new(500, 2_000, 1_000).expect("timeouts"),
        }],
        budget: Budget::new(10, 60_000, 10_000, 1.0).expect("budget"),
        checkpoint_policy: CheckpointPolicy::AfterEachTransition,
    }
}

fn running_engine() -> (TaskEngine<MemoryCheckpointStore>, TaskId, StepId) {
    let plan = read_plan();
    let task_id = plan.task_id.clone();
    let step_id = plan.steps.first().expect("one step").id.clone();
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine.create_task(plan, 1_000).expect("create");
    engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit");
    engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve");
    (engine, task_id, step_id)
}

fn verified_receipt() -> assistant_verify::VerificationReceipt {
    let postconditions = parse_postconditions(&[json!({
        "kind": "text_contains",
        "value": "hello"
    })])
    .expect("postcondition");
    let fingerprint =
        Fingerprint::parse(format!("sha256:{}", "b".repeat(64))).expect("fingerprint");
    let observation = Observation {
        text: "hello".to_owned(),
        ..Observation::new("document.body", "ui contract", fingerprint)
    };
    verify_postconditions_with_receipt(&postconditions, &observation)
        .expect("satisfied postconditions mint a receipt")
}

#[test]
fn test_golden_fixtures_parse_into_the_contract() {
    assert_eq!(UI_IPC_VERSION, "1.0");
    assert_eq!(
        parse_ui_command(&fixture("submit_intent.json")).expect("submit intent"),
        UiCommand::SubmitIntent {
            intent_id: "i_1".to_owned(),
            goal: "把报表替换成报告并保存".to_owned(),
        }
    );
    assert_eq!(
        parse_ui_command(&fixture("approve_request.json")).expect("approve"),
        UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        }
    );
    assert_eq!(
        parse_ui_command(&fixture("deny_request.json")).expect("deny"),
        UiCommand::DenyRequest {
            request_id: "a_1".to_owned(),
            reason: "user rejected the diff".to_owned(),
        }
    );
    assert_eq!(
        parse_ui_command(&fixture("take_over_task.json")).expect("take over"),
        UiCommand::TakeOverTask {
            task_id: "t_1".to_owned(),
        }
    );
}

#[test]
fn test_ui_command_reaches_the_handler_exactly_once() {
    let mut handler = RecordingHandler::default();
    let outcome = dispatch_ui_command(&mut handler, &fixture("approve_request.json"))
        .expect("valid command dispatches");
    assert!(matches!(outcome, UiCommandOutcome::IntentAccepted { .. }));
    assert_eq!(handler.seen.len(), 1);
    assert_eq!(
        handler.seen.first(),
        Some(&UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        })
    );
}

#[test]
fn test_illegal_payloads_never_reach_the_handler() {
    for (name, expected_code) in [
        ("invalid_unknown_field.json", ErrorCode::ToolInvalidArgs),
        ("invalid_unknown_kind.json", ErrorCode::ToolInvalidArgs),
        ("invalid_version.json", ErrorCode::ToolInvalidArgs),
    ] {
        let mut handler = RecordingHandler::default();
        let error = dispatch_ui_command(&mut handler, &fixture(name))
            .expect_err("illegal payload must be rejected");
        assert_eq!(error.error_code(), expected_code, "{name}");
        assert!(handler.seen.is_empty(), "{name} must not reach the handler");
    }
}

#[test]
fn test_unknown_field_is_named_in_the_rejection() {
    let error = parse_ui_command(&fixture("invalid_unknown_field.json"))
        .expect_err("unknown field must be rejected");
    assert!(matches!(
        error,
        UiCommandError::UnknownField { ref field } if field == "approved_by"
    ));
}

#[test]
fn test_denial_stops_the_referenced_step() {
    let (mut engine, task_id, step_id) = running_engine();
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock));
    handler
        .register_pending_approval(
            "a_1",
            task_id.clone(),
            step_id.clone(),
            vec![UiAuthorizationScope::Once],
        )
        .expect("register approval");

    let outcome = handler
        .handle(UiCommand::DenyRequest {
            request_id: "a_1".to_owned(),
            reason: "user rejected the diff".to_owned(),
        })
        .expect("denial applies");
    match outcome {
        UiCommandOutcome::ApprovalDenied { task_status, .. } => {
            assert_eq!(task_status, "failed");
        }
        other => panic!("expected denial, got {other:?}"),
    }
    let engine = handler.into_engine();
    let snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    assert_eq!(snapshot.status, TaskStatus::Failed);
    let step = snapshot.steps.first().expect("one step");
    assert_eq!(step.status, StepStatus::PolicyDenied);
}

#[test]
fn test_approval_scope_must_have_been_offered() {
    let (engine, task_id, step_id) = running_engine();
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock));
    handler
        .register_pending_approval("a_1", task_id, step_id, vec![UiAuthorizationScope::Once])
        .expect("register approval");
    let error = handler
        .handle(UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Persistent,
        })
        .expect_err("un-offered scope must be rejected");
    assert!(matches!(error, UiCommandError::ScopeNotOffered { .. }));
    assert_eq!(error.error_code(), ErrorCode::UserInteraction);
}

#[test]
fn test_ui_approval_releases_the_runtime_step_exactly_once() {
    let (engine, task_id, step_id) = running_engine();
    let approvals = Arc::new(ApprovalGrants::new());
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock))
        .with_approvals(Arc::clone(&approvals));
    handler
        .register_pending_approval(
            "a_1",
            task_id.clone(),
            step_id.clone(),
            vec![UiAuthorizationScope::Once],
        )
        .expect("register approval");

    let outcome = handler
        .handle(UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        })
        .expect("approval applies");
    assert_eq!(
        outcome,
        UiCommandOutcome::ApprovalGranted {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        }
    );
    assert!(
        matches!(
            approvals.consume(task_id.as_str(), step_id.as_str(), 2_000),
            Ok(Some(ApprovalScope::Once))
        ),
        "the runtime must see the human decision"
    );
    assert!(
        matches!(
            approvals.consume(task_id.as_str(), step_id.as_str(), 2_001),
            Ok(None)
        ),
        "one UI approval must release exactly one runtime execution"
    );
    assert!(matches!(
        handler
            .handle(UiCommand::ApproveRequest {
                request_id: "a_1".to_owned(),
                scope: UiAuthorizationScope::Once,
            })
            .expect_err("the consumed request is no longer pending"),
        UiCommandError::UnknownApproval { .. }
    ));
}

#[test]
fn test_ui_cannot_upgrade_a_step_approval_to_a_standing_grant() {
    let (engine, task_id, step_id) = running_engine();
    let approvals = Arc::new(ApprovalGrants::new());
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock))
        .with_approvals(Arc::clone(&approvals));
    handler
        .register_pending_approval(
            "a_1",
            task_id.clone(),
            step_id.clone(),
            vec![UiAuthorizationScope::Once, UiAuthorizationScope::Persistent],
        )
        .expect("register approval");

    let error = handler
        .handle(UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Persistent,
        })
        .expect_err("a standing grant must be refused");
    assert!(matches!(error, UiCommandError::Handler { .. }));
    assert!(
        matches!(
            approvals.consume(task_id.as_str(), step_id.as_str(), 2_000),
            Ok(None)
        ),
        "the refused standing grant must not release the step"
    );

    let outcome = handler
        .handle(UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        })
        .expect("the pending request can still receive a bounded approval");
    assert_eq!(
        outcome,
        UiCommandOutcome::ApprovalGranted {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        }
    );
}

#[test]
fn test_unknown_approval_is_rejected() {
    let (engine, _task_id, _step_id) = running_engine();
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock));
    let error = handler
        .handle(UiCommand::ApproveRequest {
            request_id: "a_missing".to_owned(),
            scope: UiAuthorizationScope::Once,
        })
        .expect_err("unknown approval must be rejected");
    assert!(matches!(error, UiCommandError::UnknownApproval { .. }));
}

#[test]
fn test_task_control_commands_move_the_state_machine() {
    let (engine, task_id, _step_id) = running_engine();
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock));
    let paused = handler
        .handle(UiCommand::PauseTask {
            task_id: task_id.to_string(),
        })
        .expect("pause applies");
    assert!(matches!(
        paused,
        UiCommandOutcome::TaskPaused { ref task_status, .. } if task_status == "paused"
    ));
    let taken_over = handler
        .handle(UiCommand::TakeOverTask {
            task_id: task_id.to_string(),
        })
        .expect("take over applies");
    assert!(matches!(
        taken_over,
        UiCommandOutcome::TaskTakenOver { ref task_status, .. } if task_status == "taken_over"
    ));
    let cancelled = handler
        .handle(UiCommand::CancelTask {
            task_id: task_id.to_string(),
        })
        .expect("cancel applies");
    assert!(matches!(
        cancelled,
        UiCommandOutcome::TaskCancelled { ref task_status, .. } if task_status == "cancelled"
    ));
}

#[test]
fn test_unknown_task_is_rejected_with_target_not_found() {
    let (engine, _task_id, _step_id) = running_engine();
    let mut handler = TaskControlHandler::new(engine, Arc::new(SystemClock));
    let error = handler
        .handle(UiCommand::PauseTask {
            task_id: "t_missing".to_owned(),
        })
        .expect_err("unknown task must be rejected");
    assert_eq!(error.error_code(), ErrorCode::TargetNotFound);
}

#[test]
fn test_committed_step_projects_a_real_verification_result() {
    let (mut engine, task_id, step_id) = running_engine();
    engine.begin_step(&task_id, &step_id, 2_000).expect("begin");
    engine
        .approve_step(&task_id, &step_id, 2_010)
        .expect("approve step");
    engine
        .begin_execute(&task_id, &step_id, 2_020)
        .expect("execute");
    engine
        .begin_verify(&task_id, &step_id, 2_030)
        .expect("verify");
    let fingerprint = format!("sha256:{}", "b".repeat(64));
    engine
        .commit_step(
            &task_id,
            &step_id,
            StepCommit::new(verified_receipt(), Some(fingerprint.clone()), false),
            2_100,
        )
        .expect("commit");

    let snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    let events = project_snapshot_events(&snapshot);
    assert!(matches!(
        events.first(),
        Some(UiEvent::TaskStateChanged { status, .. }) if status == "completed"
    ));
    let step_event = events
        .iter()
        .find_map(|event| match event {
            UiEvent::StepStateChanged {
                step_id: id,
                status,
                post_fingerprint,
                ..
            } if id == step_id.as_str() => Some((status, post_fingerprint)),
            _ => None,
        })
        .expect("step event is projected");
    assert_eq!(step_event.0, "committed");
    assert_eq!(step_event.1.as_deref(), Some(fingerprint.as_str()));
}
