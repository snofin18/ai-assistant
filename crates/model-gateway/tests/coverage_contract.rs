//! Additional public-contract coverage for request validation, routing, retries, and execution.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::redundant_clone,
    clippy::unwrap_used
)]

mod common;

use std::collections::VecDeque;
use std::sync::Arc;

use assistant_model_gateway::{
    AttemptOutcome, Budget, CacheHints, CacheKey, CancellationToken, CompletionEvent,
    CompletionRequest, CompletionStream, CostLedger, CostMicroUsd, DurationMs, FinishReason,
    Message, MessageRole, ModelGateway, ModelGatewayError, ModelId, ModelProvider, ModelResult,
    ModelRouter, MonotonicClock, NoJitter, Pricing, ProviderCapabilities, ProviderFeatures,
    ResponseFormat, RetryPolicy, RouteCondition, RouteContext, RoutingRule, RuleId, Sensitivity,
    Sleeper, SystemMonotonicClock, TaskStage, ThreadSleeper, TokenCount, ToolCallDelta, ToolChoice,
    ToolSpec, Usage, UsageRecord,
};
use assistant_protocol::ErrorCode;

use common::{
    ManualClock, ProviderAction, RecordingSleeper, ScriptedProvider, capabilities, context,
    model_id, pricing, request, successful_events,
};

fn assert_invalid(request: &CompletionRequest, field: &str) {
    let error = request.validate().unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    assert!(error.to_string().contains(field), "{error}");
}

fn assert_cache_hint_contract() {
    let mut invalid = request();
    invalid.cache_hints = CacheHints {
        cache_key: None,
        stable_prefix_end_message_index: None,
        minimum_cacheable_tokens: TokenCount::new(1),
    };
    assert_invalid(&invalid, "cache_hints.minimum_cacheable_tokens");

    let cache_key = CacheKey::new("stable-prefix").unwrap();
    let mut invalid = request();
    invalid.cache_hints = CacheHints {
        cache_key: Some(cache_key.clone()),
        stable_prefix_end_message_index: None,
        minimum_cacheable_tokens: TokenCount::new(0),
    };
    assert_invalid(&invalid, "cache_hints");

    let mut invalid = request();
    invalid.cache_hints = CacheHints {
        cache_key: None,
        stable_prefix_end_message_index: Some(0),
        minimum_cacheable_tokens: TokenCount::new(0),
    };
    assert_invalid(&invalid, "cache_hints");

    let mut valid = request_with_tools();
    valid.tool_choice = ToolChoice::Specific {
        name: "known".to_string(),
    };
    valid.response_format = ResponseFormat::JsonSchema {
        name: "answer".to_string(),
        schema_json: "{}".to_string(),
    };
    valid.temperature = Some(1.0);
    valid.max_output_tokens = Some(TokenCount::new(32));
    valid.cache_hints = CacheHints::stable_prefix(cache_key, 0, TokenCount::new(1));
    valid.validate().unwrap();
}

fn router_for(model: ModelId) -> ModelRouter {
    ModelRouter::new(model.clone(), Vec::new(), vec![model]).unwrap()
}

fn request_with_tools() -> CompletionRequest {
    CompletionRequest::new(
        vec![Message::new(MessageRole::User, "use the known tool")],
        vec![ToolSpec::new("known", "Known tool")],
        ToolChoice::Auto,
        CacheHints::disabled(),
    )
}

#[test]
fn test_request_validation_matrix_rejects_bad_fields() {
    let mut invalid = request();
    invalid.messages.clear();
    assert_invalid(&invalid, "messages");

    let mut invalid = request();
    invalid.messages[0].content.clear();
    assert_invalid(&invalid, "messages.content");

    let mut invalid = request();
    invalid.messages[0].content = "bad\0content".to_string();
    assert_invalid(&invalid, "messages.content");

    let mut invalid = request_with_tools();
    invalid.tools[0].name.clear();
    assert_invalid(&invalid, "tools.name");

    let mut invalid = request_with_tools();
    invalid.tools[0].name = "bad\nname".to_string();
    assert_invalid(&invalid, "tools.name");

    let mut invalid = request_with_tools();
    invalid.tools[0].description.clear();
    assert_invalid(&invalid, "tools.description");

    for temperature in [f64::NAN, f64::INFINITY, -0.1, 2.1] {
        let mut invalid = request();
        invalid.temperature = Some(temperature);
        assert_invalid(&invalid, "temperature");
    }

    let mut invalid = request();
    invalid.max_output_tokens = Some(TokenCount::new(0));
    assert_invalid(&invalid, "max_output_tokens");

    for name in ["", "bad\nname"] {
        let mut invalid = request();
        invalid.response_format = ResponseFormat::JsonSchema {
            name: name.to_string(),
            schema_json: "{}".to_string(),
        };
        assert_invalid(&invalid, "response_format.name");
    }

    let mut invalid = request();
    invalid.response_format = ResponseFormat::JsonSchema {
        name: "answer".to_string(),
        schema_json: String::new(),
    };
    assert_invalid(&invalid, "response_format.schema_json");

    assert_cache_hint_contract();
}

#[test]
fn test_identifier_and_integer_boundaries() {
    for invalid in ["", "bad\0value", "bad\nvalue"] {
        assert!(ModelId::new(invalid).is_err());
        assert!(RuleId::new(invalid).is_err());
        assert!(CacheKey::new(invalid).is_err());
    }

    assert_eq!(model_id("model").to_string(), "model");
    assert_eq!(RuleId::new("rule").unwrap().to_string(), "rule");
    assert_eq!(CacheKey::new("cache").unwrap().to_string(), "cache");
    assert_eq!(DurationMs::new(0).get(), 0);
    assert!(DurationMs::new(0).is_zero());
    assert!(!DurationMs::new(1).is_zero());

    assert!(
        TokenCount::new(u64::MAX)
            .checked_add(TokenCount::new(1))
            .is_err()
    );
    assert!(
        CostMicroUsd::new(u64::MAX)
            .checked_add(CostMicroUsd::new(1))
            .is_err()
    );
}

#[test]
fn test_completion_event_validation_matrix() {
    let model = model_id("model");
    assert!(
        CompletionEvent::TextDelta(String::new())
            .validate(&model)
            .is_err()
    );
    assert!(
        CompletionEvent::TextDelta("bad\0text".to_string())
            .validate(&model)
            .is_err()
    );
    assert!(
        CompletionEvent::ToolCallDelta(ToolCallDelta::new(
            "",
            Some("name".to_string()),
            String::new(),
        ))
        .validate(&model)
        .is_err()
    );
    assert!(
        CompletionEvent::ToolCallDelta(ToolCallDelta::new(
            "bad\0call",
            Some("name".to_string()),
            String::new(),
        ))
        .validate(&model)
        .is_err()
    );
    assert!(
        CompletionEvent::ToolCallDelta(ToolCallDelta::new(
            "call",
            Some(String::new()),
            String::new(),
        ))
        .validate(&model)
        .is_err()
    );
    assert!(
        CompletionEvent::Usage(Usage::new(
            TokenCount::new(1),
            TokenCount::new(2),
            TokenCount::new(0),
        ))
        .validate(&model)
        .is_err()
    );

    for event in [
        CompletionEvent::TextDelta("text".to_string()),
        CompletionEvent::ToolCallDelta(ToolCallDelta::new("call", None, "{\"value\":1}")),
        CompletionEvent::Usage(Usage::new(
            TokenCount::new(2),
            TokenCount::new(1),
            TokenCount::new(1),
        )),
        CompletionEvent::Finished(FinishReason::Stop),
    ] {
        event.validate(&model).unwrap();
    }
}

#[test]
fn test_retry_policy_validation_and_jitter_edges() {
    assert!(
        RetryPolicy::new(17, DurationMs::new(1), DurationMs::new(1))
            .validate()
            .is_err()
    );
    assert!(
        RetryPolicy::new(1, DurationMs::new(0), DurationMs::new(1))
            .validate()
            .is_err()
    );
    assert!(
        RetryPolicy::new(1, DurationMs::new(2), DurationMs::new(1))
            .validate()
            .is_err()
    );

    let policy = RetryPolicy::new(4, DurationMs::new(100), DurationMs::new(250));
    assert_eq!(
        policy.delay_for_retry(0, -500).unwrap(),
        DurationMs::new(50)
    );
    assert_eq!(
        policy.delay_for_retry(4, 1_000).unwrap(),
        DurationMs::new(250)
    );
    assert!(policy.delay_for_retry(0, -1_001).is_err());
    assert!(policy.delay_for_retry(0, 1_001).is_err());

    let overflow = RetryPolicy::new(2, DurationMs::new(u64::MAX), DurationMs::new(u64::MAX));
    assert!(overflow.delay_for_retry(1, 0).is_err());
    ThreadSleeper.sleep(DurationMs::new(0));
    let _ = SystemMonotonicClock::default().now_ms();
}

fn route_primary(condition: RouteCondition, route_context: RouteContext) -> ModelId {
    let default = model_id("default");
    let selected = model_id("selected");
    ModelRouter::new(
        default.clone(),
        vec![RoutingRule::new(
            RuleId::new("selected").unwrap(),
            1,
            vec![condition],
            selected.clone(),
        )],
        vec![default],
    )
    .unwrap()
    .route(&route_context)
    .primary
}

fn assert_router_validation_edges() {
    let default = model_id("default");
    let selected = model_id("selected");
    let fallback = vec![default.clone()];
    assert!(ModelRouter::new(default.clone(), Vec::new(), Vec::new()).is_err());
    assert!(
        ModelRouter::new(
            default.clone(),
            vec![RoutingRule::new(
                RuleId::new("empty").unwrap(),
                0,
                Vec::new(),
                selected.clone(),
            )],
            fallback.clone(),
        )
        .is_err()
    );
    assert!(
        ModelRouter::new(
            default.clone(),
            vec![
                RoutingRule::new(
                    RuleId::new("duplicate").unwrap(),
                    0,
                    vec![RouteCondition::Always],
                    selected.clone(),
                ),
                RoutingRule::new(
                    RuleId::new("duplicate").unwrap(),
                    1,
                    vec![RouteCondition::Always],
                    selected.clone(),
                ),
            ],
            fallback.clone(),
        )
        .is_err()
    );
    assert!(
        ModelRouter::new(
            default.clone(),
            Vec::new(),
            vec![default.clone(), default.clone()]
        )
        .is_err()
    );
    let referenced = ModelRouter::new(
        default.clone(),
        vec![RoutingRule::new(
            RuleId::new("selected").unwrap(),
            0,
            vec![RouteCondition::Always],
            selected.clone(),
        )],
        fallback,
    )
    .unwrap()
    .referenced_models();
    assert_eq!(referenced, vec![default, selected]);
}

#[test]
fn test_router_conditions_and_validation_edges() {
    let selected = model_id("selected");
    let base_context = RouteContext::new(
        TaskStage::Plan,
        TokenCount::new(100),
        Sensitivity::Sensitive,
        CostMicroUsd::new(50),
        4,
    );
    let cases = [
        (RouteCondition::Always, base_context),
        (RouteCondition::Stage(TaskStage::Plan), base_context),
        (
            RouteCondition::ContextTokensGreaterThan(TokenCount::new(99)),
            base_context,
        ),
        (
            RouteCondition::SensitivityAtLeast(Sensitivity::Sensitive),
            base_context,
        ),
        (
            RouteCondition::RemainingBudgetBelow(CostMicroUsd::new(51)),
            base_context,
        ),
        (RouteCondition::ToolCountLessThanOrEqual(4), base_context),
        (RouteCondition::ToolCountGreaterThan(3), base_context),
    ];

    for (condition, route_context) in cases {
        assert_eq!(route_primary(condition, route_context), selected);
    }
    assert_router_validation_edges();
}

#[test]
fn test_gateway_construction_token_count_and_poll_timeout() {
    let model = model_id("model");
    let provider = Arc::new(ScriptedProvider::new(
        model.clone(),
        vec![ProviderAction::Events(successful_events())],
    ));
    let providers: Vec<Arc<dyn ModelProvider>> = vec![provider.clone()];
    let gateway =
        ModelGateway::new(router_for(model.clone()), RetryPolicy::default(), providers).unwrap();
    assert_eq!(
        gateway
            .count_tokens(&model, &[Message::new(MessageRole::User, "abc")])
            .unwrap(),
        TokenCount::new(3)
    );
    assert!(
        gateway
            .count_tokens(
                &model_id("missing"),
                &[Message::new(MessageRole::User, "abc")]
            )
            .is_err()
    );
    gateway
        .clone()
        .with_event_poll_timeout(DurationMs::new(1))
        .unwrap();
    gateway
        .with_event_poll_timeout(DurationMs::new(500))
        .unwrap();

    let duplicate = Arc::new(ScriptedProvider::new(
        model.clone(),
        vec![ProviderAction::Events(successful_events())],
    ));
    let duplicates: Vec<Arc<dyn ModelProvider>> = vec![provider, duplicate];
    assert!(
        ModelGateway::new(
            router_for(model.clone()),
            RetryPolicy::default(),
            duplicates,
        )
        .is_err()
    );

    let missing: Vec<Arc<dyn ModelProvider>> = Vec::new();
    assert!(
        ModelGateway::new(router_for(model.clone()), RetryPolicy::default(), missing,).is_err()
    );
    assert!(
        ModelGateway::new(
            router_for(model),
            RetryPolicy::new(17, DurationMs::new(1), DurationMs::new(1)),
            Vec::new(),
        )
        .is_err()
    );
}

struct CapabilityOnlyProvider {
    model_id: ModelId,
    capabilities: ProviderCapabilities,
}

impl ModelProvider for CapabilityOnlyProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn pricing(&self) -> Pricing {
        pricing()
    }

    fn count_tokens(&self, _messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(0))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        _cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        Err(ModelGatewayError::InternalInvariant {
            reason: "capability preflight should block before complete",
        })
    }
}

#[test]
fn test_gateway_capability_preflight_falls_back() {
    let model = model_id("capability");
    let fallback_model = model_id("capability-fallback");
    let no_stream = Arc::new(CapabilityOnlyProvider {
        model_id: model.clone(),
        capabilities: ProviderCapabilities::new(ProviderFeatures::TOOLS, TokenCount::new(8_192)),
    });
    let fallback = Arc::new(ScriptedProvider::new(
        fallback_model.clone(),
        vec![ProviderAction::Events(successful_events())],
    ));
    let no_stream_gateway = ModelGateway::new(
        ModelRouter::new(model.clone(), Vec::new(), vec![fallback_model.clone()]).unwrap(),
        RetryPolicy::new(0, DurationMs::new(1), DurationMs::new(1)),
        vec![no_stream, fallback],
    )
    .unwrap();
    let completion = no_stream_gateway
        .collect(
            request(),
            context(TaskStage::Chat),
            CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(completion.usage_record.model_id, fallback_model);
    assert!(matches!(
        completion.attempts[0].outcome,
        AttemptOutcome::FellBack { .. }
    ));

    let no_tools = Arc::new(CapabilityOnlyProvider {
        model_id: model.clone(),
        capabilities: ProviderCapabilities::new(
            ProviderFeatures::STREAMING,
            TokenCount::new(8_192),
        ),
    });
    let fallback = Arc::new(ScriptedProvider::new(
        fallback_model.clone(),
        vec![ProviderAction::Events(successful_events())],
    ));
    let no_tools_gateway = ModelGateway::new(
        ModelRouter::new(model.clone(), Vec::new(), vec![fallback_model.clone()]).unwrap(),
        RetryPolicy::new(0, DurationMs::new(1), DurationMs::new(1)),
        vec![no_tools, fallback],
    )
    .unwrap();
    let completion = no_tools_gateway
        .collect(
            request_with_tools(),
            context(TaskStage::ToolSelect),
            CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(completion.usage_record.model_id, fallback_model);
    assert!(matches!(
        completion.attempts[0].outcome,
        AttemptOutcome::FellBack { .. }
    ));
}

#[test]
fn test_gateway_rejects_invalid_request_and_prestart_cancellation() {
    let model = model_id("prestart");
    let provider = Arc::new(ScriptedProvider::new(
        model.clone(),
        vec![ProviderAction::Events(successful_events())],
    ));
    let gateway = ModelGateway::new(
        router_for(model),
        RetryPolicy::default(),
        vec![provider.clone()],
    )
    .unwrap();
    assert!(
        gateway
            .complete(
                CompletionRequest::new(
                    Vec::new(),
                    Vec::new(),
                    ToolChoice::Auto,
                    CacheHints::disabled()
                ),
                context(TaskStage::Chat),
                CancellationToken::new(),
            )
            .is_err()
    );
    assert_eq!(provider.call_count(), 0);

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let mut completion = gateway
        .complete(request(), context(TaskStage::Chat), cancellation)
        .unwrap();
    assert!(matches!(
        completion.next_event().unwrap_err(),
        ModelGatewayError::Cancelled { .. }
    ));
    assert_eq!(provider.call_count(), 0);
}

fn gateway_with_events(model: ModelId, events: Vec<ModelResult<CompletionEvent>>) -> ModelGateway {
    ModelGateway::new(
        router_for(model.clone()),
        RetryPolicy::new(0, DurationMs::new(1), DurationMs::new(1)),
        vec![Arc::new(ScriptedProvider::new(
            model,
            vec![ProviderAction::Events(events)],
        ))],
    )
    .unwrap()
}

fn collect_single_error(gateway: &ModelGateway) -> ModelGatewayError {
    gateway
        .collect(
            request(),
            context(TaskStage::Chat),
            CancellationToken::new(),
        )
        .unwrap_err()
}

#[test]
fn test_gateway_rejects_invalid_terminal_sequences() {
    let model = model_id("terminal");
    let usage = Ok(CompletionEvent::Usage(Usage::new(
        TokenCount::new(1),
        TokenCount::new(0),
        TokenCount::new(1),
    )));
    let duplicate_usage = gateway_with_events(
        model.clone(),
        vec![
            usage.clone(),
            usage,
            Ok(CompletionEvent::Finished(FinishReason::Stop)),
        ],
    );
    assert!(matches!(
        collect_single_error(&duplicate_usage),
        ModelGatewayError::InvalidOutput { .. }
    ));

    let finish_without_usage = gateway_with_events(
        model.clone(),
        vec![Ok(CompletionEvent::Finished(FinishReason::Stop))],
    );
    assert!(matches!(
        collect_single_error(&finish_without_usage),
        ModelGatewayError::IncompleteResponse { .. }
    ));

    let empty_stream = gateway_with_events(model, Vec::new());
    assert!(matches!(
        collect_single_error(&empty_stream),
        ModelGatewayError::IncompleteResponse { .. }
    ));
}

#[test]
fn test_gateway_rejects_invalid_event_and_partial_output() {
    let model = model_id("invalid-stream");
    let invalid_event = gateway_with_events(
        model.clone(),
        vec![Ok(CompletionEvent::TextDelta("bad\0text".to_string()))],
    );
    assert!(matches!(
        collect_single_error(&invalid_event),
        ModelGatewayError::InvalidOutput { .. }
    ));

    let partial_output = gateway_with_events(
        model,
        vec![Ok(CompletionEvent::TextDelta("partial".to_string()))],
    );
    assert!(matches!(
        collect_single_error(&partial_output),
        ModelGatewayError::PartialOutput { .. }
    ));
}

#[test]
fn test_gateway_provider_start_failure_falls_back() {
    let model = model_id("stream");
    let fallback_model = model_id("stream-fallback");
    let fallback = Arc::new(ScriptedProvider::new(
        fallback_model.clone(),
        vec![ProviderAction::Events(successful_events())],
    ));
    let gateway = ModelGateway::new(
        ModelRouter::new(model.clone(), Vec::new(), vec![fallback_model.clone()]).unwrap(),
        RetryPolicy::new(0, DurationMs::new(1), DurationMs::new(1)),
        vec![
            Arc::new(ScriptedProvider::new(
                model,
                vec![ProviderAction::Fail(ModelGatewayError::ServerError {
                    model_id: model_id("stream"),
                    status: None,
                })],
            )),
            fallback,
        ],
    )
    .unwrap();
    let completion = gateway
        .collect(
            request(),
            context(TaskStage::Chat),
            CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(completion.usage_record.model_id, fallback_model);
    assert!(matches!(
        completion.attempts[0].outcome,
        AttemptOutcome::FellBack { .. }
    ));
}

struct AdvancingProvider {
    model_id: ModelId,
    clock: Arc<ManualClock>,
}

impl ModelProvider for AdvancingProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        capabilities(false, false)
    }

    fn pricing(&self) -> Pricing {
        pricing()
    }

    fn count_tokens(&self, _messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(0))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        _cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        Ok(Box::new(AdvancingStream {
            clock: Arc::clone(&self.clock),
            events: successful_events().into(),
        }))
    }
}

struct AdvancingStream {
    clock: Arc<ManualClock>,
    events: VecDeque<ModelResult<CompletionEvent>>,
}

impl CompletionStream for AdvancingStream {
    fn next_event(
        &mut self,
        _cancellation: &CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        self.clock.advance(10);
        self.events
            .pop_front()
            .map_or(Ok(None), |result| result.map(Some))
    }
}

#[test]
fn test_gateway_duration_budget_and_cost_ledger_edges() {
    let model = model_id("duration");
    let clock = Arc::new(ManualClock::default());
    let gateway = ModelGateway::with_runtime(
        router_for(model.clone()),
        RetryPolicy::new(0, DurationMs::new(1), DurationMs::new(1)),
        vec![Arc::new(AdvancingProvider {
            model_id: model.clone(),
            clock: Arc::clone(&clock),
        })],
        clock,
        Arc::new(RecordingSleeper::default()),
        Arc::new(NoJitter),
    )
    .unwrap();
    let request = request().with_budget(Budget {
        max_cost: None,
        max_duration: Some(DurationMs::new(5)),
    });
    let error = gateway
        .collect(request, context(TaskStage::Chat), CancellationToken::new())
        .unwrap_err();
    assert!(matches!(
        error,
        ModelGatewayError::BudgetExceeded {
            resource: "duration",
            ..
        }
    ));

    let mut ledger = CostLedger::default();
    assert_eq!(ledger.total().unwrap().call_count, 0);
    assert!(ledger.total_for_model(&model).is_none());
    let record = UsageRecord {
        model_id: model.clone(),
        usage: Usage::new(TokenCount::new(1), TokenCount::new(0), TokenCount::new(1)),
        cost: CostMicroUsd::new(u64::MAX),
        latency: DurationMs::new(1),
        cache_hit: false,
    };
    ledger.record(record.clone()).unwrap();
    assert!(ledger.record(record).is_err());

    let invalid_usage = Usage::new(TokenCount::new(1), TokenCount::new(2), TokenCount::new(0));
    assert!(pricing().compute_cost(&invalid_usage).is_err());
    assert!(
        Pricing::new(u64::MAX, u64::MAX, u64::MAX)
            .compute_cost(&Usage::new(
                TokenCount::new(u64::MAX),
                TokenCount::new(0),
                TokenCount::new(u64::MAX),
            ))
            .is_err()
    );
}

#[test]
fn test_provider_feature_accessors_and_error_display_edges() {
    let features = ProviderFeatures::NONE
        .union(ProviderFeatures::STREAMING)
        .union(ProviderFeatures::TOOLS)
        .union(ProviderFeatures::VISION)
        .union(ProviderFeatures::PROMPT_CACHE);
    let capabilities = ProviderCapabilities::new(features, TokenCount::new(16_384));
    assert!(capabilities.supports_streaming());
    assert!(capabilities.supports_tools());
    assert!(capabilities.supports_vision());
    assert!(capabilities.supports_prompt_cache());
    assert_eq!(capabilities.max_context_tokens(), TokenCount::new(16_384));
    assert!(ProviderFeatures::NONE.contains(ProviderFeatures::NONE));

    let model = model_id("display");
    let rate_limited = ModelGatewayError::RateLimited {
        model_id: model.clone(),
        retry_after: None,
    };
    assert!(rate_limited.to_string().contains("rate limited"));
    let server_error = ModelGatewayError::ServerError {
        model_id: model.clone(),
        status: None,
    };
    assert!(server_error.to_string().contains("server error"));
    assert_eq!(
        ModelGatewayError::Cancelled {
            model_id: Some(model.clone()),
            elapsed: DurationMs::new(3),
        }
        .to_string(),
        "model request to display was cancelled after 3 ms"
    );
    assert_eq!(
        ModelGatewayError::Cancelled {
            model_id: None,
            elapsed: DurationMs::new(3),
        }
        .to_string(),
        "model request was cancelled after 3 ms"
    );
    assert_eq!(
        ModelGatewayError::NumericOverflow { field: "tokens" }.error_code(),
        ErrorCode::Fatal
    );
    assert_eq!(
        ModelGatewayError::BudgetExceeded {
            model_id: model,
            resource: "cost",
            limit: 1,
            actual: 2,
        }
        .error_code(),
        ErrorCode::PolicyDenied
    );
}
