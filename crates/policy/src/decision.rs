//! Rich policy decisions and their protocol projection.

use crate::{PolicyError, PolicyResult, ScopeOption};
use assistant_protocol::{PolicyDecision, PolicyScopeOption};

/// Policy outcome before protocol projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The action may proceed without confirmation.
    Allow {
        /// Rule that produced the allow decision.
        rule_id: String,
    },
    /// The action may proceed only after explicit human confirmation.
    AllowWithConfirmation {
        /// Rule that produced the conditional allow decision.
        rule_id: String,
        /// Confirmation scopes the UI may offer.
        scope_options: Vec<ScopeOption>,
        /// Whether the confirmation UI must show a diff.
        show_diff: bool,
    },
    /// The action is denied.
    Deny {
        /// Stable rule id, including `default_deny` and mandatory safety rules.
        rule_id: String,
        /// Human-readable denial reason.
        reason: String,
    },
}

impl Decision {
    /// Returns true only for an unconditional allow.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }

    /// Returns true when the approval workflow must run before execution.
    #[must_use]
    pub const fn requires_confirmation(&self) -> bool {
        matches!(self, Self::AllowWithConfirmation { .. })
    }

    /// Projects this decision into the audit protocol decision shape.
    ///
    /// ADR-0048 extends the protocol shape with optional confirmation scope and
    /// `show_diff`, so the projection is lossless for the fields needed by HITL.
    /// An unconditional allow and a confirmation-required allow remain
    /// distinguishable by whether `scope_options` is present.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError::ProtocolProjection`] if the generated protocol type
    /// cannot deserialize the projected values.
    pub fn to_protocol_decision(&self) -> PolicyResult<PolicyDecision> {
        let (allow, rule_id, reason, scope_options, show_diff) = match self {
            Self::Allow { rule_id } => (true, Some(rule_id.clone()), None, None, None),
            Self::AllowWithConfirmation {
                rule_id,
                scope_options,
                show_diff,
            } => (
                true,
                Some(rule_id.clone()),
                Some("human confirmation required before execution".to_owned()),
                Some(
                    scope_options
                        .iter()
                        .map(|scope| match scope {
                            ScopeOption::Once => PolicyScopeOption::Once,
                            ScopeOption::ThisTask => PolicyScopeOption::ThisTask,
                        })
                        .collect::<Vec<_>>(),
                ),
                Some(*show_diff),
            ),
            Self::Deny { rule_id, reason } => (
                false,
                Some(rule_id.clone()),
                Some(reason.clone()),
                None,
                None,
            ),
        };
        let value = assistant_protocol::serde_json::json!({
            "allow": allow,
            "rule_id": rule_id,
            "reason": reason,
            "scope_options": scope_options,
            "show_diff": show_diff,
        });
        assistant_protocol::serde_json::from_value(value).map_err(|error| {
            PolicyError::ProtocolProjection {
                reason: error.to_string(),
            }
        })
    }
}
