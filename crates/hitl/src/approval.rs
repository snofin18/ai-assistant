//! Approval requests and their resolution into bounded grants.

use assistant_protocol::{PolicyDecision, PolicyScopeOption};

use crate::authorization::{
    AuthorizationDimensions, AuthorizationGrant, AuthorizationOrigin, AuthorizationRequest,
};
use crate::error::{HitlError, HitlResult};
use crate::{ApprovalRequestId, ApprovalRisk, ApprovalScope, DiffPreview, SubjectId};

/// Inputs needed to build an approval request before policy confirmation fields
/// are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequestInput {
    /// Stable approval request identifier.
    pub request_id: ApprovalRequestId,
    /// Task, step, and application session context.
    pub origin: AuthorizationOrigin,
    /// Four authorization dimensions of the proposed action.
    pub dimensions: AuthorizationDimensions,
    /// Risk and reversibility inputs.
    pub risk: ApprovalRisk,
    /// Human-readable impact summary.
    pub impact_summary: String,
    /// Prepared diff data, if any.
    pub diff: Option<DiffPreview>,
    /// Time at which the approval request is created.
    pub requested_at_ms: i64,
    /// Maximum authorization TTL the request may offer.
    pub max_ttl_ms: u64,
    /// Maximum number of uses the request may offer.
    pub max_uses: u32,
}

/// One pending approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    /// Stable request identifier.
    request_id: ApprovalRequestId,
    /// Task, step, and session context.
    origin: AuthorizationOrigin,
    /// Authorization dimensions.
    dimensions: AuthorizationDimensions,
    /// Risk and reversibility.
    risk: ApprovalRisk,
    /// Human-readable impact summary.
    impact_summary: String,
    /// Diff data required by the confirmation, when requested.
    diff: Option<DiffPreview>,
    /// Whether the UI must show the prepared diff.
    show_diff: bool,
    /// Scopes policy allows the user to select.
    scope_options: Vec<ApprovalScope>,
    /// Creation time.
    requested_at_ms: i64,
    /// Request expiry.
    expires_at_ms: i64,
    /// Maximum authorization TTL allowed by the request.
    max_ttl_ms: u64,
    /// Maximum authorization uses allowed by the request.
    max_uses: u32,
}

impl ApprovalRequest {
    /// Returns the stable request identifier.
    #[must_use]
    pub const fn request_id(&self) -> &ApprovalRequestId {
        &self.request_id
    }

    /// Returns the task, step, and application session context.
    #[must_use]
    pub const fn origin(&self) -> &AuthorizationOrigin {
        &self.origin
    }

    /// Returns the four authorization dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> &AuthorizationDimensions {
        &self.dimensions
    }

    /// Returns the risk and reversibility inputs.
    #[must_use]
    pub const fn risk(&self) -> ApprovalRisk {
        self.risk
    }

    /// Returns the human-readable impact summary.
    #[must_use]
    pub fn impact_summary(&self) -> &str {
        &self.impact_summary
    }

    /// Returns prepared diff data, if any.
    #[must_use]
    pub const fn diff(&self) -> Option<&DiffPreview> {
        self.diff.as_ref()
    }

    /// Returns whether the UI must show the prepared diff.
    #[must_use]
    pub const fn show_diff(&self) -> bool {
        self.show_diff
    }

    /// Returns the scopes policy allows the user to select.
    #[must_use]
    pub fn scope_options(&self) -> &[ApprovalScope] {
        &self.scope_options
    }

    /// Returns the request creation timestamp.
    #[must_use]
    pub const fn requested_at_ms(&self) -> i64 {
        self.requested_at_ms
    }

    /// Returns the request expiry timestamp.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns the maximum authorization TTL.
    #[must_use]
    pub const fn max_ttl_ms(&self) -> u64 {
        self.max_ttl_ms
    }

    /// Returns the maximum authorization uses.
    #[must_use]
    pub const fn max_uses(&self) -> u32 {
        self.max_uses
    }

    /// Builds an approval request from a lossless ADR-0048 confirmation.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::PolicyDenied`] for a denied policy decision,
    /// [`HitlError::ApprovalNotRequired`] for an unconditional allow,
    /// [`HitlError::InvalidPolicyConfirmation`] for missing / duplicated
    /// confirmation fields, and [`HitlError::ForbiddenAuthorizationScope`] when
    /// high risk is offered a broader scope.
    pub fn from_policy_decision(
        input: ApprovalRequestInput,
        decision: &PolicyDecision,
    ) -> HitlResult<Self> {
        validate_request_input(&input)?;
        if !decision.allow {
            return Err(HitlError::PolicyDenied);
        }
        let protocol_scopes = decision
            .scope_options
            .as_ref()
            .filter(|scopes| !scopes.is_empty())
            .ok_or(HitlError::ApprovalNotRequired)?;
        let show_diff = decision
            .show_diff
            .ok_or(HitlError::InvalidPolicyConfirmation)?;
        let scope_options = protocol_scopes
            .iter()
            .copied()
            .map(map_protocol_scope)
            .collect::<HitlResult<Vec<_>>>()?;
        if has_duplicate_scopes(&scope_options) {
            return Err(HitlError::InvalidPolicyConfirmation);
        }
        input.risk.validate_scope_options(&scope_options)?;
        if input.risk.is_high_risk() && input.max_uses != 1 {
            return Err(HitlError::ForbiddenAuthorizationScope {
                scope: "max_uses".to_owned(),
            });
        }
        if show_diff && input.diff.is_none() {
            return Err(HitlError::InvalidValue {
                reason: "show_diff=true requires prepared diff data".to_owned(),
            });
        }
        let expires_at_ms = input
            .requested_at_ms
            .checked_add(
                i64::try_from(input.max_ttl_ms).map_err(|_| HitlError::InvalidValue {
                    reason: "max_ttl_ms does not fit in timestamp".to_owned(),
                })?,
            )
            .ok_or_else(|| HitlError::InvalidValue {
                reason: "approval expiry overflowed".to_owned(),
            })?;

        Ok(Self {
            request_id: input.request_id,
            origin: input.origin,
            dimensions: input.dimensions,
            risk: input.risk,
            impact_summary: input.impact_summary,
            diff: input.diff,
            show_diff,
            scope_options,
            requested_at_ms: input.requested_at_ms,
            expires_at_ms,
            max_ttl_ms: input.max_ttl_ms,
            max_uses: input.max_uses,
        })
    }

    /// Returns whether the request has expired at `now_ms`.
    #[must_use]
    pub const fn is_expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// Resolves a user decision.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::ClockWentBackwards`] for a time before request
    /// creation, [`HitlError::ScopeNotOffered`] for a scope outside the policy
    /// offer, and validation errors when TTL / uses exceed request bounds.
    /// Expiry is an explicit [`ApprovalOutcome::TimedOut`].
    pub fn resolve(&self, decision: ApprovalDecision, now_ms: i64) -> HitlResult<ApprovalOutcome> {
        if now_ms < self.requested_at_ms {
            return Err(HitlError::ClockWentBackwards {
                previous_ms: self.requested_at_ms,
                now_ms,
            });
        }
        if self.is_expired(now_ms) {
            return Ok(ApprovalOutcome::TimedOut);
        }
        match decision {
            ApprovalDecision::Deny { reason } => {
                if reason.trim().is_empty() {
                    return Err(HitlError::InvalidValue {
                        reason: "approval denial reason must not be empty".to_owned(),
                    });
                }
                Ok(ApprovalOutcome::Denied { reason })
            }
            ApprovalDecision::Approve {
                subject,
                authorization,
            } => {
                self.validate_authorization_request(&authorization)?;
                let mut dimensions = self.dimensions.clone();
                dimensions.subject = subject;
                let grant = AuthorizationGrant::issue(
                    dimensions,
                    self.origin.clone(),
                    &authorization,
                    &self.risk,
                    now_ms,
                )?;
                Ok(ApprovalOutcome::Approved { grant })
            }
        }
    }

    fn validate_authorization_request(&self, request: &AuthorizationRequest) -> HitlResult<()> {
        request.validate_for_risk(&self.risk)?;
        if !self.scope_options.contains(&request.scope) {
            return Err(HitlError::ScopeNotOffered);
        }
        if request.ttl_ms > self.max_ttl_ms {
            return Err(HitlError::InvalidValue {
                reason: "authorization TTL exceeds approval request maximum".to_owned(),
            });
        }
        if request.max_uses > self.max_uses {
            return Err(HitlError::InvalidValue {
                reason: "authorization uses exceed approval request maximum".to_owned(),
            });
        }
        Ok(())
    }
}

/// One user decision for an approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApprovalDecision {
    /// Approve with a selected, bounded authorization.
    Approve {
        /// Human or principal granting the authorization.
        subject: SubjectId,
        /// Selected authorization options.
        authorization: AuthorizationRequest,
    },
    /// Reject the request with a non-empty reason.
    Deny {
        /// Human-readable denial reason.
        reason: String,
    },
}

/// Result of resolving an approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApprovalOutcome {
    /// A bounded grant was created.
    Approved {
        /// Issued authorization grant.
        grant: AuthorizationGrant,
    },
    /// The user rejected the request.
    Denied {
        /// Human-readable denial reason.
        reason: String,
    },
    /// No decision arrived before the request expiry boundary.
    TimedOut,
}

fn validate_request_input(input: &ApprovalRequestInput) -> HitlResult<()> {
    if input.impact_summary.trim().is_empty() || input.impact_summary.len() > 4_096 {
        return Err(HitlError::InvalidValue {
            reason: "impact summary must be non-empty and at most 4096 bytes".to_owned(),
        });
    }
    if input.max_ttl_ms == 0 {
        return Err(HitlError::InvalidValue {
            reason: "approval request max_ttl_ms must be greater than zero".to_owned(),
        });
    }
    if input.max_uses == 0 {
        return Err(HitlError::InvalidValue {
            reason: "approval request max_uses must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

const fn map_protocol_scope(scope: PolicyScopeOption) -> HitlResult<ApprovalScope> {
    match scope {
        PolicyScopeOption::Once => Ok(ApprovalScope::Once),
        PolicyScopeOption::ThisStepPattern => Ok(ApprovalScope::ThisStepPattern),
        PolicyScopeOption::ThisTask => Ok(ApprovalScope::ThisTask),
        PolicyScopeOption::ThisAppSession => Ok(ApprovalScope::ThisAppSession),
        PolicyScopeOption::Persistent => Ok(ApprovalScope::Persistent),
        _ => Err(HitlError::InvalidPolicyConfirmation),
    }
}

fn has_duplicate_scopes(scopes: &[ApprovalScope]) -> bool {
    scopes
        .iter()
        .enumerate()
        .any(|(index, scope)| scopes.iter().skip(index + 1).any(|other| other == scope))
}
