//! Fail-closed policy evaluation for the AI assistant.
//!
//! Responsibilities:
//! - evaluate declarative rules against an `EvaluationContext`;
//! - expose pure parameter validators for untrusted model output;
//! - project policy decisions into `assistant_protocol::PolicyDecision`.
//!
//! Boundaries:
//! - no IO, clock, randomness, filesystem access, or platform calls;
//! - no tool execution or approval workflow;
//! - no persistence or hot reload of rule sets.
//!
//! Invariants:
//! - a context with `Reversibility::L3Irreversible` and `unattended = true`
//!   is always denied, even when a custom rule set tries to allow it;
//! - a tainted context cannot allow an L3, high-risk, or critical-risk action;
//! - no matching rule produces `default_deny`;
//! - every denial carries a non-empty rule id and readable reason.
//!
//! Typical use:
//! ```
//! use assistant_policy::{EvaluationContext, EgressDestination, Effect, Reversibility, RuleSet};
//! use assistant_protocol::RiskLevel;
//!
//! let context = EvaluationContext {
//!     effect: Effect::Read,
//!     risk_level: RiskLevel::Low,
//!     reversibility: Reversibility::L0UndoStack,
//!     unattended: false,
//!     tainted: false,
//!     target_app: "notepad".to_owned(),
//!     egress: EgressDestination::None,
//! };
//! let decision = RuleSet::example_v0().evaluate(&context);
//! assert!(decision.is_ok());
//! ```
//!
//! Related architecture sections: v2 sections 8.7, 9.1, 12.2, 12.3, and 12.4.

#![deny(unsafe_code)]

mod decision;
mod dsl;
mod error;
mod model;
mod rule_set;
mod validation;

pub use decision::Decision;
pub use error::{PolicyError, PolicyResult};
pub use model::{
    Effect, EgressDestination, EvaluationContext, PolicyRule, Reversibility, RuleConditions,
    RuleEffect, RuleId, ScopeOption,
};
pub use rule_set::RuleSet;
pub use validation::{
    PathValidationRequest, TextLimits, UrlValidationRequest, ValidatedPath,
    ValidatedRegularExpression, ValidatedUrl, validate_integer_range, validate_path,
    validate_regular_expression, validate_text_limits, validate_url,
};
