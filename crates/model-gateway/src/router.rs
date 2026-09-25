//! Deterministic typed routing from task context to a primary model and fallback chain.

use crate::error::{ModelGatewayError, ModelResult};
use crate::identity::{CostMicroUsd, ModelId, RuleId, Sensitivity, TaskStage, TokenCount};

/// Inputs available to pure routing rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteContext {
    /// Current task stage.
    pub stage: TaskStage,
    /// Estimated context size before the request.
    pub context_tokens: TokenCount,
    /// Highest sensitivity in the request context.
    pub sensitivity: Sensitivity,
    /// Remaining task budget in micro-USD.
    pub remaining_budget: CostMicroUsd,
    /// Number of mounted tools visible to the model.
    pub tool_count: u32,
}

impl RouteContext {
    /// Creates a routing context.
    #[must_use]
    pub const fn new(
        stage: TaskStage,
        context_tokens: TokenCount,
        sensitivity: Sensitivity,
        remaining_budget: CostMicroUsd,
        tool_count: u32,
    ) -> Self {
        Self {
            stage,
            context_tokens,
            sensitivity,
            remaining_budget,
            tool_count,
        }
    }
}

/// One typed predicate in a routing rule. Multiple predicates are combined with logical AND.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteCondition {
    /// Matches every context.
    Always,
    /// Matches one task stage.
    Stage(TaskStage),
    /// Matches when context tokens are strictly greater than the threshold.
    ContextTokensGreaterThan(TokenCount),
    /// Matches when sensitivity is equal to or greater than the threshold.
    SensitivityAtLeast(Sensitivity),
    /// Matches when remaining budget is strictly below the threshold.
    RemainingBudgetBelow(CostMicroUsd),
    /// Matches when tool count is less than or equal to the threshold.
    ToolCountLessThanOrEqual(u32),
    /// Matches when tool count is strictly greater than the threshold.
    ToolCountGreaterThan(u32),
}

impl RouteCondition {
    fn matches(self, context: &RouteContext) -> bool {
        match self {
            Self::Always => true,
            Self::Stage(stage) => context.stage == stage,
            Self::ContextTokensGreaterThan(threshold) => context.context_tokens > threshold,
            Self::SensitivityAtLeast(threshold) => context.sensitivity >= threshold,
            Self::RemainingBudgetBelow(threshold) => context.remaining_budget < threshold,
            Self::ToolCountLessThanOrEqual(threshold) => context.tool_count <= threshold,
            Self::ToolCountGreaterThan(threshold) => context.tool_count > threshold,
        }
    }
}

/// One first-match routing rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingRule {
    /// Stable rule identifier.
    pub id: RuleId,
    /// Lower values are evaluated first; declaration order breaks ties.
    pub priority: u32,
    /// All conditions must match.
    pub conditions: Vec<RouteCondition>,
    /// Selected model when the rule matches.
    pub model_id: ModelId,
}

impl RoutingRule {
    /// Creates a rule.
    #[must_use]
    pub const fn new(
        id: RuleId,
        priority: u32,
        conditions: Vec<RouteCondition>,
        model_id: ModelId,
    ) -> Self {
        Self {
            id,
            priority,
            conditions,
            model_id,
        }
    }
}

/// Pure routing result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    /// First model to attempt.
    pub primary: ModelId,
    /// Ordered models to try after the primary fails.
    pub fallback_chain: Vec<ModelId>,
}

/// Deterministic router with a default model, prioritized rules, and one fallback chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRouter {
    default_model: ModelId,
    rules: Vec<RoutingRule>,
    fallback_chain: Vec<ModelId>,
}

impl ModelRouter {
    /// Creates and validates a router.
    ///
    /// Rules are sorted by `(priority, declaration_index)`. The fallback chain must be non-empty
    /// and contain no duplicate model identifiers. Returns `InvalidConfiguration` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfiguration` for an empty fallback chain, duplicate rule identifiers, or
    /// duplicate fallback models.
    pub fn new(
        default_model: ModelId,
        mut rules: Vec<RoutingRule>,
        fallback_chain: Vec<ModelId>,
    ) -> ModelResult<Self> {
        if fallback_chain.is_empty() {
            return Err(ModelGatewayError::InvalidConfiguration {
                reason: "fallback chain must not be empty".to_string(),
            });
        }
        let mut seen_rule_ids = std::collections::BTreeSet::new();
        for rule in &rules {
            if rule.conditions.is_empty() {
                return Err(ModelGatewayError::InvalidConfiguration {
                    reason: format!("routing rule {} has no conditions", rule.id),
                });
            }
            if !seen_rule_ids.insert(rule.id.as_str().to_string()) {
                return Err(ModelGatewayError::InvalidConfiguration {
                    reason: format!("duplicate routing rule id {}", rule.id),
                });
            }
        }
        let mut seen_models = std::collections::BTreeSet::new();
        for model_id in &fallback_chain {
            if !seen_models.insert(model_id.as_str().to_string()) {
                return Err(ModelGatewayError::InvalidConfiguration {
                    reason: format!("duplicate fallback model {model_id}"),
                });
            }
        }
        rules.sort_by_key(|rule| rule.priority);
        Ok(Self {
            default_model,
            rules,
            fallback_chain,
        })
    }

    /// Routes a context to one primary and an ordered fallback chain.
    ///
    /// This method is pure, deterministic, and has no side effects. The returned fallback chain
    /// omits the selected primary if it also appears in the global chain.
    #[must_use]
    pub fn route(&self, context: &RouteContext) -> RoutePlan {
        let primary = self
            .rules
            .iter()
            .find(|rule| {
                rule.conditions
                    .iter()
                    .all(|condition| condition.matches(context))
            })
            .map_or_else(|| self.default_model.clone(), |rule| rule.model_id.clone());
        let fallback_chain = self
            .fallback_chain
            .iter()
            .filter(|model_id| **model_id != primary)
            .cloned()
            .collect();
        RoutePlan {
            primary,
            fallback_chain,
        }
    }

    /// Returns every model referenced by the default, a rule, or the fallback chain.
    #[must_use]
    pub fn referenced_models(&self) -> Vec<ModelId> {
        let mut models = Vec::with_capacity(self.rules.len() + self.fallback_chain.len() + 1);
        models.push(self.default_model.clone());
        models.extend(self.rules.iter().map(|rule| rule.model_id.clone()));
        models.extend(self.fallback_chain.iter().cloned());
        models.sort();
        models.dedup();
        models
    }
}
