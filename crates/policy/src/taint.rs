//! Taint-specific permission decay.
//!
//! Responsibility: keep the policy-level rule that a tainted context offers
//! only one-shot confirmation, regardless of the scopes requested by a rule.
//!
//! Boundary: this module does not store session state and does not decide
//! whether a context is tainted; the caller supplies that boolean.
//!
//! Invariant: a tainted confirmation never carries a standing scope.

use crate::ScopeOption;

/// Returns the confirmation scopes allowed while the context is tainted.
///
/// A tainted context may still be confirmed, but only `once` is valid. The
/// caller must not preserve `this_task` or any future broader scope here.
pub(super) fn decay_confirmation_scopes(_requested: &[ScopeOption]) -> Vec<ScopeOption> {
    vec![ScopeOption::Once]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use crate::{
        Effect, EgressDestination, EvaluationContext, Reversibility, RuleSet, ScopeOption,
    };
    use assistant_protocol::RiskLevel;

    #[test]
    fn test_tainted_medium_write_decays_confirmation_to_once() {
        let context = EvaluationContext {
            effect: Effect::Write,
            risk_level: RiskLevel::Medium,
            reversibility: Reversibility::L1Snapshot,
            unattended: false,
            tainted: true,
            target_app: "notepad".to_owned(),
            egress: EgressDestination::None,
        };
        let decision = RuleSet::example_v0()
            .evaluate(&context)
            .expect("example rule set is valid");
        let crate::Decision::AllowWithConfirmation { scope_options, .. } = decision else {
            panic!("tainted medium write must require confirmation");
        };
        assert_eq!(scope_options, vec![ScopeOption::Once]);
    }
}
