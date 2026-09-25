//! Provider-contract and router tests for prompt-cache forwarding and deterministic selection.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::redundant_clone
)]

mod common;

use assistant_model_gateway::{
    CacheHints, CacheKey, CompletionRequest, Message, MessageRole, ModelGateway, ModelId,
    ModelRouter, ProviderFeatures, RetryPolicy, RouteCondition, RouteContext, RoutingRule, RuleId,
    Sensitivity, TaskStage, TokenCount, ToolChoice,
};

use common::{
    ProviderAction, ScriptedProvider, capabilities, context, model_id, request_with_cache,
};

#[test]
fn test_model_id_with_control_character_returns_invalid_request() {
    let error = ModelId::new("bad\nmodel").unwrap_err();
    assert_eq!(
        error.error_code(),
        assistant_protocol::ErrorCode::ToolInvalidArgs
    );
}

#[test]
fn test_completion_request_with_out_of_range_cache_prefix_returns_invalid_request() {
    let request = request_with_cache();
    let mut invalid = request.clone();
    invalid.cache_hints =
        CacheHints::stable_prefix(CacheKey::new("prompt-v2").unwrap(), 2, TokenCount::new(32));
    let error = invalid.validate().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stable_prefix_end_message_index")
    );
}

#[test]
fn test_gateway_forwards_prompt_cache_hints_and_records_usage() {
    let model = model_id("cache_model");
    let provider = std::sync::Arc::new(ScriptedProvider::new(
        model.clone(),
        vec![ProviderAction::Events(common::successful_events())],
    ));
    let router = ModelRouter::new(model.clone(), Vec::new(), vec![model.clone()]).unwrap();
    let gateway =
        ModelGateway::new(router, RetryPolicy::default(), vec![provider.clone()]).unwrap();

    let completion = gateway
        .collect(
            request_with_cache(),
            context(TaskStage::Chat),
            assistant_model_gateway::CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(completion.events.len(), 3);
    assert_eq!(completion.usage_record.usage.input_tokens.get(), 100);
    assert_eq!(completion.usage_record.usage.output_tokens.get(), 50);
    assert_eq!(completion.usage_record.model_id, model);
    let seen_request = provider.requests().pop().unwrap();
    assert!(seen_request.cache_hints.is_enabled());
    assert_eq!(
        seen_request
            .cache_hints
            .cache_key
            .as_ref()
            .map(CacheKey::as_str),
        Some("prompt-v1")
    );
}

#[test]
fn test_router_prioritizes_sensitive_rule_before_budget_rule() {
    let default_model = model_id("default");
    let private_model = model_id("private");
    let small_model = model_id("small");
    let router = ModelRouter::new(
        default_model.clone(),
        vec![
            RoutingRule::new(
                RuleId::new("budget").unwrap(),
                20,
                vec![RouteCondition::RemainingBudgetBelow(
                    assistant_model_gateway::CostMicroUsd::new(100),
                )],
                small_model.clone(),
            ),
            RoutingRule::new(
                RuleId::new("sensitive").unwrap(),
                10,
                vec![RouteCondition::SensitivityAtLeast(Sensitivity::Sensitive)],
                private_model.clone(),
            ),
        ],
        vec![
            small_model.clone(),
            private_model.clone(),
            default_model.clone(),
        ],
    )
    .unwrap();
    let context = RouteContext::new(
        TaskStage::Plan,
        TokenCount::new(32_000),
        Sensitivity::Sensitive,
        assistant_model_gateway::CostMicroUsd::new(10),
        3,
    );
    let plan = router.route(&context);
    assert_eq!(plan.primary, private_model);
    assert_eq!(plan.fallback_chain, vec![small_model, default_model]);
}

#[test]
fn test_provider_capabilities_feature_set_is_intersection_safe() {
    let features = ProviderFeatures::STREAMING.union(ProviderFeatures::TOOLS);
    let capabilities = capabilities(true, false);
    assert!(capabilities.supports_streaming());
    assert!(capabilities.supports_tools());
    assert!(!capabilities.supports_prompt_cache());
    assert!(features.contains(ProviderFeatures::STREAMING));
    assert!(!features.contains(ProviderFeatures::VISION));
}

#[test]
fn test_completion_request_with_specific_unknown_tool_returns_invalid_request() {
    let invalid = CompletionRequest::new(
        vec![Message::new(MessageRole::User, "use tool")],
        vec![assistant_model_gateway::ToolSpec::new(
            "known",
            "Known tool",
        )],
        ToolChoice::Specific {
            name: "unknown".to_string(),
        },
        CacheHints::disabled(),
    );
    assert!(invalid.validate().is_err());
}

#[test]
fn test_router_with_duplicate_fallback_returns_invalid_configuration() {
    let model = model_id("duplicate");
    assert!(ModelRouter::new(model.clone(), Vec::new(), vec![model.clone(), model]).is_err());
}
