//! Pure policy input model and DSL rule shapes.

use assistant_protocol::RiskLevel;

use crate::{PolicyError, PolicyResult};

/// Operation effect declared by a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Reads data without mutating it.
    Read,
    /// Writes or mutates data.
    Write,
    /// Sends data to another system.
    Send,
    /// Invokes an application action.
    Invoke,
    /// Destroys data or a resource.
    Destroy,
}

/// Reversibility model from architecture v2 section 9.1.
///
/// This is the single policy-local definition until TASK-024 converges the
/// model; it is intentionally not added to `protocol/` before that ADR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reversibility {
    /// The target application keeps an undo stack.
    L0UndoStack,
    /// A snapshot can restore the object.
    L1Snapshot,
    /// No undo exists, but a compensating action exists.
    L2Compensating,
    /// No recovery mechanism exists.
    L3Irreversible,
}

/// Destination of data leaving the local trust boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EgressDestination {
    /// No data leaves the machine.
    None,
    /// Data is sent to a local model runtime.
    LocalModel,
    /// Data is sent to a cloud model provider.
    CloudModel,
}

/// All inputs consumed by policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationContext {
    /// Operation effect.
    pub effect: Effect,
    /// Tool-declared risk level.
    pub risk_level: RiskLevel,
    /// Worst-case reversibility of the step.
    pub reversibility: Reversibility,
    /// Whether execution runs without a person present.
    pub unattended: bool,
    /// Whether untrusted content has tainted the active context.
    pub tainted: bool,
    /// Bound target application identifier.
    pub target_app: String,
    /// Destination used by this operation.
    pub egress: EgressDestination,
}

/// Stable identifier of one rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(String);

impl RuleId {
    /// Creates a rule id. Validity is checked by `PolicyRule::validate`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Confirmation scope that a rule may offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeOption {
    /// Confirm this invocation only.
    Once,
    /// Confirm for the remainder of the current task.
    ThisTask,
}

/// Outcome produced by a matching rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    /// Allow without confirmation.
    Allow,
    /// Allow only through the approval workflow.
    AllowWithConfirmation {
        /// Scopes the UI may offer.
        scope_options: Vec<ScopeOption>,
        /// Whether the confirmation UI must show a diff.
        show_diff: bool,
    },
    /// Deny with a stable id and readable reason.
    Deny {
        /// Human-readable denial reason.
        reason: String,
    },
}

/// Declarative predicates evaluated against `EvaluationContext`.
///
/// `None` means "do not constrain this field". Lists are OR-ed; different
/// fields are AND-ed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleConditions {
    /// Allowed operation effects.
    pub effect: Option<Vec<Effect>>,
    /// Allowed risk levels.
    pub risk_level: Option<Vec<RiskLevel>>,
    /// Allowed reversibility levels.
    pub reversibility: Option<Vec<Reversibility>>,
    /// Required unattended value.
    pub unattended: Option<bool>,
    /// Required taint value.
    pub tainted: Option<bool>,
    /// Allowed target application identifiers.
    pub target_app: Option<Vec<String>>,
    /// Allowed egress destinations.
    pub egress: Option<Vec<EgressDestination>>,
}

impl RuleConditions {
    /// Returns true when all constrained fields match the context.
    #[must_use]
    pub fn matches(&self, context: &EvaluationContext) -> bool {
        list_contains(self.effect.as_ref(), &context.effect)
            && list_contains(self.risk_level.as_ref(), &context.risk_level)
            && list_contains(self.reversibility.as_ref(), &context.reversibility)
            && self
                .unattended
                .is_none_or(|value| value == context.unattended)
            && self.tainted.is_none_or(|value| value == context.tainted)
            && self.target_app.as_ref().is_none_or(|applications| {
                applications
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(&context.target_app))
            })
            && list_contains(self.egress.as_ref(), &context.egress)
    }

    const fn is_empty(&self) -> bool {
        self.effect.is_none()
            && self.risk_level.is_none()
            && self.reversibility.is_none()
            && self.unattended.is_none()
            && self.tainted.is_none()
            && self.target_app.is_none()
            && self.egress.is_none()
    }

    fn validate(&self, rule_id: &str) -> PolicyResult<()> {
        if self.is_empty() {
            return invalid_rule(rule_id, "rule has no conditions");
        }
        for (name, empty) in [
            ("effect", is_empty_list(self.effect.as_ref())),
            ("risk_level", is_empty_list(self.risk_level.as_ref())),
            ("reversibility", is_empty_list(self.reversibility.as_ref())),
            ("target_app", is_empty_list(self.target_app.as_ref())),
            ("egress", is_empty_list(self.egress.as_ref())),
        ] {
            if empty {
                return invalid_rule(rule_id, &format!("condition {name} is an empty list"));
            }
        }
        if let Some(applications) = &self.target_app {
            for application in applications {
                if !is_identifier(application) {
                    return invalid_rule(rule_id, "target_app contains an invalid identifier");
                }
            }
        }
        Ok(())
    }
}

/// One declarative policy rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRule {
    /// Stable rule id.
    pub id: RuleId,
    /// Predicates required for this rule to match.
    pub conditions: RuleConditions,
    /// Outcome emitted on a match.
    pub effect: RuleEffect,
}

impl PolicyRule {
    /// Validates identifiers, condition shape, and effect payloads.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError::InvalidRuleSet`] when any rule field is malformed
    /// or empty where a value is required.
    pub fn validate(&self) -> PolicyResult<()> {
        let rule_id = self.id.as_str();
        if !is_identifier(rule_id) {
            return invalid_rule(rule_id, "rule id is empty or contains invalid characters");
        }
        self.conditions.validate(rule_id)?;
        match &self.effect {
            RuleEffect::Allow => Ok(()),
            RuleEffect::AllowWithConfirmation {
                scope_options,
                show_diff: _,
            } => {
                if scope_options.is_empty() {
                    return invalid_rule(rule_id, "confirmation scope_options must not be empty");
                }
                for (index, scope) in scope_options.iter().enumerate() {
                    if scope_options
                        .iter()
                        .skip(index.saturating_add(1))
                        .any(|other| other == scope)
                    {
                        return invalid_rule(
                            rule_id,
                            "confirmation scope_options contain duplicates",
                        );
                    }
                }
                Ok(())
            }
            RuleEffect::Deny { reason } => {
                if reason.trim().is_empty() || reason.len() > 512 {
                    return invalid_rule(
                        rule_id,
                        "deny reason must be non-empty and at most 512 bytes",
                    );
                }
                Ok(())
            }
        }
    }
}

fn list_contains<T: PartialEq>(condition: Option<&Vec<T>>, actual: &T) -> bool {
    condition.is_none_or(|values| values.contains(actual))
}

fn is_empty_list<T>(condition: Option<&Vec<T>>) -> bool {
    condition.is_some_and(Vec::is_empty)
}

fn is_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && value.len() <= 128
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

fn invalid_rule<T>(rule_id: &str, reason: &str) -> PolicyResult<T> {
    Err(PolicyError::InvalidRuleSet {
        reason: format!("rule '{rule_id}': {reason}"),
    })
}
