//! Integration tests for model-output planning.

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
    ProviderFeatures, ResponseFormat, ToolCallDelta, ToolChoice,
};
use assistant_protocol::ToolSchema;
use assistant_protocol::serde_json::{self, Value, json};
use assistant_task_engine::{Budget, MemoryCheckpointStore, PlanId, TaskEngine, TaskId};

struct FakeProvider {
    model_id: ModelId,
    events: Mutex<VecDeque<CompletionEvent>>,
    last_request: Mutex<Option<CompletionRequest>>,
}

impl FakeProvider {
    fn new(events: Vec<CompletionEvent>) -> Self {
        Self {
            model_id: ModelId::new("planner_test").unwrap(),
            events: Mutex::new(events.into()),
            last_request: Mutex::new(None),
        }
    }

    fn last_request(&self) -> CompletionRequest {
        self.last_request
            .lock()
            .unwrap()
            .clone()
            .expect("provider request")
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
        ProviderCapabilities::new(
            ProviderFeatures::STREAMING,
            assistant_model_gateway::TokenCount::new(8_192),
        )
    }

    fn pricing(&self) -> Pricing {
        Pricing::new(1, 1, 1)
    }

    fn count_tokens(
        &self,
        messages: &[Message],
    ) -> ModelResult<assistant_model_gateway::TokenCount> {
        Ok(assistant_model_gateway::TokenCount::new(
            messages.len() as u64
        ))
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

fn tool(name: &str) -> ToolSchema {
    serde_json::from_value(json!({
        "version": "1.0",
        "name": name,
        "description": format!("test tool {name}"),
        "input": {"type": "object"},
        "output": {"type": "object"},
        "risk_level": "low"
    }))
    .unwrap()
}

fn request(tools: Vec<ToolSchema>) -> PlannerRequest {
    PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "read the document",
        tools,
        Budget::new(10, 60_000, 1_000, 0.5).unwrap(),
    )
    .unwrap()
}

fn read_step(id: &str, sequence: u32, tool_name: &str, depends_on: &[&str]) -> Value {
    json!({
        "id": id,
        "sequence": sequence,
        "tool": tool_name,
        "args": {},
        "depends_on": depends_on,
        "postconditions": [],
        "effect": "read",
        "reversibility": "l0_undo_stack",
        "point_of_no_return": false,
        "timeouts": {
            "resolve_ms": 100,
            "execute_ms": 200,
            "verify_ms": 100
        }
    })
}

fn write_step(
    id: &str,
    sequence: u32,
    tool_name: &str,
    depends_on: &[&str],
    reversibility: &str,
    point_of_no_return: bool,
) -> Value {
    json!({
        "id": id,
        "sequence": sequence,
        "tool": tool_name,
        "args": {"text": "hello"},
        "depends_on": depends_on,
        "postconditions": [{"kind": "element_exists"}],
        "effect": "write",
        "reversibility": reversibility,
        "point_of_no_return": point_of_no_return,
        "timeouts": {
            "resolve_ms": 100,
            "execute_ms": 200,
            "verify_ms": 100
        }
    })
}

fn write_step_without_postconditions(id: &str, sequence: u32, tool_name: &str) -> Value {
    json!({
        "id": id,
        "sequence": sequence,
        "tool": tool_name,
        "args": {"text": "hello"},
        "depends_on": [],
        "postconditions": [],
        "effect": "write",
        "reversibility": "l1_snapshot",
        "point_of_no_return": false,
        "timeouts": {
            "resolve_ms": 100,
            "execute_ms": 200,
            "verify_ms": 100
        }
    })
}

fn planner_for(output: &str) -> Planner {
    Planner::new(Arc::new(FakeProvider::new(vec![
        CompletionEvent::TextDelta(output.to_string()),
        CompletionEvent::Finished(FinishReason::Stop),
    ])))
}

fn plan_output(steps: &Value) -> String {
    json!({"steps": steps}).to_string()
}

#[test]
fn test_planner_valid_output_produces_task_engine_plan() {
    let tools = vec![tool("notepad.text.read"), tool("notepad.text.write")];
    let output = plan_output(&json!([
        read_step("s_1", 1, "notepad.text.read", &[]),
        write_step(
            "s_2",
            2,
            "notepad.text.write",
            &["s_1"],
            "l1_snapshot",
            false,
        )
    ]));
    let planner = planner_for(&output);
    let plan = planner
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap();

    assert_eq!(plan.plan_id.as_str(), "p_1");
    assert_eq!(plan.task_id.as_str(), "t_1");
    assert_eq!(plan.goal, "read the document");
    assert_eq!(plan.steps.len(), 2);
    assert!(plan.validate().is_ok());

    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    let snapshot = engine.create_task(plan, 1_000).unwrap();
    assert_eq!(snapshot.plan.steps.len(), 2);
}

#[test]
fn test_planner_rejects_duplicate_step_id() {
    let tools = vec![tool("notepad.text.read")];
    let output = plan_output(&json!([
        read_step("s_1", 1, "notepad.text.read", &[]),
        read_step("s_1", 2, "notepad.text.read", &[])
    ]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlan { .. }));
    assert_eq!(error.reason_code(), "invalid_plan");
}

#[test]
fn test_planner_rejects_missing_dependency() {
    let tools = vec![tool("notepad.text.read")];
    let output = plan_output(&json!([read_step(
        "s_1",
        1,
        "notepad.text.read",
        &["s_missing"]
    )]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    let CoreError::InvalidPlan { reason } = error else {
        panic!("expected invalid plan");
    };
    assert!(reason.contains("missing"));
}

#[test]
fn test_planner_rejects_dependency_cycle() {
    let tools = vec![tool("notepad.text.read")];
    let output = plan_output(&json!([
        read_step("s_1", 1, "notepad.text.read", &["s_2"]),
        read_step("s_2", 2, "notepad.text.read", &["s_1"])
    ]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    let CoreError::InvalidPlan { reason } = error else {
        panic!("expected invalid plan");
    };
    assert!(reason.contains("cycle"));
}

#[test]
fn test_planner_rejects_invalid_tool_name_syntax() {
    let tools = vec![tool("notepad.text.read")];
    let output = plan_output(&json!([read_step("s_1", 1, "notepad.read", &[])]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlan { .. }));
}

#[test]
fn test_planner_rejects_tool_outside_catalog() {
    let tools = vec![tool("notepad.text.read")];
    let output = plan_output(&json!([read_step("s_1", 1, "paint.canvas.draw", &[])]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, CoreError::UnknownPlannerTool { .. }));
    assert_eq!(
        error.error_code(),
        assistant_protocol::ErrorCode::ToolInvalidArgs
    );
}

#[test]
fn test_planner_rejects_write_step_without_postconditions() {
    let tools = vec![tool("notepad.text.write")];
    let output = plan_output(&json!([write_step_without_postconditions(
        "s_1",
        1,
        "notepad.text.write",
    )]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlan { .. }));
}

#[test]
fn test_planner_rejects_l3_without_point_of_no_return() {
    let tools = vec![tool("notepad.text.write")];
    let output = plan_output(&json!([write_step(
        "s_1",
        1,
        "notepad.text.write",
        &[],
        "l3_irreversible",
        false,
    )]));
    let error = planner_for(&output)
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    let CoreError::InvalidPlan { reason } = error else {
        panic!("expected invalid plan");
    };
    assert!(reason.contains("point_of_no_return"));
}

#[test]
fn test_planner_rejects_invalid_json_output() {
    let tools = vec![tool("notepad.text.read")];
    let error = planner_for("not json")
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
    assert_eq!(
        error.error_code(),
        assistant_protocol::ErrorCode::ModelInvalidOutput
    );
}

#[test]
fn test_planner_rejects_missing_or_wrongly_typed_steps() {
    let tools = vec![tool("notepad.text.read")];
    for output in ["{}", "{\"steps\":\"wrong\"}", "{\"steps\":[]}"] {
        let error = planner_for(output)
            .generate_plan(&request(tools.clone()), CancellationToken::new())
            .unwrap_err();
        assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
    }
}

#[test]
fn test_planner_rejects_tool_call_output() {
    let provider = FakeProvider::new(vec![
        CompletionEvent::ToolCallDelta(ToolCallDelta::new(
            "call_1",
            Some("notepad.text.read".to_string()),
            "{}".to_string(),
        )),
        CompletionEvent::Finished(FinishReason::ToolCalls),
    ]);
    let error = Planner::new(Arc::new(provider))
        .generate_plan(
            &request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_request_rejects_bad_or_duplicate_tool_catalog() {
    let duplicate = vec![tool("notepad.text.read"), tool("notepad.text.read")];
    assert!(
        PlannerRequest::new(
            PlanId::new("p_1").unwrap(),
            TaskId::new("t_1").unwrap(),
            "goal",
            duplicate,
            Budget::new(10, 60_000, 1_000, 0.5).unwrap(),
        )
        .is_err()
    );

    assert!(
        PlannerRequest::new(
            PlanId::new("p_1").unwrap(),
            TaskId::new("t_1").unwrap(),
            "",
            vec![tool("notepad.text.read")],
            Budget::new(10, 60_000, 1_000, 0.5).unwrap(),
        )
        .is_err()
    );
}

#[test]
fn test_planner_rejects_stream_without_finish() {
    let provider = FakeProvider::new(vec![CompletionEvent::TextDelta("{}".to_string())]);
    let error = Planner::new(Arc::new(provider))
        .generate_plan(
            &request(vec![tool("notepad.text.read")]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::InvalidPlannerOutput { .. }));
}

#[test]
fn test_planner_builds_json_only_provider_request() {
    let tools = vec![tool("notepad.text.read")];
    let provider = Arc::new(FakeProvider::new(vec![
        CompletionEvent::TextDelta(plan_output(&json!([read_step(
            "s_1",
            1,
            "notepad.text.read",
            &[]
        )]))),
        CompletionEvent::Finished(FinishReason::Stop),
    ]));
    let planner = Planner::new(provider.clone());
    planner
        .generate_plan(&request(tools), CancellationToken::new())
        .unwrap();

    let captured = provider.last_request();
    assert_eq!(captured.tool_choice, ToolChoice::None);
    assert_eq!(captured.response_format, ResponseFormat::JsonObject);
    assert!(captured.tools.is_empty());
}
