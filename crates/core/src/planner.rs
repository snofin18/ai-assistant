//! Deterministic model-output planning into the task-engine Plan contract.
//!
//! Responsibilities:
//! - build one JSON-only planning request from a goal and tool catalog;
//! - consume a `ModelProvider` stream without routing, retry, or tool execution;
//! - parse model output fail-closed into `assistant-task-engine::PlanStep`;
//! - reject malformed plans and tools absent from the supplied catalog.
//!
//! Boundaries:
//! - does not define a second Plan / Step representation;
//! - does not select a model, retry a provider, or fall back to another route;
//! - does not grant permission for a tool or execute any step;
//! - does not persist the resulting Plan.
//!
//! Invariants:
//! - malformed model output never becomes an empty or partially repaired Plan;
//! - every produced Plan passes `Plan::validate`;
//! - every step tool appears in the caller-supplied catalog.
//!
//! Related documents: architecture v2 sections 5 and 8.3,
//! `docs/spec/core-orchestration.md`, and ADR-0053.

use std::collections::BTreeSet;
use std::sync::Arc;

use assistant_model_gateway::{
    CacheHints, CancellationToken, CompletionEvent, CompletionRequest, DurationMs, FinishReason,
    Message, MessageRole, ModelGatewayError, ModelProvider, ResponseFormat, ToolChoice,
};
use assistant_protocol::{ToolSchema, serde_json};
use assistant_task_engine::{Budget, CheckpointPolicy, Plan, PlanId, PlanStep, TaskId};

use crate::error::{CoreError, CoreResult};

const DEFAULT_EVENT_TIMEOUT: DurationMs = DurationMs::new(250);
const MAX_EVENT_TIMEOUT_MS: u64 = 500;
const MAX_PLAN_OUTPUT_BYTES: usize = 1_048_576;
const PLANNER_SYSTEM_PROMPT: &str = r#"Create one executable Plan DAG from the caller goal.
Return exactly one JSON object with one field named "steps". Do not use Markdown.
Each step must use the PlanStep JSON fields: id, sequence, tool, args, depends_on,
postconditions, effect, reversibility, point_of_no_return, and timeouts.
Use only tools from the supplied catalog. Write steps require postconditions.
L3 irreversible steps must set point_of_no_return to true.
"#;

/// Validated inputs for one planning request.
///
/// The model supplies only the `steps` array. Plan identifiers, the goal, the
/// budget, and the checkpoint policy come from this trusted caller-owned
/// request so provider output cannot replace task identity.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannerRequest {
    /// Caller-selected plan identifier.
    pub plan_id: PlanId,
    /// Caller-selected task identifier.
    pub task_id: TaskId,
    /// User-visible goal.
    pub goal: String,
    /// Complete tool catalog available to the model.
    pub tools: Vec<ToolSchema>,
    /// Task resource budget.
    pub budget: Budget,
    /// Persistence policy copied into the produced Plan.
    pub checkpoint_policy: CheckpointPolicy,
}

impl PlannerRequest {
    /// Creates a request with the only currently defined checkpoint policy.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] when the goal is empty or the tool
    /// catalog is empty, malformed, or contains duplicate names.
    pub fn new(
        plan_id: PlanId,
        task_id: TaskId,
        goal: impl Into<String>,
        tools: Vec<ToolSchema>,
        budget: Budget,
    ) -> CoreResult<Self> {
        let request = Self {
            plan_id,
            task_id,
            goal: goal.into(),
            tools,
            budget,
            checkpoint_policy: CheckpointPolicy::AfterEachTransition,
        };
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> CoreResult<()> {
        validate_text("planner.goal", &self.goal)?;
        if self.tools.is_empty() {
            return Err(CoreError::InvalidContent {
                field: "planner.tools",
                reason: "must contain at least one tool schema".to_string(),
            });
        }

        let mut names = BTreeSet::new();
        for tool in &self.tools {
            validate_text("planner.tools.name", &tool.name)?;
            validate_text("planner.tools.version", &tool.version)?;
            validate_text("planner.tools.description", &tool.description)?;
            if !is_tool_name(&tool.name) {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.name",
                    reason: format!("tool {:?} must use <app>.<domain>.<action>", tool.name),
                });
            }
            if !names.insert(tool.name.as_str()) {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.name",
                    reason: format!("duplicate tool {:?}", tool.name),
                });
            }
        }
        Ok(())
    }
}

/// Model provider wrapper that produces validated task-engine Plans.
pub struct Planner {
    provider: Arc<dyn ModelProvider>,
    event_timeout: DurationMs,
}

impl Planner {
    /// Creates a Planner over one injected model provider.
    ///
    /// Construction performs no model call and has no side effects.
    #[must_use]
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            provider,
            event_timeout: DEFAULT_EVENT_TIMEOUT,
        }
    }

    /// Overrides the maximum time allowed for one provider event poll.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] when the timeout is zero or
    /// exceeds 500 ms, the maximum that keeps cancellation bounded to one
    /// second.
    pub fn with_event_timeout(mut self, event_timeout: DurationMs) -> CoreResult<Self> {
        if event_timeout.is_zero() || event_timeout.get() > MAX_EVENT_TIMEOUT_MS {
            return Err(CoreError::InvalidContent {
                field: "planner.event_timeout",
                reason: "must be within 1..=500 ms".to_string(),
            });
        }
        self.event_timeout = event_timeout;
        Ok(self)
    }

    /// Produces a validated Plan from one provider response.
    ///
    /// The method is not idempotent because it calls the provider. Cancellation
    /// is cooperative: the token is passed to the provider and checked before
    /// every event poll. Each poll is bounded by the configured timeout. A
    /// timeout, cancellation, malformed stream, or invalid plan returns a typed
    /// error and never a partial Plan.
    ///
    /// # Errors
    ///
    /// - [`CoreError::InvalidContent`] for invalid request fields or catalog.
    /// - [`CoreError::PlannerModel`] for provider or stream failures.
    /// - [`CoreError::InvalidPlannerOutput`] for malformed JSON or stream shape.
    /// - [`CoreError::InvalidPlan`] when task-engine rejects the DAG.
    /// - [`CoreError::UnknownPlannerTool`] when a step uses a catalog-external tool.
    pub fn generate_plan(
        &self,
        request: &PlannerRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<Plan> {
        request.validate()?;
        let completion = Self::build_completion_request(request)?;
        let output = self.collect_output(completion, cancellation)?;
        parse_plan_output(&output, request)
    }

    fn build_completion_request(request: &PlannerRequest) -> CoreResult<CompletionRequest> {
        let catalog_json = serde_json::to_string_pretty(&request.tools).map_err(|error| {
            CoreError::InvalidPlannerOutput {
                reason: format!("tool catalog could not be serialized: {error}"),
            }
        })?;
        let user_content = format!("Goal:\n{}\n\nTool catalog:\n{catalog_json}", request.goal);
        let completion = CompletionRequest::new(
            vec![
                Message::new(MessageRole::System, PLANNER_SYSTEM_PROMPT),
                Message::new(MessageRole::User, user_content),
            ],
            Vec::new(),
            ToolChoice::None,
            CacheHints::disabled(),
        )
        .with_response_format(ResponseFormat::JsonObject)
        .with_temperature(0.0);
        completion.validate().map_err(CoreError::PlannerModel)?;
        Ok(completion)
    }

    fn collect_output(
        &self,
        request: CompletionRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<String> {
        let model_id = self.provider.model_id().clone();
        if cancellation.is_cancelled() {
            return Err(CoreError::PlannerModel(ModelGatewayError::Cancelled {
                model_id: Some(model_id),
                elapsed: DurationMs::new(0),
            }));
        }

        let stream_cancellation = cancellation.clone();
        let mut stream = self
            .provider
            .complete(request, cancellation)
            .map_err(CoreError::PlannerModel)?;
        let mut output = String::new();

        loop {
            let event = stream
                .next_event(&stream_cancellation, self.event_timeout)
                .map_err(CoreError::PlannerModel)?;
            let Some(event) = event else {
                return Err(CoreError::InvalidPlannerOutput {
                    reason: "provider stream ended before a Stop finish event".to_string(),
                });
            };
            event.validate(&model_id).map_err(CoreError::PlannerModel)?;
            match event {
                CompletionEvent::TextDelta(delta) => {
                    let next_length = output.len().checked_add(delta.len()).ok_or_else(|| {
                        CoreError::InvalidPlannerOutput {
                            reason: "planner output length overflowed".to_string(),
                        }
                    })?;
                    if next_length > MAX_PLAN_OUTPUT_BYTES {
                        return Err(CoreError::InvalidPlannerOutput {
                            reason: format!("planner output exceeds {MAX_PLAN_OUTPUT_BYTES} bytes"),
                        });
                    }
                    output.push_str(&delta);
                }
                CompletionEvent::ToolCallDelta(_) => {
                    return Err(CoreError::InvalidPlannerOutput {
                        reason: "planner request forbids tool-call output".to_string(),
                    });
                }
                CompletionEvent::Usage(_) => {}
                CompletionEvent::Finished(FinishReason::Stop) => return Ok(output),
                CompletionEvent::Finished(_) => {
                    return Err(CoreError::InvalidPlannerOutput {
                        reason: "provider finished without a normal Stop reason".to_string(),
                    });
                }
            }
        }
    }
}

fn parse_plan_output(output: &str, request: &PlannerRequest) -> CoreResult<Plan> {
    if output.trim().is_empty() {
        return Err(CoreError::InvalidPlannerOutput {
            reason: "planner output is empty".to_string(),
        });
    }
    let envelope: serde_json::Value =
        serde_json::from_str(output).map_err(|error| CoreError::InvalidPlannerOutput {
            reason: format!("planner output is not valid JSON: {error}"),
        })?;
    let object = envelope
        .as_object()
        .ok_or_else(|| CoreError::InvalidPlannerOutput {
            reason: "planner output must be a JSON object".to_string(),
        })?;
    if object.len() != 1 || !object.contains_key("steps") {
        return Err(CoreError::InvalidPlannerOutput {
            reason: "planner output must contain only the steps field".to_string(),
        });
    }
    let steps_value =
        object
            .get("steps")
            .cloned()
            .ok_or_else(|| CoreError::InvalidPlannerOutput {
                reason: "planner output is missing steps".to_string(),
            })?;
    let steps: Vec<PlanStep> =
        serde_json::from_value(steps_value).map_err(|error| CoreError::InvalidPlannerOutput {
            reason: format!("steps do not match the PlanStep contract: {error}"),
        })?;
    if steps.is_empty() {
        return Err(CoreError::InvalidPlannerOutput {
            reason: "planner output must contain at least one step".to_string(),
        });
    }

    let plan = Plan {
        plan_id: request.plan_id.clone(),
        task_id: request.task_id.clone(),
        goal: request.goal.clone(),
        steps,
        budget: request.budget.clone(),
        checkpoint_policy: request.checkpoint_policy,
    };
    plan.validate().map_err(|error| CoreError::InvalidPlan {
        reason: error.to_string(),
    })?;

    let catalog: BTreeSet<&str> = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    for step in &plan.steps {
        if !catalog.contains(step.tool.as_str()) {
            return Err(CoreError::UnknownPlannerTool {
                step_id: step.id.to_string(),
                tool: step.tool.clone(),
            });
        }
    }
    Ok(plan)
}

fn validate_text(field: &'static str, value: &str) -> CoreResult<()> {
    if value.trim().is_empty() {
        return Err(CoreError::InvalidContent {
            field,
            reason: "must not be empty".to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(CoreError::InvalidContent {
            field,
            reason: "must not contain control characters".to_string(),
        });
    }
    Ok(())
}

fn is_tool_name(value: &str) -> bool {
    let segments: Vec<&str> = value.split('.').collect();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}
