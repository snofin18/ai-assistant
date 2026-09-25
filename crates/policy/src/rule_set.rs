//! Rule-set validation and fail-closed evaluation.

use crate::dsl;
use crate::{
    Decision, EvaluationContext, PolicyError, PolicyResult, PolicyRule, Reversibility,
    RuleConditions, RuleEffect, RuleId, ScopeOption,
};
use assistant_protocol::RiskLevel;
use std::collections::HashSet;

const MAX_RULES: usize = 256;
const DEFAULT_DENY_RULE_ID: &str = "default_deny";
const IRREVERSIBLE_UNATTENDED_RULE_ID: &str = "block_irreversible_unattended";
const TAINTED_UNTRUSTED_RULE_ID: &str = "block_after_untrusted_content";

/// Ordered collection of rules. The absence of a match always means deny.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    rules: Vec<PolicyRule>,
}

impl RuleSet {
    /// Creates a rule set. Call [`RuleSet::validate`] or [`RuleSet::evaluate`]
    /// before relying on it.
    #[must_use]
    pub const fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }

    /// Returns the configured rules in declaration order.
    #[must_use]
    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    /// Parses DSL v0 from JSON and validates every rule.
    ///
    /// JSON is used for TOML-independent DSL v0. Unknown keys and malformed
    /// values are rejected; unsupported syntax is never treated as allow.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError::InvalidRuleSet`] or [`PolicyError::RuleSetConflict`]
    /// when the JSON cannot be parsed or the resulting rules are unsafe.
    pub fn from_json(json: &str) -> PolicyResult<Self> {
        let rule_set = dsl::parse_rule_set(json)?;
        rule_set.validate()?;
        Ok(rule_set)
    }

    /// Validates ids, duplicate ids, rule count, predicates, and effect payloads.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError::InvalidRuleSet`] for malformed rules and
    /// [`PolicyError::RuleSetConflict`] for duplicate rule ids.
    pub fn validate(&self) -> PolicyResult<()> {
        if self.rules.len() > MAX_RULES {
            return Err(PolicyError::InvalidRuleSet {
                reason: format!(
                    "rule count {} exceeds maximum {MAX_RULES}",
                    self.rules.len()
                ),
            });
        }
        let mut identifiers = HashSet::with_capacity(self.rules.len());
        for rule in &self.rules {
            rule.validate()?;
            if !identifiers.insert(rule.id.as_str()) {
                return Err(PolicyError::RuleSetConflict {
                    rule_id: rule.id.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Evaluates all rules against one immutable context.
    ///
    /// Matching deny rules take precedence over confirmation and allow rules.
    /// Mandatory safety floors are evaluated before declarative rules, so no
    /// custom rule set can bypass them.
    ///
    /// # Errors
    ///
    /// Returns the same rule-set validation errors as [`RuleSet::validate`].
    pub fn evaluate(&self, context: &EvaluationContext) -> PolicyResult<Decision> {
        self.validate()?;

        if context.reversibility == Reversibility::L3Irreversible && context.unattended {
            return Ok(deny(
                IRREVERSIBLE_UNATTENDED_RULE_ID,
                "irreversible actions must never run unattended",
            ));
        }
        if context.tainted
            && (context.reversibility == Reversibility::L3Irreversible
                || matches!(context.risk_level, RiskLevel::High | RiskLevel::Critical))
        {
            return Ok(deny(
                TAINTED_UNTRUSTED_RULE_ID,
                "untrusted content taints the context; high-risk actions are blocked",
            ));
        }

        let mut confirmation = None;
        let mut allow = None;
        for rule in &self.rules {
            if !rule.conditions.matches(context) {
                continue;
            }
            match &rule.effect {
                RuleEffect::Deny { reason } => {
                    return Ok(deny(rule.id.as_str(), reason));
                }
                RuleEffect::AllowWithConfirmation {
                    scope_options,
                    show_diff,
                } => {
                    if confirmation.is_none() {
                        confirmation = Some(Decision::AllowWithConfirmation {
                            rule_id: rule.id.as_str().to_owned(),
                            scope_options: scope_options.clone(),
                            show_diff: *show_diff,
                        });
                    }
                }
                RuleEffect::Allow => {
                    if allow.is_none() {
                        allow = Some(Decision::Allow {
                            rule_id: rule.id.as_str().to_owned(),
                        });
                    }
                }
            }
        }

        Ok(confirmation.or(allow).unwrap_or_else(default_deny))
    }

    /// Returns the five example rules from architecture v2 section 12.2.
    #[must_use]
    pub fn example_v0() -> Self {
        Self::new(vec![
            PolicyRule {
                id: RuleId::new("allow_read_low_risk"),
                conditions: RuleConditions {
                    effect: Some(vec![crate::Effect::Read]),
                    risk_level: Some(vec![RiskLevel::Low]),
                    ..RuleConditions::default()
                },
                effect: RuleEffect::Allow,
            },
            PolicyRule {
                id: RuleId::new("confirm_medium_write"),
                conditions: RuleConditions {
                    effect: Some(vec![crate::Effect::Write]),
                    risk_level: Some(vec![RiskLevel::Medium]),
                    reversibility: Some(vec![
                        Reversibility::L0UndoStack,
                        Reversibility::L1Snapshot,
                        Reversibility::L2Compensating,
                    ]),
                    ..RuleConditions::default()
                },
                effect: RuleEffect::AllowWithConfirmation {
                    scope_options: vec![ScopeOption::Once, ScopeOption::ThisTask],
                    show_diff: true,
                },
            },
            PolicyRule {
                id: RuleId::new(IRREVERSIBLE_UNATTENDED_RULE_ID),
                conditions: RuleConditions {
                    reversibility: Some(vec![Reversibility::L3Irreversible]),
                    unattended: Some(true),
                    ..RuleConditions::default()
                },
                effect: RuleEffect::Deny {
                    reason: "irreversible actions are forbidden while unattended".to_owned(),
                },
            },
            PolicyRule {
                id: RuleId::new(TAINTED_UNTRUSTED_RULE_ID),
                conditions: RuleConditions {
                    tainted: Some(true),
                    risk_level: Some(vec![RiskLevel::High]),
                    ..RuleConditions::default()
                },
                effect: RuleEffect::Deny {
                    reason: "high-risk actions are forbidden after untrusted content".to_owned(),
                },
            },
            PolicyRule {
                id: RuleId::new("deny_sensitive_egress"),
                conditions: RuleConditions {
                    target_app: Some(vec!["hr_system".to_owned(), "finance".to_owned()]),
                    egress: Some(vec![crate::EgressDestination::CloudModel]),
                    ..RuleConditions::default()
                },
                effect: RuleEffect::Deny {
                    reason: "sensitive application content must not leave the machine".to_owned(),
                },
            },
        ])
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

fn deny(rule_id: &str, reason: &str) -> Decision {
    Decision::Deny {
        rule_id: rule_id.to_owned(),
        reason: reason.to_owned(),
    }
}

fn default_deny() -> Decision {
    deny(
        DEFAULT_DENY_RULE_ID,
        "no policy rule explicitly allowed this action",
    )
}
