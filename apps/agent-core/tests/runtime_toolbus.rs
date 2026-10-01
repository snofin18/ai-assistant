//! End-to-end runtime execution over the real in-process MCP tool bus.
//!
//! This is the TASK-103 acceptance proof for the `DoD` line "Agent-core can
//! assemble and execute a real Plan": a real `Plan` is driven by the
//! `RuntimeExecutor` through the real `ToolBus` MCP round trip, verified with a
//! real `VerificationReceipt`, and committed by the real task-engine. The
//! handler is deterministic, so the test stays inside the "no real Notepad"
//! boundary while still exercising every component on the path.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use assistant_agent_core::{
    EnvelopeObservationCollector, ReservedRuntimeInvoker, RuntimeExecutor, StepExecutionOutcome,
    StepPolicy, ToolBusInvoker, ToolInvoker,
};
use assistant_policy::Decision;
use assistant_protocol::{RiskLevel, ToolEffect, ToolReversibility, serde_json::json};
use assistant_task_engine::{
    Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep, Reversibility,
    StepEffect, StepId, StepStatus, StepTimeouts, TaskEngine, TaskEvent, TaskId, TaskSnapshot,
    TaskStatus,
};
use assistant_tool_bus::{
    CallContext, MountSelection, SystemClock, ToolBus, ToolBusConfig, ToolBusError, ToolDefinition,
    ToolHandler, ToolOutput, ToolRegistry,
};

const TOOL_NAME: &str = "notepad.text.write";
const FINGERPRINT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Deterministic Host-side handler that counts real MCP dispatches.
struct WriteTextHandler {
    calls: Arc<AtomicUsize>,
}

impl ToolHandler for WriteTextHandler {
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::json(json!({
            "text": "hello",
            "fingerprint": FINGERPRINT,
        })))
    }
}

/// Policy stub for the acceptance path: this card wires the executor, not the
/// rule engine, so the only decision used here is an unconditional allow.
#[derive(Clone, Copy)]
struct AllowPolicy;

impl StepPolicy for AllowPolicy {
    fn decide(
        &self,
        _step: &PlanStep,
    ) -> Result<Decision, assistant_agent_core::RuntimeExecutionError> {
        Ok(Decision::Allow {
            rule_id: "acceptance_allow".to_owned(),
        })
    }
}

fn write_plan() -> Plan {
    let task_id = TaskId::new("t_toolbus").expect("task id");
    Plan {
        plan_id: PlanId::new("p_toolbus").expect("plan id"),
        task_id,
        goal: "write a greeting through the real tool bus".to_owned(),
        steps: vec![PlanStep {
            id: StepId::new("s_1").expect("step id"),
            sequence: 1,
            tool: TOOL_NAME.to_owned(),
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

async fn started_bus(calls: Arc<AtomicUsize>) -> ToolBus {
    let mut registry = ToolRegistry::new();
    let definition = ToolDefinition::new(
        TOOL_NAME,
        "writes text into the target document",
        RiskLevel::Low,
        ToolEffect::Write,
        ToolReversibility::L0UndoStack,
        json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
        }),
    )
    .expect("tool definition");
    registry
        .register(definition, Arc::new(WriteTextHandler { calls }))
        .expect("register handler");
    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("acceptance-session"),
        Arc::new(SystemClock),
    )
    .await
    .expect("start tool bus");
    bus
}

fn running_engine() -> (TaskEngine<MemoryCheckpointStore>, TaskId, StepId) {
    let plan = write_plan();
    let task_id = plan.task_id.clone();
    let step_id = plan.steps.first().expect("plan has one step").id.clone();
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine.create_task(plan, 1_000).expect("create task");
    engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit for approval");
    engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve plan");
    (engine, task_id, step_id)
}

#[tokio::test]
async fn test_real_plan_commits_through_real_mcp_tool_bus() {
    let calls = Arc::new(AtomicUsize::new(0));
    let bus = started_bus(Arc::clone(&calls)).await;
    let (engine, task_id, step_id) = running_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        AllowPolicy,
        ToolBusInvoker::new(&bus),
        EnvelopeObservationCollector,
    );

    let outcome = executor
        .advance(&task_id, &step_id, 2_000)
        .await
        .expect("advance");

    match outcome {
        StepExecutionOutcome::Committed(snapshot) => {
            assert_eq!(snapshot.status, TaskStatus::Completed);
            assert_eq!(snapshot.warning_count, 0);
        }
        other => panic!("expected commit, got {other:?}"),
    }
    // Exactly one real MCP dispatch, and the post fingerprint was recorded.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let engine = executor.into_engine();
    let snapshot = engine.load_snapshot(&task_id).expect("snapshot");
    let step = snapshot.steps.first().expect("one step snapshot");
    assert_eq!(step.status, StepStatus::Committed);
    assert_eq!(step.post_fingerprint.as_deref(), Some(FINGERPRINT));
}

/// ADR-0060: a reserved runtime tool is handled by the binary layer, **never**
/// by the bus, and reports a *known* failure instead of an unknown outcome.
#[tokio::test]
async fn test_reserved_runtime_tool_is_handled_locally_as_a_known_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let bus = started_bus(Arc::clone(&calls)).await;
    let snapshot: Arc<Mutex<Option<TaskSnapshot>>> = Arc::new(Mutex::new(None));
    let invoker = ReservedRuntimeInvoker::new(&bus, snapshot);

    let plan = write_plan();
    let task_id = plan.task_id.clone();
    let mut step = plan.steps.first().expect("one step").clone();
    step.tool = "assistant.runtime.verify_postconditions".to_owned();

    let envelope = invoker
        .invoke(&task_id, &step)
        .await
        .expect("a reserved step yields an envelope, not an unknown outcome");

    assert!(
        !envelope.ok,
        "no published snapshot means verify cannot pass"
    );
    assert_eq!(
        envelope.error.as_ref().map(|error| error.code),
        Some(assistant_protocol::ErrorCode::VerifyFailed),
        "a verify step with no snapshot must fail with a stable ErrorCode"
    );
    assert!(
        envelope
            .error
            .as_ref()
            .is_some_and(|error| error.message.contains("no task snapshot")),
        "the refusal must say why, not fail silently"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a reserved tool must never reach the model-visible tool bus"
    );
}

/// ADR-0059 D3: an irreversible step cannot be anchored. `prepare_anchors` must
/// refuse **before** anything executes - that refusal is the whole point of the
/// step, so it is asserted rather than assumed.
#[tokio::test]
async fn test_prepare_anchors_refuses_an_irreversible_level() {
    let calls = Arc::new(AtomicUsize::new(0));
    let bus = started_bus(Arc::clone(&calls)).await;
    let snapshot: Arc<Mutex<Option<TaskSnapshot>>> = Arc::new(Mutex::new(None));
    let invoker = ReservedRuntimeInvoker::new(&bus, snapshot);

    let plan = write_plan();
    let task_id = plan.task_id.clone();
    let mut step = plan.steps.first().expect("one step").clone();
    step.tool = "assistant.runtime.prepare_anchors".to_owned();
    step.args = json!({
        "replace_recipe": "l1_snapshot",
        "save_recipe": "l1_snapshot",
        "required_levels": ["l1_snapshot", "l3_irreversible"],
    });

    let envelope = invoker
        .invoke(&task_id, &step)
        .await
        .expect("a reserved step yields an envelope");

    assert!(!envelope.ok, "an irreversible step must not be anchored");
    assert_eq!(
        envelope.error.as_ref().map(|error| error.code),
        Some(assistant_protocol::ErrorCode::PolicyDenied),
        "the refusal must be a policy decision, not a generic failure"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the refusal must happen before any tool call"
    );
}
