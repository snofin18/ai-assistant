//! Budget accounting fail-closed contract: constructor validation, counter and
//! token-overflow handling, and every `BudgetCheck` dimension.
//!
//! Why an integration test: these are all **public contracts** — `Budget::new`,
//! `BudgetUsage::{total_tokens, record_step_started, record_usage}` and
//! `Budget::check`. The crate promises no silent saturation (invariant 6), so each
//! overflow / invalid-input branch needs a regression proof.
//!
//! Only public API; no production-logic change. Test lint exceptions are opened by
//! the single line below (AGENTS.md §5.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_task_engine::{
    Budget, BudgetCheck, BudgetLimit, BudgetUsage, TaskEngineError, UsageDelta,
};

/// `Budget::new` rejects every non-positive bound and bad cost value.
#[test]
fn test_budget_new_rejects_invalid_bounds() {
    assert!(matches!(
        Budget::new(0, 1, 1, 0.0),
        Err(TaskEngineError::InvalidPlan { .. })
    ));
    assert!(matches!(
        Budget::new(1, 0, 1, 0.0),
        Err(TaskEngineError::InvalidPlan { .. })
    ));
    for bad_cost in [-0.1, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(
            matches!(
                Budget::new(1, 1, 1, bad_cost),
                Err(TaskEngineError::InvalidPlan { .. })
            ),
            "cost {bad_cost} must be rejected"
        );
    }
    assert!(Budget::new(1, 1, 0, 0.0).is_ok());
    assert!(Budget::new(1, 1, 1, 0.0).is_ok());
}

/// Summing input and output tokens must fail loudly instead of wrapping.
#[test]
fn test_total_tokens_overflow_is_explicit() {
    let usage = BudgetUsage {
        tokens_in: u64::MAX,
        tokens_out: 1,
        ..Default::default()
    };
    assert!(matches!(
        usage.total_tokens(),
        Err(TaskEngineError::NumericOverflow { .. })
    ));
}

/// A token sum that overflows is reported as the token dimension, and a fresh
/// usage sits within every bound.
#[test]
fn test_budget_check_reports_token_overflow_and_within() {
    let budget = Budget::new(1, 60_000, u64::MAX, 1.0).expect("budget");
    let overflowing = BudgetUsage {
        tokens_in: u64::MAX,
        tokens_out: 1,
        ..Default::default()
    };
    assert_eq!(
        budget.check(&overflowing, 0),
        BudgetCheck::Exceeded(BudgetLimit::Tokens)
    );
    assert_eq!(
        budget.check(&BudgetUsage::default(), 0),
        BudgetCheck::Within
    );
}

/// A started-step counter at the ceiling must not wrap.
#[test]
fn test_record_step_started_overflow_is_explicit() {
    let mut usage = BudgetUsage {
        steps_started: u64::MAX,
        ..Default::default()
    };
    assert!(matches!(
        usage.record_step_started(),
        Err(TaskEngineError::NumericOverflow { .. })
    ));
}

/// `record_usage` rejects invalid cost and every counter overflow, and the happy
/// path accumulates without saturation.
#[test]
fn test_record_usage_rejects_invalid_cost_and_overflows() {
    let mut usage = BudgetUsage::default();
    assert!(matches!(
        usage.record_usage(0, 0, -1.0),
        Err(TaskEngineError::InvalidPlan { .. })
    ));
    assert!(matches!(
        usage.record_usage(0, 0, f64::NAN),
        Err(TaskEngineError::InvalidPlan { .. })
    ));

    let mut in_overflow = BudgetUsage {
        tokens_in: u64::MAX,
        ..Default::default()
    };
    assert!(matches!(
        in_overflow.record_usage(1, 0, 0.0),
        Err(TaskEngineError::NumericOverflow { .. })
    ));

    let mut out_overflow = BudgetUsage {
        tokens_out: u64::MAX,
        ..Default::default()
    };
    assert!(matches!(
        out_overflow.record_usage(0, 1, 0.0),
        Err(TaskEngineError::NumericOverflow { .. })
    ));

    let mut cost_overflow = BudgetUsage {
        cost_usd: f64::MAX,
        ..Default::default()
    };
    assert!(matches!(
        cost_overflow.record_usage(0, 0, f64::MAX),
        Err(TaskEngineError::NumericOverflow { .. })
    ));

    let mut ok = BudgetUsage::default();
    ok.record_usage(2, 3, 0.5).expect("valid usage");
    assert_eq!(ok.total_tokens().expect("sum"), 5);
    assert!((ok.cost_usd - 0.5).abs() < f64::EPSILON);
}

/// `UsageDelta` is a plain public value object.
#[test]
fn test_usage_delta_is_constructible() {
    let delta = UsageDelta {
        tokens_in: 1,
        tokens_out: 2,
        cost_usd: 0.25,
    };
    assert_eq!((delta.tokens_in, delta.tokens_out), (1, 2));
    assert!((delta.cost_usd - 0.25).abs() < f64::EPSILON);
}
