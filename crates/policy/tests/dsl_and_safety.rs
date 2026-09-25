//! DSL parser and non-bypassable safety-floor tests.

use assistant_policy::{
    Decision, Effect, EgressDestination, EvaluationContext, PolicyError, Reversibility, RuleSet,
};
use assistant_protocol::RiskLevel;

const ARCHITECTURE_EXAMPLE: &str = r#"{
  "default": { "effect": "deny" },
  "rules": [
    {
      "id": "allow_read_low_risk",
      "when": { "effect": "read", "risk_level": ["low"] },
      "effect": "allow"
    },
    {
      "id": "confirm_medium_write",
      "when": {
        "effect": "write",
        "risk_level": ["medium"],
        "reversibility": ["L0_undo_stack", "L1_snapshot", "L2_compensating"]
      },
      "effect": "allow_with_confirmation",
      "confirmation": { "scope_options": ["once", "this_task"], "show_diff": true }
    },
    {
      "id": "block_irreversible_unattended",
      "when": { "reversibility": ["L3_irreversible"], "unattended": true },
      "effect": "deny",
      "reason": "irreversible actions are forbidden while unattended"
    },
    {
      "id": "block_after_untrusted_content",
      "when": { "tainted": true, "risk_level": ["high"] },
      "effect": "deny",
      "reason": "high-risk actions are forbidden after untrusted content"
    },
    {
      "id": "deny_sensitive_egress",
      "when": { "target_app": ["hr_system", "finance"], "egress": "cloud_model" },
      "effect": "deny",
      "reason": "sensitive application content must not leave the machine"
    }
  ]
}"#;

fn context(effect: Effect) -> EvaluationContext {
    EvaluationContext {
        effect,
        risk_level: RiskLevel::Low,
        reversibility: Reversibility::L0UndoStack,
        unattended: false,
        tainted: false,
        target_app: "notepad".to_owned(),
        egress: EgressDestination::None,
    }
}

#[test]
fn test_dsl_v0_parses_all_five_architecture_rules() {
    let rule_set = RuleSet::from_json(ARCHITECTURE_EXAMPLE);
    assert!(rule_set.is_ok());
    let rule_set = match rule_set {
        Ok(value) => value,
        Err(error) => {
            assert!(
                error.to_string().is_empty(),
                "architecture DSL example must parse"
            );
            RuleSet::default()
        }
    };
    assert_eq!(rule_set.rules().len(), 5);
    assert_eq!(
        rule_set.evaluate(&context(Effect::Read)),
        Ok(Decision::Allow {
            rule_id: "allow_read_low_risk".to_owned()
        })
    );
}

#[test]
fn test_dsl_rejects_unknown_keys_and_non_deny_default() {
    let unknown_key = RuleSet::from_json(r#"{"default":{"effect":"deny"},"extra":[]}"#);
    assert!(matches!(
        unknown_key,
        Err(PolicyError::InvalidRuleSet { .. })
    ));
    let allow_default = RuleSet::from_json(r#"{"default":{"effect":"allow"},"rules":[]}"#);
    assert!(matches!(
        allow_default,
        Err(PolicyError::InvalidRuleSet { .. })
    ));
}

#[test]
fn test_mandatory_safety_floors_cannot_be_overridden_by_custom_rules() {
    let rule_set = RuleSet::from_json(
        r#"{
          "default": { "effect": "deny" },
          "rules": [{
            "id": "unsafe_allow",
            "when": { "effect": "send" },
            "effect": "allow"
          }]
        }"#,
    );
    let rule_set = match rule_set {
        Ok(value) => value,
        Err(error) => {
            assert!(error.to_string().is_empty(), "custom rule set must parse");
            RuleSet::default()
        }
    };
    let mut unattended = context(Effect::Send);
    unattended.reversibility = Reversibility::L3Irreversible;
    unattended.unattended = true;
    assert!(matches!(
        rule_set.evaluate(&unattended),
        Ok(Decision::Deny { rule_id, .. }) if rule_id == "block_irreversible_unattended"
    ));

    let mut tainted = context(Effect::Send);
    tainted.tainted = true;
    tainted.risk_level = RiskLevel::Critical;
    assert!(matches!(
        rule_set.evaluate(&tainted),
        Ok(Decision::Deny { rule_id, .. }) if rule_id == "block_after_untrusted_content"
    ));
}
