//! Public-contract tests for the planner's model-output parsing paths.
//!
//! Scope: drive `assistant_core::Planner` and `PlannerRequest` through their
//! public API only, using an in-memory fake provider. These tests add no
//! production behaviour, touch no network or filesystem, and are read-only
//! with respect to the model gateway.
//!
//! Invariants under test (architecture v2 sections 5 and 8.3, iron rule 2):
//! - a model-supplied stream that is cancelled, oversized, non-Stop, or
//!   malformed never becomes a `Plan`;
//! - request validation rejects an empty or ill-shaped tool catalog before any
//!   provider call;
//! - trusted `effect` / `reversibility` metadata is injected from the catalog,
//!   not from model output.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use assistant_core::{CoreError, Planner, PlannerRequest};
use assistant_model_gateway::{
    CancellationToken, CompletionEvent, CompletionRequest, CompletionStream, DurationMs,
    FinishReason, Message, ModelId, ModelProvider, ModelResult, Pricing, ProviderCapabilities,
    ProviderFeatures, TokenCount, Usage,
};
use assistant_protocol::ToolSchema;
use assistant_protocol::serde_json::{self, Value, json};
use assistant_task_engine::{Budget, PlanId, Reversibility, TaskId};

/// One-shot provider that replays a fixed event queue without IO.
struct FakeProvider {
    model_id: ModelId,
    events: Mutex<VecDeque<CompletionEvent>>,
    last_request: Mutex<Option<CompletionRequest>>,
}

impl FakeProvider {
    fn new(events: Vec<CompletionEvent>) -> Self {
        Self {
            model_id: ModelId::new("planner_contract_test").unwrap(),
            events: Mutex::new(events.into()),
            last_request: Mutex::new(None),
        }
    }

    fn saw_request(&self) -> bool {
        self.last_request.lock().unwrap().is_some()
    }
}

struct FakeStream {
    events: VecDeque<CompletionEvent>,
}

impl CompletionStream for FakeStream {
    fn next_event(
        &mut self,
        _cancellation: &CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        Ok(self.events.pop_front())
    }
}

impl ModelProvider for FakeProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::new(ProviderFeatures::STREAMING, TokenCount::new(8_192))
    }

    fn pricing(&self) -> Pricing {
        Pricing::new(1, 1, 1)
    }

    fn count_tokens(&self, messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(messages.len() as u64))
    }

    fn complete(
        &self,
        request: CompletionRequest,
        _cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        *self.last_request.lock().unwrap() = Some(request);
        let events = std::mem::take(&mut *self.events.lock().unwrap());
        Ok(Box::new(FakeStream { events }))
    }
}

fn budget() -> Budget {
    Budget::new(10, 60_000, 1_000, 0.5).unwrap()
}

fn tool_schema(name: &str, effect: &str, reversibility: &str, risk_level: &str) -> ToolSchema {
    serde_json::from_value(json!({
        "version": "1.0",
        "name": name,
        "description": format!("test tool {name}"),
        "input": {"type": "object"},
        "output": {"type": "object"},
        "effect": effect,
        "reversibility": reversibility,
        "risk_level": risk_level,
        "requires_approval": risk_level == "critical" || reversibility == "l3_irreversible"
    }))
    .unwrap()
}

fn tool(name: &str) -> ToolSchema {
    tool_schema(name, "read", "l0_undo_stack", "low")
}

fn planner_request(tools: Vec<ToolSchema>) -> PlannerRequest {
    PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "read the document",
        tools,
        budget(),
    )
    .unwrap()
}

fn read_step(id: &str, sequence: u32, tool_name: &str) -> Value {
    json!({
        "id": id,
        "sequence": sequence,
        "tool": tool_name,
        "args": {},
        "depends_on": [],
        "postconditions": [],
        "point_of_no_return": false,
        "timeouts": {"resolve_ms": 100, "execute_ms": 200, "verify_ms": 100}
    })
}

fn write_step(id: &str, sequence: u32, tool_name: &str) -> Value {
    json!({
        "id": id,
        "sequence": sequence,
        "tool": tool_name,
        "args": {"text": "hello"},
        "depends_on": [],
        "postconditions": [{"kind": "element_exists"}],
        "point_of_no_return": false,
        "timeouts": {"resolve_ms": 100, "execute_ms": 200, "verify_ms": 100}
    })
}

fn plan_output(steps: &Value) -> String {
    json!({"steps": steps}).to_string()
}

fn planner_for(events: Vec<CompletionEvent>) -> Planner {
    Planner::new(Arc::new(FakeProvider::new(events)))
}

fn stop_after(output: &str) -> Vec<CompletionEvent> {
    vec![
        CompletionEvent::TextDelta(output.to_string()),
        CompletionEvent::Finished(FinishReason::Stop),
    ]
}

#[test]
fn test_planner_request_rejects_empty_tool_catalog() {
    let error = PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "goal",
        Vec::new(),
        budget(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        CoreError::InvalidContent {
            field: "planner.tools",
            ..
        }
    ));
}

#[test]
fn test_planner_request_rejects_non_object_tool_output() {
    let schema = serde_json::from_value::<ToolSchema>(json!({
        "version": "1.0",
        "name": "notepad.text.read",
        "description": "bad output",
        "input": {"type": "object"},
        "output": "not an object",
        "effect": "read",
        "reversibility": "l0_undo_stack",
        "risk_level": "low"
    }))
    .unwrap();
    let error = PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "goal",
        vec![schema],
        budget(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        CoreError::InvalidContent {
            field: "planner.tools.output",
            ..
        }
    ));
}

#[test]
fn test_planner_request_accepts_well_formed_tool_tags() {
    let schema = serde_json::from_value::<ToolSchema>(json!({
        "version": "1.0",
        "name": "notepad.text.read",
        "description": "tagged tool",
        "input": {"type": "object"},
        "output": {"type": "object"},
        "effect": "read",
        "reversibility": "l0_undo_stack",
        "risk_level": "low",
        "tags": ["readonly", "notepad_1"]
    }))
    .unwrap();

    let request = PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "goal",
        vec![schema],
        budget(),
    )
    .unwrap();

    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.tools[0].tags.len(), 2);
}

#[test]
fn test_planner_request_rejects_empty_tool_name_segment() {
    let error = PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "goal",
        vec![tool("notepad..read")],
        budget(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        CoreError::InvalidContent {
            field: "planner.tools.name",
            ..
        }
    ));
}

#[test]
fn test_planner_request_rejects_control_characters_in_goal() {
    let error = PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "bad\u{7}goal",
        vec![tool("notepad.text.read")],
        budget(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        CoreError::InvalidContent {
            field: "planner.goal",
            ..
        }
    ));
}

#[test]
fn test_planner_event_timeout_accepts_the_inclusive_bounds() {
    let output = plan_output(&json!([read_step("s_1", 1, "notepad.text.read")]));
    let planner = planner_for(stop_after(&output))
        .with_event_timeout(DurationMs::new(1))
        .unwrap()
        .with_event_timeout(DurationMs::new(500))
        .unwrap();
    let plan = planner
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(plan.steps.len(), 1);
}

#[test]
fn test_planner_event_timeout_rejects_zero_and_over_ceiling() {
    for milliseconds in [0_u64, 501] {
        let result = planner_for(Vec::new()).with_event_timeout(DurationMs::new(milliseconds));

        assert!(matches!(
            result,
            Err(CoreError::InvalidContent {
                field: "planner.event_timeout",
                ..
            })
        ));
    }
}

#[test]
fn test_planner_rejects_pre_cancelled_token_without_calling_provider() {
    let provider = Arc::new(FakeProvider::new(stop_after("{}")));
    let planner = Planner::new(provider.clone());
    let token = CancellationToken::new();
    token.cancel();

    let error = planner
        .generate_plan(&planner_request(vec![tool("notepad.text.read")]), token)
        .unwrap_err();

    assert!(matches!(error, CoreError::PlannerModel(_)));
    assert!(!provider.saw_request());
}

#[test]
fn test_planner_rejects_oversized_output() {
    let oversized = "a".repeat(1_048_577);
    let error = planner_for(stop_after(&oversized))
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_ignores_usage_events_before_stop() {
    let output = plan_output(&json!([read_step("s_1", 1, "notepad.text.read")]));
    let provider = FakeProvider::new(vec![
        CompletionEvent::Usage(Usage::new(
            TokenCount::new(5),
            TokenCount::new(0),
            TokenCount::new(2),
        )),
        CompletionEvent::TextDelta(output),
        CompletionEvent::Finished(FinishReason::Stop),
    ]);

    let plan = Planner::new(Arc::new(provider))
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(plan.steps.len(), 1);
}

#[test]
fn test_planner_rejects_non_stop_finish_reason() {
    let error = planner_for(vec![
        CompletionEvent::TextDelta("{}".to_string()),
        CompletionEvent::Finished(FinishReason::Length),
    ])
    .generate_plan(
        &planner_request(vec![tool("notepad.text.read")]),
        CancellationToken::new(),
    )
    .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_rejects_empty_output() {
    let error = planner_for(vec![CompletionEvent::Finished(FinishReason::Stop)])
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_rejects_non_object_envelope() {
    for output in ["[]", "\"steps\"", "42"] {
        let error = planner_for(stop_after(output))
            .generate_plan(
                &planner_request(vec![tool("notepad.text.read")]),
                CancellationToken::new(),
            )
            .unwrap_err();

        assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
    }
}

#[test]
fn test_planner_rejects_step_that_violates_plan_step_contract() {
    let mut step = read_step("s_1", 1, "notepad.text.read");
    step["sequence"] = json!("not-a-number");
    let error = planner_for(stop_after(&plan_output(&json!([step]))))
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_rejects_non_object_step() {
    let error = planner_for(stop_after(&plan_output(&json!(["not-an-object"]))))
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_rejects_step_without_tool_field() {
    let mut step = read_step("s_1", 1, "notepad.text.read");
    step.as_object_mut().unwrap().remove("tool");
    let error = planner_for(stop_after(&plan_output(&json!([step]))))
        .generate_plan(
            &planner_request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_projects_l2_compensation_tool_metadata() {
    let tools = vec![tool_schema(
        "notepad.text.write",
        "write",
        "l2_compensation",
        "medium",
    )];
    let plan = planner_for(stop_after(&plan_output(&json!([write_step(
        "s_1",
        1,
        "notepad.text.write"
    )]))))
    .generate_plan(&planner_request(tools), CancellationToken::new())
    .unwrap();

    assert_eq!(plan.steps[0].reversibility, Reversibility::L2Compensation);
}
