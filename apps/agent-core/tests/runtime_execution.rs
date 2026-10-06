//! Contract tests for the binary-layer runtime execution coordinator.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use assistant_agent_core::{
    ObservationCollector, RuntimeExecutionError, RuntimeExecutor, StepExecutionOutcome, StepPolicy,
    ToolInvoker,
};
use assistant_platform_api::Fingerprint;
use assistant_policy::{Decision, ScopeOption};
use assistant_protocol::{ErrorCode, ToolEnvelope, serde_json::json};
use assistant_task_engine::{
    Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep, Reversibility,
    StepEffect, StepId, StepTimeouts, TaskEngine, TaskEvent, TaskId, TaskStatus,
};
use assistant_verify::{GrayImage, Observation, VisualObservation};

#[derive(Clone)]
struct AllowPolicy;

impl StepPolicy for AllowPolicy {
    fn decide(&self, _step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        Ok(Decision::Allow {
            rule_id: "allow_test".to_owned(),
        })
    }
}

#[derive(Clone)]
struct DenyPolicy;

impl StepPolicy for DenyPolicy {
    fn decide(&self, _step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        Ok(Decision::Deny {
            rule_id: "deny_test".to_owned(),
            reason: "test denial".to_owned(),
        })
    }
}

#[derive(Clone)]
struct ConfirmPolicy;

impl StepPolicy for ConfirmPolicy {
    fn decide(&self, _step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        Ok(Decision::AllowWithConfirmation {
            rule_id: "confirm_test".to_owned(),
            scope_options: vec![ScopeOption::Once],
            show_diff: true,
        })
    }
}

struct RecordingInvoker {
    events: Arc<Mutex<Vec<&'static str>>>,
    mode: InvokeMode,
}

#[derive(Clone, Copy)]
enum InvokeMode {
    Succeed,
    TransportFailure,
    ErrorEnvelope,
}

impl ToolInvoker for RecordingInvoker {
    fn invoke(
        &self,
        _task_id: &TaskId,
        step: &PlanStep,
    ) -> impl std::future::Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send {
        self.events.lock().expect("events").push("invoke");
        let result = match self.mode {
            InvokeMode::Succeed => Ok(ToolEnvelope::ok(
                step.tool.clone(),
                "t_runtime".to_owned(),
                step.id.to_string(),
                json!({"text": "hello"}),
            )),
            InvokeMode::TransportFailure => Err(RuntimeExecutionError::Tool {
                reason: "host disconnected".to_owned(),
            }),
            InvokeMode::ErrorEnvelope => Ok(ToolEnvelope::error(
                step.tool.clone(),
                "t_runtime".to_owned(),
                step.id.to_string(),
                ErrorCode::TargetNotFound,
                "target window is gone",
            )),
        };
        async move { result }
    }
}

struct RecordingObserver {
    events: Arc<Mutex<Vec<&'static str>>>,
    text: &'static str,
    visual: Option<VisualObservation>,
}

impl ObservationCollector for RecordingObserver {
    fn observe(
        &self,
        _step: &PlanStep,
        _envelope: &ToolEnvelope,
    ) -> Result<Observation, RuntimeExecutionError> {
        self.events.lock().expect("events").push("observe");
        let fingerprint =
            Fingerprint::parse(format!("sha256:{}", "a".repeat(64))).expect("fingerprint");
        Ok(Observation {
            text: self.text.to_owned(),
            ..Observation::new("document.body", "test", fingerprint)
        })
    }

    fn observe_visual(
        &self,
        _step: &PlanStep,
        _envelope: &ToolEnvelope,
    ) -> Result<Option<VisualObservation>, RuntimeExecutionError> {
        self.events.lock().expect("events").push("observe_visual");
        Ok(self.visual.clone())
    }
}

fn plan() -> Plan {
    let task_id = TaskId::new("t_runtime").expect("task");
    Plan {
        plan_id: PlanId::new("p_runtime").expect("plan"),
        task_id,
        goal: "runtime contract".to_owned(),
        steps: vec![PlanStep {
            id: StepId::new("s_1").expect("step"),
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
    let plan = plan();
    let task_id = plan.task_id.clone();
    let step_id = plan.steps.first().expect("plan has one step").id.clone();
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

fn visual_plan() -> Plan {
    let task_id = TaskId::new("t_visual_runtime").expect("task");
    Plan {
        plan_id: PlanId::new("p_visual_runtime").expect("plan"),
        task_id,
        goal: "visual runtime contract".to_owned(),
        steps: vec![PlanStep {
            id: StepId::new("s_visual").expect("step"),
            sequence: 1,
            tool: "paint.canvas.capture".to_owned(),
            args: json!({}),
            depends_on: Vec::new(),
            postconditions: vec![json!({
                "kind": "visual_assert",
                "field": "pixels",
                "op": "mean_abs_diff_within",
                "max_mean_abs_diff": 0,
                "confidence_min": 0.8
            })],
            effect: StepEffect::Write,
            reversibility: Reversibility::L0UndoStack,
            point_of_no_return: false,
            timeouts: StepTimeouts::new(500, 2_000, 1_000).expect("timeouts"),
        }],
        budget: Budget::new(10, 60_000, 10_000, 1.0).expect("budget"),
        checkpoint_policy: CheckpointPolicy::AfterEachTransition,
    }
}

fn running_visual_engine() -> (TaskEngine<MemoryCheckpointStore>, TaskId, StepId) {
    let plan = visual_plan();
    let task_id = plan.task_id.clone();
    let step_id = plan.steps.first().expect("plan has one step").id.clone();
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

fn matching_visual_observation() -> VisualObservation {
    let image = GrayImage::new(1, 1, vec![128]).expect("valid image");
    VisualObservation::new(image.clone(), image, 0.95).expect("valid visual observation")
}

#[tokio::test]
async fn test_verified_step_is_committed_only_after_tool_and_observation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::Succeed,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    assert!(matches!(outcome, StepExecutionOutcome::Committed(_)));
    assert_eq!(
        *events.lock().expect("events"),
        vec!["invoke", "observe", "observe_visual"]
    );
}

#[tokio::test]
async fn test_policy_denial_prevents_tool_invocation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        DenyPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::Succeed,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    assert!(matches!(outcome, StepExecutionOutcome::PolicyDenied(_)));
    assert!(events.lock().expect("events").is_empty());
}

#[tokio::test]
async fn test_confirmation_required_prevents_tool_invocation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        ConfirmPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::Succeed,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    assert!(matches!(outcome, StepExecutionOutcome::AwaitingApproval));
    assert!(events.lock().expect("events").is_empty());
}

#[tokio::test]
async fn test_failed_verification_does_not_commit() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::Succeed,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "wrong",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    match outcome {
        StepExecutionOutcome::VerificationFailed { snapshot, .. } => {
            assert_eq!(snapshot.status, TaskStatus::Failed);
        }
        other => panic!("expected verification failure, got {other:?}"),
    }
}

#[tokio::test]
async fn test_unknown_tool_outcome_requires_human() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::TransportFailure,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    match outcome {
        StepExecutionOutcome::NeedsHuman { snapshot, reason } => {
            assert_eq!(snapshot.status, TaskStatus::NeedsHuman);
            assert!(reason.contains("tool outcome unknown"), "{reason}");
        }
        other => panic!("expected NeedsHuman, got {other:?}"),
    }
    assert_eq!(*events.lock().expect("events"), vec!["invoke"]);
}

#[tokio::test]
async fn test_error_envelope_fails_step_without_verification() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::ErrorEnvelope,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    match outcome {
        StepExecutionOutcome::ToolFailed {
            snapshot,
            code,
            message,
        } => {
            assert_eq!(snapshot.status, TaskStatus::Failed);
            assert_eq!(snapshot.last_error_code, Some(ErrorCode::TargetNotFound));
            assert_eq!(code, ErrorCode::TargetNotFound);
            assert_eq!(message, "target window is gone");
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    // The tool ran, so the observer must not run: no verification without a result.
    assert_eq!(*events.lock().expect("events"), vec!["invoke"]);
}

#[tokio::test]
async fn test_visual_assert_commits_when_runtime_collects_matching_images() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_visual_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::Succeed,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: Some(matching_visual_observation()),
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    assert!(matches!(outcome, StepExecutionOutcome::Committed(_)));
    assert_eq!(
        *events.lock().expect("events"),
        vec!["invoke", "observe", "observe_visual"]
    );
}

#[tokio::test]
async fn test_visual_assert_without_runtime_images_stays_unevaluable() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (engine, task_id, step_id) = running_visual_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        RecordingInvoker {
            events: Arc::clone(&events),
            mode: InvokeMode::Succeed,
        },
        RecordingObserver {
            events: Arc::clone(&events),
            text: "hello",
            visual: None,
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");
    match outcome {
        StepExecutionOutcome::VerificationFailed { snapshot, .. } => {
            assert_eq!(snapshot.status, TaskStatus::Failed);
        }
        other => panic!("expected visual verification failure, got {other:?}"),
    }
}
