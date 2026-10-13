//! Direct contract tests for the pure policy model (`PolicyRule` / `RuleConditions`).
//!
//! These cover branches the DSL round-trip tests cannot reach through JSON alone:
//! identifier shape, empty condition lists, confirmation scope duplicates, deny
//! reason bounds, and every `RuleConditions::matches` constraint. The policy
//! engine is the single allow/deny point, so these malformed shapes must keep
//! failing closed instead of being silently accepted.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use assistant_policy::{
    Effect, EgressDestination, EvaluationContext, PolicyError, PolicyRule, Reversibility,
    RuleConditions, RuleEffect, RuleId, ScopeOption,
};
use assistant_protocol::RiskLevel;

fn base_conditions() -> RuleConditions {
    RuleConditions {
        effect: Some(vec![Effect::Write]),
        ..RuleConditions::default()
    }
}

fn context(
    effect: Effect,
    risk_level: RiskLevel,
    target_app: &str,
    egress: EgressDestination,
) -> EvaluationContext {
    EvaluationContext {
        effect,
        risk_level,
        reversibility: Reversibility::L0UndoStack,
        unattended: false,
        tainted: false,
        target_app: target_app.to_owned(),
        egress,
    }
}

fn assert_invalid(rule: &PolicyRule, fragment: &str) {
    match rule.validate() {
        Err(PolicyError::InvalidRuleSet { reason }) => assert!(
            reason.contains(fragment),
            "reason {reason:?} must mention {fragment:?}"
        ),
        other => panic!("rule {rule:?} must fail validation, got {other:?}"),
    }
}

#[test]
fn test_rule_validation_rejects_rule_without_any_condition() {
    let rule = PolicyRule {
        id: RuleId::new("empty_conditions"),
        conditions: RuleConditions::default(),
        effect: RuleEffect::Allow,
    };
    assert_invalid(&rule, "no conditions");
}

#[test]
fn test_rule_validation_rejects_each_empty_condition_list() {
    let cases = [
        (
            RuleConditions {
                effect: Some(vec![]),
                ..RuleConditions::default()
            },
            "condition effect is an empty list",
        ),
        (
            RuleConditions {
                risk_level: Some(vec![]),
                ..RuleConditions::default()
            },
            "condition risk_level is an empty list",
        ),
        (
            RuleConditions {
                reversibility: Some(vec![]),
                ..RuleConditions::default()
            },
            "condition reversibility is an empty list",
        ),
        (
            RuleConditions {
                target_app: Some(vec![]),
                ..RuleConditions::default()
            },
            "condition target_app is an empty list",
        ),
        (
            RuleConditions {
                egress: Some(vec![]),
                ..RuleConditions::default()
            },
            "condition egress is an empty list",
        ),
    ];
    for (conditions, expected) in cases {
        let rule = PolicyRule {
            id: RuleId::new("empty_list"),
            conditions,
            effect: RuleEffect::Allow,
        };
        assert_invalid(&rule, expected);
    }
}

#[test]
fn test_rule_validation_rejects_malformed_ids_and_target_identifiers() {
    let malformed_ids = ["1leading", "with space", "with/slash", "", "a:b"];
    for id in malformed_ids {
        let rule = PolicyRule {
            id: RuleId::new(id),
            conditions: base_conditions(),
            effect: RuleEffect::Allow,
        };
        assert_invalid(&rule, "rule id is empty or contains invalid characters");
    }

    let long_id = "a".repeat(129);
    let oversized = PolicyRule {
        id: RuleId::new(long_id),
        conditions: base_conditions(),
        effect: RuleEffect::Allow,
    };
    assert_invalid(
        &oversized,
        "rule id is empty or contains invalid characters",
    );

    let bad_target = PolicyRule {
        id: RuleId::new("bad_target"),
        conditions: RuleConditions {
            target_app: Some(vec!["has space".to_owned()]),
            ..RuleConditions::default()
        },
        effect: RuleEffect::Allow,
    };
    assert_invalid(&bad_target, "target_app contains an invalid identifier");
}

#[test]
fn test_rule_validation_accepts_each_well_formed_effect() {
    let allow = PolicyRule {
        id: RuleId::new("allow_rule"),
        conditions: base_conditions(),
        effect: RuleEffect::Allow,
    };
    assert_eq!(allow.validate(), Ok(()));

    let confirm = PolicyRule {
        id: RuleId::new("confirm_rule"),
        conditions: base_conditions(),
        effect: RuleEffect::AllowWithConfirmation {
            scope_options: vec![ScopeOption::Once, ScopeOption::ThisTask],
            show_diff: false,
        },
    };
    assert_eq!(confirm.validate(), Ok(()));

    let deny = PolicyRule {
        id: RuleId::new("deny_rule"),
        conditions: base_conditions(),
        effect: RuleEffect::Deny {
            reason: "policy denies this write".to_owned(),
        },
    };
    assert_eq!(deny.validate(), Ok(()));
}

#[test]
fn test_rule_validation_rejects_empty_or_duplicate_confirmation_scopes() {
    let empty_scopes = PolicyRule {
        id: RuleId::new("empty_scopes"),
        conditions: base_conditions(),
        effect: RuleEffect::AllowWithConfirmation {
            scope_options: vec![],
            show_diff: true,
        },
    };
    assert_invalid(&empty_scopes, "scope_options must not be empty");

    let duplicate_scopes = PolicyRule {
        id: RuleId::new("duplicate_scopes"),
        conditions: base_conditions(),
        effect: RuleEffect::AllowWithConfirmation {
            scope_options: vec![ScopeOption::Once, ScopeOption::Once],
            show_diff: true,
        },
    };
    assert_invalid(&duplicate_scopes, "scope_options contain duplicates");
}

#[test]
fn test_rule_validation_rejects_deny_reason_out_of_bounds() {
    let blank_reason = PolicyRule {
        id: RuleId::new("blank_reason"),
        conditions: base_conditions(),
        effect: RuleEffect::Deny {
            reason: "   ".to_owned(),
        },
    };
    assert_invalid(
        &blank_reason,
        "deny reason must be non-empty and at most 512 bytes",
    );

    let oversized_reason = PolicyRule {
        id: RuleId::new("oversized_reason"),
        conditions: base_conditions(),
        effect: RuleEffect::Deny {
            reason: "x".repeat(513),
        },
    };
    assert_invalid(
        &oversized_reason,
        "deny reason must be non-empty and at most 512 bytes",
    );
}

#[test]
fn test_rule_conditions_matches_unconstrained_and_emptied_fields() {
    let context = context(
        Effect::Write,
        RiskLevel::Medium,
        "notepad",
        EgressDestination::None,
    );
    assert!(RuleConditions::default().matches(&context));

    let empty_effect = RuleConditions {
        effect: Some(vec![]),
        ..RuleConditions::default()
    };
    assert!(!empty_effect.matches(&context));
}

#[test]
fn test_rule_conditions_matches_all_constraints_and_target_app_is_case_insensitive() {
    let context = context(
        Effect::Write,
        RiskLevel::Medium,
        "NotePad",
        EgressDestination::LocalModel,
    );
    let conditions = RuleConditions {
        effect: Some(vec![Effect::Write, Effect::Invoke]),
        risk_level: Some(vec![RiskLevel::Medium]),
        reversibility: Some(vec![Reversibility::L0UndoStack]),
        unattended: Some(false),
        tainted: Some(false),
        target_app: Some(vec!["notepad".to_owned()]),
        egress: Some(vec![EgressDestination::LocalModel]),
    };
    assert!(conditions.matches(&context));
}

#[test]
fn test_rule_conditions_rejects_each_mismatched_constraint() {
    let context = context(
        Effect::Write,
        RiskLevel::Medium,
        "notepad",
        EgressDestination::None,
    );
    let mismatched = [
        RuleConditions {
            effect: Some(vec![Effect::Read]),
            ..RuleConditions::default()
        },
        RuleConditions {
            risk_level: Some(vec![RiskLevel::High]),
            ..RuleConditions::default()
        },
        RuleConditions {
            reversibility: Some(vec![Reversibility::L3Irreversible]),
            ..RuleConditions::default()
        },
        RuleConditions {
            unattended: Some(true),
            ..RuleConditions::default()
        },
        RuleConditions {
            tainted: Some(true),
            ..RuleConditions::default()
        },
        RuleConditions {
            target_app: Some(vec!["mspaint".to_owned()]),
            ..RuleConditions::default()
        },
        RuleConditions {
            egress: Some(vec![EgressDestination::CloudModel]),
            ..RuleConditions::default()
        },
    ];
    for conditions in mismatched {
        assert!(!conditions.matches(&context));
    }
}
