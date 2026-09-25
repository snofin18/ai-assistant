//! Determinism and stable median-latency tests.

use assistant_policy::{Effect, EgressDestination, EvaluationContext, Reversibility, RuleSet};
use assistant_protocol::RiskLevel;
use std::io::Write;
use std::time::{Duration, Instant};

fn context() -> EvaluationContext {
    EvaluationContext {
        effect: Effect::Read,
        risk_level: RiskLevel::Low,
        reversibility: Reversibility::L0UndoStack,
        unattended: false,
        tainted: false,
        target_app: "notepad".to_owned(),
        egress: EgressDestination::None,
    }
}

#[test]
fn test_policy_evaluation_is_deterministic_for_identical_input() {
    let rule_set = RuleSet::example_v0();
    let context = context();
    assert_eq!(rule_set.evaluate(&context), rule_set.evaluate(&context));
}

#[test]
fn test_policy_evaluation_median_is_below_ten_times_budget() {
    let rule_set = RuleSet::example_v0();
    let context = context();
    let iterations = 5_000_usize;
    let mut measurements = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let started = Instant::now();
        let result = std::hint::black_box(rule_set.evaluate(std::hint::black_box(&context)));
        let elapsed = started.elapsed();
        assert!(result.is_ok());
        measurements.push(elapsed);
    }
    measurements.sort_unstable();
    let median = measurements
        .get(measurements.len() / 2)
        .copied()
        .unwrap_or(Duration::from_secs(1));
    let output = format!("assistant-policy median evaluation: {median:?}\n");
    assert!(std::io::stdout().write_all(output.as_bytes()).is_ok());
    assert!(
        median < Duration::from_micros(500),
        "median {median:?} exceeded the 10x-confirmed 50 microsecond budget"
    );
}
