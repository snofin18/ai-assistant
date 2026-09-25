//! Contract tests for the five architecture example rules and default deny.

use assistant_policy::{
    Decision, Effect, EgressDestination, EvaluationContext, Reversibility, RuleSet, ScopeOption,
};
use assistant_protocol::RiskLevel;

fn context(
    effect: Effect,
    risk_level: RiskLevel,
    reversibility: Reversibility,
) -> EvaluationContext {
    EvaluationContext {
        effect,
        risk_level,
        reversibility,
        unattended: false,
        tainted: false,
        target_app: "notepad".to_owned(),
        egress: EgressDestination::None,
    }
}

#[test]
fn test_allow_read_low_risk_matches_and_non_match_is_denied() {
    let rule_set = RuleSet::example_v0();
    let positive = context(Effect::Read, RiskLevel::Low, Reversibility::L0UndoStack);
    assert_eq!(
        rule_set.evaluate(&positive),
        Ok(Decision::Allow {
            rule_id: "allow_read_low_risk".to_owned()
        })
    );

    let negative = context(Effect::Read, RiskLevel::High, Reversibility::L0UndoStack);
    assert!(matches!(
        rule_set.evaluate(&negative),
        Ok(Decision::Deny { rule_id, .. }) if rule_id == "default_deny"
    ));
}

#[test]
fn test_confirm_medium_write_matches_and_non_match_is_denied() {
    let rule_set = RuleSet::example_v0();
    let positive = context(Effect::Write, RiskLevel::Medium, Reversibility::L1Snapshot);
    assert_eq!(
        rule_set.evaluate(&positive),
        Ok(Decision::AllowWithConfirmation {
            rule_id: "confirm_medium_write".to_owned(),
            scope_options: vec![ScopeOption::Once, ScopeOption::ThisTask],
            show_diff: true,
        })
    );

    let negative = context(
        Effect::Write,
        RiskLevel::Medium,
        Reversibility::L3Irreversible,
    );
    assert!(matches!(
        rule_set.evaluate(&negative),
        Ok(Decision::Deny { rule_id, .. }) if rule_id == "default_deny"
    ));
}

#[test]
fn test_block_irreversible_unattended_matches_and_attended_does_not() {
    let rule_set = RuleSet::example_v0();
    let positive = EvaluationContext {
        unattended: true,
        target_app: "mail".to_owned(),
        egress: EgressDestination::CloudModel,
        ..context(Effect::Send, RiskLevel::High, Reversibility::L3Irreversible)
    };
    assert!(matches!(
        rule_set.evaluate(&positive),
        Ok(Decision::Deny { rule_id, reason })
            if rule_id == "block_irreversible_unattended" && !reason.is_empty()
    ));

    let negative = EvaluationContext {
        target_app: "mail".to_owned(),
        egress: EgressDestination::CloudModel,
        ..context(Effect::Send, RiskLevel::High, Reversibility::L3Irreversible)
    };
    assert!(matches!(
        rule_set.evaluate(&negative),
        Ok(Decision::Deny { rule_id, .. }) if rule_id == "default_deny"
    ));
}

#[test]
fn test_block_after_untrusted_content_matches_and_low_risk_read_does_not() {
    let rule_set = RuleSet::example_v0();
    let positive = EvaluationContext {
        tainted: true,
        ..context(Effect::Write, RiskLevel::High, Reversibility::L1Snapshot)
    };
    assert!(matches!(
        rule_set.evaluate(&positive),
        Ok(Decision::Deny { rule_id, reason })
            if rule_id == "block_after_untrusted_content" && !reason.is_empty()
    ));

    let negative = EvaluationContext {
        tainted: true,
        ..context(Effect::Read, RiskLevel::Low, Reversibility::L0UndoStack)
    };
    assert_eq!(
        rule_set.evaluate(&negative),
        Ok(Decision::Allow {
            rule_id: "allow_read_low_risk".to_owned()
        })
    );
}

#[test]
fn test_deny_sensitive_egress_matches_and_non_sensitive_target_does_not() {
    let rule_set = RuleSet::example_v0();
    let positive = EvaluationContext {
        target_app: "finance".to_owned(),
        egress: EgressDestination::CloudModel,
        ..context(Effect::Send, RiskLevel::Medium, Reversibility::L1Snapshot)
    };
    assert!(matches!(
        rule_set.evaluate(&positive),
        Ok(Decision::Deny { rule_id, reason })
            if rule_id == "deny_sensitive_egress" && !reason.is_empty()
    ));

    let negative = EvaluationContext {
        egress: EgressDestination::CloudModel,
        ..context(Effect::Send, RiskLevel::Medium, Reversibility::L1Snapshot)
    };
    assert!(matches!(
        rule_set.evaluate(&negative),
        Ok(Decision::Deny { rule_id, .. }) if rule_id == "default_deny"
    ));
}

#[test]
fn test_empty_and_unmatched_rule_sets_default_to_deny() {
    let context = context(Effect::Read, RiskLevel::Low, Reversibility::L0UndoStack);
    assert!(matches!(
        RuleSet::default().evaluate(&context),
        Ok(Decision::Deny { rule_id, reason })
            if rule_id == "default_deny" && !reason.is_empty()
    ));
    assert!(matches!(
        RuleSet::example_v0().evaluate(&context),
        Ok(Decision::Allow { .. })
    ));
}
