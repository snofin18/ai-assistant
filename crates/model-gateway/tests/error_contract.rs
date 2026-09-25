//! Error mapping and diagnostic contract tests.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::redundant_clone
)]

mod common;

use std::sync::Arc;

use assistant_model_gateway::{
    DurationMs, ModelGateway, ModelGatewayError, ModelId, ModelRouter, RetryPolicy,
};

use common::{ProviderAction, ScriptedProvider, model_id};

fn all_errors() -> Vec<ModelGatewayError> {
    let model = model_id("model");
    vec![
        ModelGatewayError::InvalidConfiguration {
            reason: "bad config".to_string(),
        },
        ModelGatewayError::InvalidRequest {
            field: "field",
            reason: "bad request".to_string(),
        },
        ModelGatewayError::ProviderNotFound {
            model_id: model.clone(),
        },
        ModelGatewayError::CapabilityMissing {
            model_id: model.clone(),
            capability: "tools",
        },
        ModelGatewayError::RateLimited {
            model_id: model.clone(),
            retry_after: Some(DurationMs::new(25)),
        },
        ModelGatewayError::Timeout {
            model_id: model.clone(),
            timeout: DurationMs::new(50),
        },
        ModelGatewayError::ServerError {
            model_id: model.clone(),
            status: Some(503),
        },
        ModelGatewayError::NetworkFailure {
            model_id: model.clone(),
            reason: "reset".to_string(),
        },
        ModelGatewayError::InvalidOutput {
            model_id: model.clone(),
            reason: "bad json".to_string(),
        },
        ModelGatewayError::Cancelled {
            model_id: Some(model.clone()),
            elapsed: DurationMs::new(5),
        },
        ModelGatewayError::Cancelled {
            model_id: None,
            elapsed: DurationMs::new(5),
        },
        ModelGatewayError::IncompleteResponse {
            model_id: model.clone(),
            reason: "missing finish",
        },
        ModelGatewayError::PartialOutput {
            model_id: model.clone(),
            reason: "connection lost".to_string(),
        },
        ModelGatewayError::BudgetExceeded {
            model_id: model.clone(),
            resource: "cost",
            limit: 1,
            actual: 2,
        },
        ModelGatewayError::NumericOverflow { field: "tokens" },
        ModelGatewayError::InternalInvariant {
            reason: "impossible state",
        },
    ]
}

#[test]
fn test_every_error_variant_has_stable_code_and_display_text() {
    for error in all_errors() {
        assert!(!error.to_string().is_empty());
        assert!(matches!(
            error.error_code(),
            assistant_protocol::ErrorCode::ToolInvalidArgs
                | assistant_protocol::ErrorCode::ModelNetworkFailure
                | assistant_protocol::ErrorCode::ModelInvalidOutput
                | assistant_protocol::ErrorCode::UserInteraction
                | assistant_protocol::ErrorCode::PolicyDenied
                | assistant_protocol::ErrorCode::Fatal
        ));
    }
}

#[test]
fn test_retry_fallback_and_cancellation_classification_is_conservative() {
    let model = model_id("model");
    let rate_limited = ModelGatewayError::RateLimited {
        model_id: model.clone(),
        retry_after: None,
    };
    assert!(rate_limited.is_retryable_for_model());
    assert!(rate_limited.should_fallback());
    assert!(!rate_limited.is_cancellation());

    let invalid = ModelGatewayError::InvalidOutput {
        model_id: model,
        reason: "bad output".to_string(),
    };
    assert!(!invalid.is_retryable_for_model());
    assert!(!invalid.should_fallback());
    assert!(!invalid.is_cancellation());

    let cancelled = ModelGatewayError::Cancelled {
        model_id: None,
        elapsed: DurationMs::new(1),
    };
    assert!(!cancelled.is_retryable_for_model());
    assert!(!cancelled.should_fallback());
    assert!(cancelled.is_cancellation());
}

#[test]
fn test_gateway_rejects_invalid_event_poll_timeout() {
    let model: ModelId = model_id("model");
    let provider = Arc::new(ScriptedProvider::new(
        model.clone(),
        vec![ProviderAction::Events(common::successful_events())],
    ));
    let router = ModelRouter::new(model.clone(), Vec::new(), vec![model]).unwrap();
    let gateway = ModelGateway::new(router, RetryPolicy::default(), vec![provider]).unwrap();

    assert!(
        gateway
            .clone()
            .with_event_poll_timeout(DurationMs::new(0))
            .is_err()
    );
    assert!(
        gateway
            .with_event_poll_timeout(DurationMs::new(501))
            .is_err()
    );
}
