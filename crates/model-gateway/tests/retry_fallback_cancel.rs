//! Retry, fallback, cancellation, and budget tests using injected runtime dependencies.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::redundant_clone,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use assistant_model_gateway::{
    AttemptOutcome, CancellationToken, CompletionEvent, DurationMs, FinishReason, ModelGateway,
    ModelGatewayError, ModelId, ModelRouter, NoJitter, RetryPolicy, TaskStage, TokenCount, Usage,
};

use common::{
    ManualClock, ProviderAction, RecordingSleeper, ScriptedProvider, SlowCancelProvider, context,
    model_id, request,
};

fn router(primary: ModelId, fallback: ModelId) -> ModelRouter {
    ModelRouter::new(primary.clone(), Vec::new(), vec![primary, fallback]).unwrap()
}

#[test]
fn test_rate_limited_primary_retries_then_succeeds() {
    let primary = model_id("primary");
    let provider = Arc::new(ScriptedProvider::new(
        primary.clone(),
        vec![
            ProviderAction::Fail(ModelGatewayError::RateLimited {
                model_id: primary.clone(),
                retry_after: None,
            }),
            ProviderAction::Fail(ModelGatewayError::ServerError {
                model_id: primary.clone(),
                status: Some(503),
            }),
            ProviderAction::Events(common::successful_events()),
        ],
    ));
    let clock = Arc::new(ManualClock::default());
    let sleeper = Arc::new(RecordingSleeper::default());
    let gateway = ModelGateway::with_runtime(
        ModelRouter::new(primary.clone(), Vec::new(), vec![primary.clone()]).unwrap(),
        RetryPolicy::new(2, DurationMs::new(10), DurationMs::new(100)),
        vec![provider.clone()],
        clock,
        sleeper.clone(),
        Arc::new(NoJitter),
    )
    .unwrap();

    let completion = gateway
        .collect(
            request(),
            context(TaskStage::Chat),
            CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(provider.call_count(), 3);
    assert_eq!(
        sleeper.slept(),
        vec![DurationMs::new(10), DurationMs::new(20)]
    );
    assert!(matches!(
        completion.attempts[0].outcome,
        AttemptOutcome::Retried { .. }
    ));
    assert!(matches!(
        completion.attempts[1].outcome,
        AttemptOutcome::Retried { .. }
    ));
    assert_eq!(completion.attempts[2].outcome, AttemptOutcome::Succeeded);
}

#[test]
fn test_exhausted_primary_falls_back_to_secondary() {
    let primary = model_id("primary");
    let fallback = model_id("fallback");
    let primary_provider = Arc::new(ScriptedProvider::new(
        primary.clone(),
        vec![
            ProviderAction::Fail(ModelGatewayError::Timeout {
                model_id: primary.clone(),
                timeout: DurationMs::new(10),
            }),
            ProviderAction::Fail(ModelGatewayError::Timeout {
                model_id: primary.clone(),
                timeout: DurationMs::new(10),
            }),
        ],
    ));
    let fallback_provider = Arc::new(ScriptedProvider::new(
        fallback.clone(),
        vec![ProviderAction::Events(common::successful_events())],
    ));
    let gateway = ModelGateway::new(
        router(primary.clone(), fallback.clone()),
        RetryPolicy::new(1, DurationMs::new(1), DurationMs::new(2)),
        vec![primary_provider.clone(), fallback_provider.clone()],
    )
    .unwrap();

    let completion = gateway
        .collect(
            request(),
            context(TaskStage::ToolSelect),
            CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(primary_provider.call_count(), 2);
    assert_eq!(fallback_provider.call_count(), 1);
    assert_eq!(completion.usage_record.model_id, fallback);
    assert!(matches!(
        completion.attempts[1].outcome,
        AttemptOutcome::FellBack { .. }
    ));
}

#[test]
fn test_error_after_text_returns_partial_output_and_never_calls_fallback() {
    let primary = model_id("primary");
    let fallback = model_id("fallback");
    let primary_provider = Arc::new(ScriptedProvider::new(
        primary.clone(),
        vec![ProviderAction::Events(vec![
            Ok(CompletionEvent::TextDelta("partial".to_string())),
            Err(ModelGatewayError::ServerError {
                model_id: primary.clone(),
                status: Some(500),
            }),
        ])],
    ));
    let fallback_provider = Arc::new(ScriptedProvider::new(
        fallback.clone(),
        vec![ProviderAction::Events(common::successful_events())],
    ));
    let gateway = ModelGateway::new(
        router(primary.clone(), fallback),
        RetryPolicy::new(2, DurationMs::new(1), DurationMs::new(2)),
        vec![primary_provider, fallback_provider.clone()],
    )
    .unwrap();

    let mut completion = gateway
        .complete(
            request(),
            context(TaskStage::Chat),
            CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(
        completion.next_event().unwrap(),
        Some(CompletionEvent::TextDelta("partial".to_string()))
    );
    let error = completion.next_event().unwrap_err();
    assert!(matches!(error, ModelGatewayError::PartialOutput { .. }));
    assert_eq!(fallback_provider.call_count(), 0);
}

#[test]
fn test_cancellation_stops_slow_stream_without_retry() {
    let model = model_id("slow");
    let provider = Arc::new(SlowCancelProvider::new(model.clone()));
    let gateway = ModelGateway::new(
        ModelRouter::new(model.clone(), Vec::new(), vec![model]).unwrap(),
        RetryPolicy::default(),
        vec![provider.clone()],
    )
    .unwrap();
    let cancellation = CancellationToken::new();
    let cancellation_from_thread = cancellation.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(StdDuration::from_millis(100));
        cancellation_from_thread.cancel();
    });
    let started = Instant::now();
    let mut completion = gateway
        .complete(request(), context(TaskStage::Chat), cancellation)
        .unwrap();
    let error = loop {
        match completion.next_event() {
            Ok(Some(_)) => {}
            Ok(None) => panic!("slow provider ended unexpectedly"),
            Err(error) => break error,
        }
    };
    canceller.join().unwrap();

    assert!(matches!(error, ModelGatewayError::Cancelled { .. }));
    assert!(started.elapsed() < StdDuration::from_secs(1));
    assert_eq!(provider.call_count(), 1);
}

#[test]
fn test_budget_overrun_returns_policy_denied_error() {
    let model = model_id("expensive");
    let provider = Arc::new(ScriptedProvider::new(
        model.clone(),
        vec![ProviderAction::Events(vec![
            Ok(CompletionEvent::Usage(Usage::new(
                TokenCount::new(1_000_000),
                TokenCount::new(0),
                TokenCount::new(0),
            ))),
            Ok(CompletionEvent::Finished(FinishReason::Stop)),
        ])],
    ));
    let gateway = ModelGateway::new(
        ModelRouter::new(model.clone(), Vec::new(), vec![model]).unwrap(),
        RetryPolicy::default(),
        vec![provider],
    )
    .unwrap();
    let request = request().with_budget(assistant_model_gateway::Budget {
        max_cost: Some(assistant_model_gateway::CostMicroUsd::new(1)),
        max_duration: None,
    });
    let error = gateway
        .collect(request, context(TaskStage::Chat), CancellationToken::new())
        .unwrap_err();
    assert!(matches!(error, ModelGatewayError::BudgetExceeded { .. }));
    assert_eq!(
        error.error_code(),
        assistant_protocol::ErrorCode::PolicyDenied
    );
}

#[test]
fn test_retry_policy_clamps_exponential_delay() {
    let policy = RetryPolicy::new(4, DurationMs::new(100), DurationMs::new(250));
    assert_eq!(policy.delay_for_retry(0, 0).unwrap(), DurationMs::new(100));
    assert_eq!(policy.delay_for_retry(1, 0).unwrap(), DurationMs::new(200));
    assert_eq!(policy.delay_for_retry(2, 0).unwrap(), DurationMs::new(250));
}
