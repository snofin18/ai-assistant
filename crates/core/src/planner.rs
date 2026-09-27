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
use assistant_protocol::{RiskLevel, ToolEffect, ToolReversibility, ToolSchema, serde_json};
use assistant_task_engine::{
    Budget, CheckpointPolicy, Plan, PlanId, PlanStep, StepId, StepTimeouts, TaskId,
};

use crate::error::{CoreError, CoreResult};

const DEFAULT_EVENT_TIMEOUT: DurationMs = DurationMs::new(250);
const MAX_EVENT_TIMEOUT_MS: u64 = 500;
const MAX_PLAN_OUTPUT_BYTES: usize = 1_048_576;
const PLANNER_SYSTEM_PROMPT: &str = r#"Create one executable Plan DAG from the caller goal.
Return exactly one JSON object with one field named "steps". Do not use Markdown.
Each step must use the PlanStep JSON fields: id, sequence, tool, args, depends_on,
postconditions, point_of_no_return, and timeouts. Never output effect or
reversibility: those are authoritative tool metadata supplied by the caller.
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
            if tool.version != "1.0" {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.version",
                    reason: format!("tool {:?} must use schema version 1.0", tool.name),
                });
            }
            if !is_tool_name(&tool.name) {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.name",
                    reason: format!("tool {:?} must use <app>.<domain>.<action>", tool.name),
                });
            }
            if !tool.input.is_object() {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.input",
                    reason: format!("tool {:?} input must be a JSON object", tool.name),
                });
            }
            if !tool.output.is_object() {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.output",
                    reason: format!("tool {:?} output must be a JSON object", tool.name),
                });
            }
            if tool.risk_level == RiskLevel::Critical && !tool.requires_approval {
                return Err(CoreError::InvalidContent {
                    field: "planner.tools.requires_approval",
                    reason: format!(
                        "critical tool {:?} must declare requires_approval=true",
                        tool.name
                    ),
                });
            }
            for tag in &tool.tags {
                if !is_tag_name(tag) {
                    return Err(CoreError::InvalidContent {
                        field: "planner.tools.tags",
                        reason: format!("tool {:?} contains invalid tag {tag:?}", tool.name),
                    });
                }
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
    let steps = parse_steps(&steps_value, request)?;

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

    Ok(plan)
}

fn parse_steps(
    steps_value: &serde_json::Value,
    request: &PlannerRequest,
) -> CoreResult<Vec<PlanStep>> {
    let mut steps_value =
        steps_value
            .as_array()
            .cloned()
            .ok_or_else(|| CoreError::InvalidPlannerOutput {
                reason: "planner steps must be a JSON array".to_string(),
            })?;
    if steps_value.is_empty() {
        return Err(CoreError::InvalidPlannerOutput {
            reason: "planner output must contain at least one step".to_string(),
        });
    }
    let catalog = build_tool_catalog(request)?;
    for (index, step_value) in steps_value.iter_mut().enumerate() {
        inject_trusted_step_metadata(index, step_value, &catalog)?;
    }
    let steps: Vec<PlanStep> = serde_json::from_value(serde_json::Value::Array(steps_value))
        .map_err(|error| CoreError::InvalidPlannerOutput {
            reason: format!("steps do not match the PlanStep contract: {error}"),
        })?;
    validate_deserialized_steps(&steps)?;
    Ok(steps)
}

fn inject_trusted_step_metadata(
    index: usize,
    step_value: &mut serde_json::Value,
    catalog: &std::collections::BTreeMap<&str, &ToolSchema>,
) -> CoreResult<()> {
    let object = step_value
        .as_object_mut()
        .ok_or_else(|| CoreError::InvalidPlannerOutput {
            reason: format!("planner step {index} must be a JSON object"),
        })?;
    if object.contains_key("effect") || object.contains_key("reversibility") {
        return Err(CoreError::InvalidPlannerOutput {
            reason: format!(
                "planner step {index} must not supply effect or reversibility; they come from tool metadata"
            ),
        });
    }
    let tool_name = object
        .get("tool")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CoreError::InvalidPlannerOutput {
            reason: format!("planner step {index} is missing string tool"),
        })?;
    let tool = catalog
        .get(tool_name)
        .ok_or_else(|| CoreError::UnknownPlannerTool {
            step_id: object
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing>")
                .to_string(),
            tool: tool_name.to_string(),
        })?;
    object.insert(
        "effect".to_string(),
        serde_json::Value::String(tool_effect_token(tool.effect).to_string()),
    );
    object.insert(
        "reversibility".to_string(),
        serde_json::Value::String(tool_reversibility_token(tool.reversibility).to_string()),
    );
    Ok(())
}

fn build_tool_catalog(
    request: &PlannerRequest,
) -> CoreResult<std::collections::BTreeMap<&str, &ToolSchema>> {
    let mut catalog = std::collections::BTreeMap::new();
    for tool in &request.tools {
        if catalog.insert(tool.name.as_str(), tool).is_some() {
            return Err(CoreError::InvalidContent {
                field: "planner.tools.name",
                reason: format!("duplicate tool {:?}", tool.name),
            });
        }
    }
    Ok(catalog)
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
    segments.len() == 3 && segments.iter().all(|segment| is_tool_name_segment(segment))
}

fn is_tool_name_segment(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_tag_name(value: &str) -> bool {
    is_tool_name_segment(value)
}

fn validate_deserialized_steps(steps: &[PlanStep]) -> CoreResult<()> {
    for step in steps {
        StepId::new(step.id.as_str()).map_err(|_| CoreError::InvalidPlannerOutput {
            reason: format!("planner step id {:?} is invalid", step.id.as_str()),
        })?;
        for dependency in &step.depends_on {
            StepId::new(dependency.as_str()).map_err(|_| CoreError::InvalidPlannerOutput {
                reason: format!(
                    "planner dependency id {:?} on step {:?} is invalid",
                    dependency.as_str(),
                    step.id.as_str()
                ),
            })?;
        }
        StepTimeouts::new(
            step.timeouts.resolve_ms,
            step.timeouts.execute_ms,
            step.timeouts.verify_ms,
        )
        .map_err(|_| CoreError::InvalidPlannerOutput {
            reason: format!(
                "planner step {:?} must use non-zero resolve/execute/verify timeouts",
                step.id.as_str()
            ),
        })?;
    }
    Ok(())
}

const fn tool_effect_token(effect: ToolEffect) -> &'static str {
    match effect {
        ToolEffect::Read => "read",
        ToolEffect::Write => "write",
        _ => "unknown",
    }
}

const fn tool_reversibility_token(reversibility: ToolReversibility) -> &'static str {
    match reversibility {
        ToolReversibility::L0UndoStack => "l0_undo_stack",
        ToolReversibility::L1Snapshot => "l1_snapshot",
        ToolReversibility::L2Compensation => "l2_compensation",
        ToolReversibility::L3Irreversible => "l3_irreversible",
        _ => "unknown",
    }
}
