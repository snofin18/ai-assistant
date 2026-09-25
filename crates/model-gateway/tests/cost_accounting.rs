//! Cost calculation and ledger aggregation tests.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::redundant_clone
)]

mod common;

use assistant_model_gateway::{CostLedger, CostMicroUsd, Pricing, TokenCount, Usage, UsageRecord};

use common::{model_id, pricing};

#[test]
fn test_pricing_applies_cached_rate_and_rounds_each_component_up() {
    let pricing = Pricing::new(3_000_000, 300_000, 15_000_000);
    let usage = Usage::new(
        TokenCount::new(1_000_000),
        TokenCount::new(900_000),
        TokenCount::new(500_000),
    );
    assert_eq!(
        pricing.compute_cost(&usage).unwrap(),
        CostMicroUsd::new(8_070_000)
    );
}

#[test]
fn test_cost_ledger_aggregates_records_by_model() {
    let first_model = model_id("first");
    let second_model = model_id("second");
    let usage = Usage::new(
        TokenCount::new(1_000),
        TokenCount::new(250),
        TokenCount::new(500),
    );
    let first_cost = pricing().compute_cost(&usage).unwrap();
    let second_cost = pricing().compute_cost(&usage).unwrap();
    let mut ledger = CostLedger::default();
    ledger
        .record(UsageRecord {
            model_id: first_model.clone(),
            usage,
            cost: first_cost,
            latency: assistant_model_gateway::DurationMs::new(10),
            cache_hit: true,
        })
        .unwrap();
    ledger
        .record(UsageRecord {
            model_id: second_model.clone(),
            usage,
            cost: second_cost,
            latency: assistant_model_gateway::DurationMs::new(20),
            cache_hit: true,
        })
        .unwrap();

    let first_total = ledger.total_for_model(&first_model).unwrap();
    assert_eq!(first_total.call_count, 1);
    assert_eq!(first_total.input_tokens, TokenCount::new(1_000));
    assert_eq!(first_total.output_tokens, TokenCount::new(500));
    assert_eq!(first_total.cost, first_cost);
    let total = ledger.total().unwrap();
    assert_eq!(total.call_count, 2);
    assert_eq!(total.cost, first_cost.checked_add(second_cost).unwrap());
}

#[test]
fn test_usage_cache_hit_is_derived_from_cached_input() {
    let miss = Usage::new(TokenCount::new(10), TokenCount::new(0), TokenCount::new(1));
    let hit = Usage::new(TokenCount::new(10), TokenCount::new(5), TokenCount::new(1));
    assert!(!miss.cache_hit());
    assert!(hit.cache_hit());
    assert_eq!(hit.total_tokens().unwrap(), TokenCount::new(11));
}

#[test]
fn test_usage_with_cached_input_above_total_returns_invalid_request() {
    let invalid = Usage::new(TokenCount::new(1), TokenCount::new(2), TokenCount::new(0));
    assert!(invalid.validate().is_err());
}
