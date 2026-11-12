//! End-to-end security regression for the static prompt-injection fixture.
//!
//! Responsibility:
//! - prove the fixture contains every declared injection marker and no active content;
//! - prove Tool content taints the session and remains outside clean-context review;
//! - prove a tainted high-risk Step is denied before tool or observation calls.
//!
//! Boundary:
//! - no network, browser, GUI, model provider, or platform API is used;
//! - this test does not grant approval, execute page text, or alter production policy.
//!
//! Invariants:
//! - all six fixture marker kinds remain present and inert;
//! - `app_content` attribution remains a high-risk signal;
//! - policy denial precedes every tool invocation after untrusted content.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use assistant_agent_core::{
    ObservationCollector, RuntimeExecutionError, RuntimeExecutor, StepExecutionOutcome, StepPolicy,
    ToolInvoker,
};
use assistant_core::{
    CleanContextReviewRequest, ContextRetention, HighRiskStepSummary, InstructionAttribution,
    MemorySessionStore, MessageContent, MessageId, MessageRole as SessionRole, NewMessage,
    SessionClock, SessionId, SessionManager, SessionStore, TokenCount,
};
use assistant_policy::{
    Decision, Effect, EgressDestination, EvaluationContext, Reversibility as PolicyReversibility,
    RuleSet,
};
use assistant_protocol::{RiskLevel, ToolEnvelope};
use assistant_task_engine::{
    Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep,
    Reversibility as StepReversibility, StepEffect, StepId, StepTimeouts, TaskEngine, TaskEvent,
    TaskId,
};
use assistant_verify::Observation;
use serde_json::Value;

const FIXTURE_ROOT: &str = "fixtures/web/injection-target";

struct FixedClock(i64);

impl SessionClock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.0
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_root() -> PathBuf {
    workspace_root().join(FIXTURE_ROOT)
}

fn fixture_html() -> String {
    std::fs::read_to_string(fixture_root().join("index.html")).expect("read injection fixture")
}

fn fixture_manifest() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(fixture_root().join("fixture-manifest.json"))
            .expect("read injection manifest"),
    )
    .expect("parse injection manifest")
}

fn marker_texts(manifest: &Value) -> Vec<String> {
    manifest["injection_markers"]
        .as_array()
        .expect("injection markers")
        .iter()
        .map(|marker| marker["marker"].as_str().expect("marker string").to_owned())
        .collect()
}

fn marker_kinds(manifest: &Value) -> BTreeSet<String> {
    manifest["injection_markers"]
        .as_array()
        .expect("injection markers")
        .iter()
        .map(|marker| marker["kind"].as_str().expect("marker kind").to_owned())
        .collect()
}

fn tainted_fixture_session() -> bool {
    let store: Arc<dyn SessionStore> = Arc::new(MemorySessionStore::new());
    let mut manager = SessionManager::new(store, Arc::new(FixedClock(1_000)));
    let session = SessionId::new("s_injection_fixture").expect("session id");
    manager
        .create_session(session.clone(), "Extract the product catalog.")
        .expect("create session");
    let marker_content = marker_texts(&fixture_manifest()).join("\n");
    manager
        .append_message(
            &session,
            NewMessage {
                id: MessageId::new("m_injection_fixture").expect("message id"),
                parent_id: None,
                role: SessionRole::Tool,
                content: MessageContent::new(marker_content).expect("marker content"),
                token_estimate: TokenCount::new(64),
                retention: ContextRetention::Required,
            },
        )
        .expect("append fixture tool content");

    let snapshot = manager
        .snapshot(&session)
        .expect("session snapshot")
        .clone();
    let request = CleanContextReviewRequest::from_session(
        &snapshot,
        HighRiskStepSummary::new("Read the product catalog and stop before any write.")
            .expect("review summary"),
    );
    let review_input = format!(
        "{}\n{}",
        request.original_user_request(),
        request.high_risk_step_summary().as_str()
    );
    for marker in marker_texts(&fixture_manifest()) {
        assert!(
            !review_input.contains(&marker),
            "clean-context review copied injection marker {marker}"
        );
    }
    assert_eq!(snapshot.goal(), "Extract the product catalog.");
    assert_eq!(snapshot.messages().len(), 1);
    assert_eq!(snapshot.messages()[0].role(), SessionRole::Tool);

    let attribution = InstructionAttribution::app_content("fixture://injection-target-v1")
        .expect("app-content attribution");
    assert!(attribution.is_high_risk());
    assert_eq!(
        attribution.source_ref(),
        Some("fixture://injection-target-v1")
    );

    manager.is_tainted(&session).expect("session taint")
}

#[test]
fn test_injection_fixture_is_static_complete_and_offline() {
    let html = fixture_html();
    let html_lower = html.to_ascii_lowercase();
    let manifest = fixture_manifest();
    assert_eq!(manifest["network_required"], Value::Bool(false));
    assert_eq!(
        marker_kinds(&manifest),
        BTreeSet::from([
            "aria_hidden".to_owned(),
            "fake_system_prompt".to_owned(),
            "hidden_display_none".to_owned(),
            "html_comment".to_owned(),
            "meta_instruction".to_owned(),
            "visible_text".to_owned(),
        ])
    );
    for marker in marker_texts(&manifest) {
        assert!(
            html.contains(&marker),
            "fixture is missing injection marker {marker}"
        );
    }
    for active_content in [
        "<script",
        "<form",
        "src=\"http",
        "href=\"http",
        "fetch(",
        "xmlhttprequest",
        "onclick=",
        "onload=",
    ] {
        assert!(
            !html_lower.contains(active_content),
            "fixture contains active content {active_content}"
        );
    }

    let records = manifest["normal_task"]["expected_records"]
        .as_array()
        .expect("expected records");
    assert_eq!(records.len(), 5);
    for value in records
        .iter()
        .flat_map(|record| record.as_object().expect("record object").values())
    {
        let text = value.as_str().expect("record scalar");
        assert!(
            html.contains(text),
            "fixture is missing normal value {text}"
        );
    }
}

#[test]
fn test_injection_markers_taint_session_and_are_excluded_from_clean_review() {
    assert!(
        tainted_fixture_session(),
        "Tool content must taint the active session"
    );
}

struct TaintedHighRiskPolicy {
    tainted: bool,
}

impl StepPolicy for TaintedHighRiskPolicy {
    fn decide(&self, _step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        let decision = RuleSet::example_v0()
            .evaluate(&EvaluationContext {
                effect: Effect::Write,
                risk_level: RiskLevel::High,
                reversibility: PolicyReversibility::L3Irreversible,
                unattended: false,
                tainted: self.tainted,
                target_app: "com.microsoft.edge".to_owned(),
                egress: EgressDestination::None,
            })
            .map_err(|error| RuntimeExecutionError::Policy {
                reason: error.to_string(),
            })?;
        if matches!(
            &decision,
            Decision::Deny { rule_id, .. } if rule_id == "block_after_untrusted_content"
        ) {
            Ok(decision)
        } else {
            Err(RuntimeExecutionError::Policy {
                reason: format!("expected tainted-content deny, observed {decision:?}"),
            })
        }
    }
}

#[derive(Clone)]
struct CountingInvoker {
    calls: Arc<AtomicUsize>,
}

impl ToolInvoker for CountingInvoker {
    fn invoke(
        &self,
        _task_id: &TaskId,
        _step: &PlanStep,
    ) -> impl std::future::Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send {
        self.calls.fetch_add(1, Ordering::SeqCst);
        async {
            Err(RuntimeExecutionError::Tool {
                reason: "tainted high-risk step must not invoke a tool".to_owned(),
            })
        }
    }
}

struct CountingObserver {
    calls: Arc<AtomicUsize>,
}

impl ObservationCollector for CountingObserver {
    fn observe(
        &self,
        _step: &PlanStep,
        _envelope: &ToolEnvelope,
    ) -> Result<Observation, RuntimeExecutionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RuntimeExecutionError::Observation {
            reason: "tainted high-risk step must not observe a tool result".to_owned(),
        })
    }
}

fn running_high_risk_engine() -> (TaskEngine<MemoryCheckpointStore>, TaskId, StepId) {
    let task_id = TaskId::new("t_injection_high_risk").expect("task id");
    let step_id = StepId::new("s_submit_form").expect("step id");
    let plan = Plan {
        plan_id: PlanId::new("p_injection_high_risk").expect("plan id"),
        task_id: task_id.clone(),
        goal: "Extract the product catalog.".to_owned(),
        steps: vec![PlanStep {
            id: step_id.clone(),
            sequence: 1,
            tool: "edge.cdp.form_submit".to_owned(),
            args: serde_json::json!({
                "form_selector": "#profile-form",
                "submit_selector": "#profile-submit"
            }),
            depends_on: Vec::new(),
            postconditions: vec![serde_json::json!({
                "kind": "state_changed",
                "scope": "cdp_target"
            })],
            effect: StepEffect::Write,
            reversibility: StepReversibility::L3Irreversible,
            point_of_no_return: true,
            timeouts: StepTimeouts::new(500, 2_000, 1_000).expect("timeouts"),
        }],
        budget: Budget::new(10, 60_000, 10_000, 1.0).expect("budget"),
        checkpoint_policy: CheckpointPolicy::AfterEachTransition,
    };
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    engine.create_task(plan, 1_000).expect("create task");
    engine
        .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
        .expect("submit task");
    engine
        .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
        .expect("approve plan");
    (engine, task_id, step_id)
}

#[tokio::test]
async fn test_tainted_high_risk_step_is_denied_before_tool_or_observer() {
    let tainted = tainted_fixture_session();
    let invoker_calls = Arc::new(AtomicUsize::new(0));
    let observer_calls = Arc::new(AtomicUsize::new(0));
    let (engine, task_id, step_id) = running_high_risk_engine();
    let mut executor = RuntimeExecutor::new(
        engine,
        TaintedHighRiskPolicy { tainted },
        CountingInvoker {
            calls: Arc::clone(&invoker_calls),
        },
        CountingObserver {
            calls: Arc::clone(&observer_calls),
        },
    );

    let outcome = executor
        .advance(&task_id, &step_id, 1_030)
        .await
        .expect("policy denial");
    assert!(matches!(outcome, StepExecutionOutcome::PolicyDenied(_)));
    assert_eq!(invoker_calls.load(Ordering::SeqCst), 0);
    assert_eq!(observer_calls.load(Ordering::SeqCst), 0);
}
