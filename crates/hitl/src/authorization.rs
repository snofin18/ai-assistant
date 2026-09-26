//! Four-dimensional authorization scopes with TTL and bounded use.

use assistant_policy::Effect;
use assistant_protocol::RiskLevel;
use assistant_task_engine::{Reversibility, StepId, TaskId};

use crate::error::{HitlError, HitlResult};
use crate::{AppSessionId, SubjectId};

/// Authorization scope from architecture v2 section 10.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApprovalScope {
    /// This invocation only.
    Once,
    /// Same tool and exact target for the rest of this task.
    ThisStepPattern,
    /// Same tool anywhere in this task.
    ThisTask,
    /// Same tool for the current application session.
    ThisAppSession,
    /// Long-lived authorization for the exact target.
    Persistent,
}

impl ApprovalScope {
    /// Stable protocol spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::ThisStepPattern => "this_step_pattern",
            Self::ThisTask => "this_task",
            Self::ThisAppSession => "this_app_session",
            Self::Persistent => "persistent",
        }
    }

    /// Parses a protocol scope string.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidPolicyConfirmation`] for an unsupported
    /// value. Unknown values are rejected rather than guessed.
    pub fn parse(value: &str) -> HitlResult<Self> {
        match value {
            "once" => Ok(Self::Once),
            "this_step_pattern" => Ok(Self::ThisStepPattern),
            "this_task" => Ok(Self::ThisTask),
            "this_app_session" => Ok(Self::ThisAppSession),
            "persistent" => Ok(Self::Persistent),
            _ => Err(HitlError::InvalidPolicyConfirmation),
        }
    }
}

/// Validated tool dimension.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ToolAuthorization(String);

impl ToolAuthorization {
    /// Validates a `<app>.<domain>.<action>` tool name.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidToolName`] when any segment is empty or
    /// contains characters outside lowercase ASCII, digits, or underscore.
    pub fn new(value: impl Into<String>) -> HitlResult<Self> {
        let value = value.into();
        let segments: Vec<&str> = value.split('.').collect();
        let is_valid = segments.len() == 3
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            });
        if !is_valid {
            return Err(HitlError::InvalidToolName { tool: value });
        }
        Ok(Self(value))
    }

    /// Returns the tool name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated application and target dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetAuthorization {
    app_id: String,
    target_id: String,
}

impl TargetAuthorization {
    /// Creates a target authorization.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidValue`] when either component is empty or
    /// exceeds 256 bytes.
    pub fn new(app_id: impl Into<String>, target_id: impl Into<String>) -> HitlResult<Self> {
        let app_id = app_id.into();
        let target_id = target_id.into();
        validate_text("app_id", &app_id)?;
        validate_text("target_id", &target_id)?;
        Ok(Self { app_id, target_id })
    }

    /// Returns the application identifier.
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Returns the target identifier.
    #[must_use]
    pub fn target_id(&self) -> &str {
        &self.target_id
    }
}

/// The four dimensions that must agree for authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDimensions {
    /// Human or principal granting the authorization.
    pub subject: SubjectId,
    /// Tool dimension.
    pub tool: ToolAuthorization,
    /// Application and target dimension.
    pub target: TargetAuthorization,
    /// Operation effect dimension.
    pub effect: Effect,
}

/// Origin of a grant or proposed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationOrigin {
    /// Current task.
    pub task_id: TaskId,
    /// Current step.
    pub step_id: StepId,
    /// Current application session.
    pub app_session_id: AppSessionId,
}

/// Context of one proposed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationContext {
    /// Dimensions of the proposed action.
    pub dimensions: AuthorizationDimensions,
    /// Current task, step, and application session.
    pub origin: AuthorizationOrigin,
}

/// Caller-selected authorization options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationRequest {
    /// Requested scope.
    pub scope: ApprovalScope,
    /// Requested TTL in milliseconds.
    pub ttl_ms: u64,
    /// Maximum number of actions covered by the grant.
    pub max_uses: u32,
}

impl AuthorizationRequest {
    /// Validates basic bounds and high-risk restrictions.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidValue`] for zero TTL / uses, or
    /// [`HitlError::ForbiddenAuthorizationScope`] for a high-risk request that
    /// is not exactly one `once` use.
    pub fn validate_for_risk(&self, risk: &ApprovalRisk) -> HitlResult<()> {
        if self.ttl_ms == 0 {
            return Err(HitlError::InvalidValue {
                reason: "authorization TTL must be greater than zero".to_owned(),
            });
        }
        if self.max_uses == 0 {
            return Err(HitlError::InvalidValue {
                reason: "authorization max_uses must be greater than zero".to_owned(),
            });
        }
        if risk.is_high_risk() && (self.scope != ApprovalScope::Once || self.max_uses != 1) {
            return Err(HitlError::ForbiddenAuthorizationScope {
                scope: self.scope.as_str().to_owned(),
            });
        }
        Ok(())
    }
}

/// Risk inputs relevant to authorization policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalRisk {
    /// Tool-declared risk.
    pub level: RiskLevel,
    /// Worst-case reversibility.
    pub reversibility: Reversibility,
}

impl ApprovalRisk {
    /// Returns true when only `once` authorization is allowed.
    #[must_use]
    pub const fn is_high_risk(self) -> bool {
        matches!(self.level, RiskLevel::High | RiskLevel::Critical)
            || matches!(self.reversibility, Reversibility::L3Irreversible)
    }

    /// Validates every scope advertised by policy.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::ForbiddenAuthorizationScope`] instead of silently
    /// removing a forbidden scope.
    pub fn validate_scope_options(&self, scopes: &[ApprovalScope]) -> HitlResult<()> {
        if scopes.is_empty() {
            return Err(HitlError::InvalidPolicyConfirmation);
        }
        if self.is_high_risk()
            && let Some(scope) = scopes.iter().find(|scope| **scope != ApprovalScope::Once)
        {
            return Err(HitlError::ForbiddenAuthorizationScope {
                scope: scope.as_str().to_owned(),
            });
        }
        Ok(())
    }
}

/// Decision of whether a grant covers an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// The action is covered.
    Allowed,
    /// The action is not covered.
    Denied(AuthorizationDenial),
}

impl AuthorizationDecision {
    /// Returns true only when the action is covered.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Deterministic reason an authorization check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthorizationDenial {
    /// Grant expired at the current timestamp.
    Expired,
    /// No uses remain.
    UsesExhausted,
    /// Subject differs.
    SubjectMismatch,
    /// Tool differs.
    ToolMismatch,
    /// Target identity differs.
    TargetMismatch,
    /// Effect differs.
    EffectMismatch,
    /// Task differs for a task-scoped grant.
    TaskMismatch,
    /// Step differs for a once grant.
    StepMismatch,
    /// Application session differs.
    AppSessionMismatch,
}

/// Approved, bounded authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationGrant {
    /// Covered dimensions.
    dimensions: AuthorizationDimensions,
    /// Task, step, and session in which the grant was issued.
    origin: AuthorizationOrigin,
    /// Scope of the grant.
    scope: ApprovalScope,
    /// Absolute expiry timestamp.
    expires_at_ms: i64,
    /// Remaining covered action count.
    remaining_uses: u32,
}

impl AuthorizationGrant {
    /// Issues a bounded grant after applying risk constraints.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::ForbiddenAuthorizationScope`] for high-risk broad
    /// scopes, [`HitlError::InvalidValue`] for zero TTL / uses or timestamp
    /// overflow.
    pub fn issue(
        dimensions: AuthorizationDimensions,
        origin: AuthorizationOrigin,
        authorization: &AuthorizationRequest,
        risk: &ApprovalRisk,
        issued_at_ms: i64,
    ) -> HitlResult<Self> {
        authorization.validate_for_risk(risk)?;
        let expires_at_ms = issued_at_ms
            .checked_add(i64::try_from(authorization.ttl_ms).map_err(|_| {
                HitlError::InvalidValue {
                    reason: "authorization TTL does not fit in timestamp".to_owned(),
                }
            })?)
            .ok_or_else(|| HitlError::InvalidValue {
                reason: "authorization expiry overflowed".to_owned(),
            })?;
        Ok(Self {
            dimensions,
            origin,
            scope: authorization.scope,
            expires_at_ms,
            remaining_uses: authorization.max_uses,
        })
    }

    /// Returns the covered dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> &AuthorizationDimensions {
        &self.dimensions
    }

    /// Returns the task, step, and session origin.
    #[must_use]
    pub const fn origin(&self) -> &AuthorizationOrigin {
        &self.origin
    }

    /// Returns the grant scope.
    #[must_use]
    pub const fn scope(&self) -> ApprovalScope {
        self.scope
    }

    /// Returns the absolute expiry timestamp.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns the remaining covered action count.
    #[must_use]
    pub const fn remaining_uses(&self) -> u32 {
        self.remaining_uses
    }

    /// Checks whether this grant covers one proposed action.
    #[must_use]
    pub fn covers(&self, context: &AuthorizationContext, now_ms: i64) -> AuthorizationDecision {
        if now_ms >= self.expires_at_ms {
            return AuthorizationDecision::Denied(AuthorizationDenial::Expired);
        }
        if self.remaining_uses == 0 {
            return AuthorizationDecision::Denied(AuthorizationDenial::UsesExhausted);
        }
        if self.dimensions.subject != context.dimensions.subject {
            return AuthorizationDecision::Denied(AuthorizationDenial::SubjectMismatch);
        }
        if self.dimensions.tool != context.dimensions.tool {
            return AuthorizationDecision::Denied(AuthorizationDenial::ToolMismatch);
        }
        if self.dimensions.effect != context.dimensions.effect {
            return AuthorizationDecision::Denied(AuthorizationDenial::EffectMismatch);
        }

        match self.scope {
            ApprovalScope::Once => {
                if self.dimensions.target != context.dimensions.target {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TargetMismatch);
                }
                if self.origin.task_id != context.origin.task_id {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TaskMismatch);
                }
                if self.origin.step_id != context.origin.step_id {
                    return AuthorizationDecision::Denied(AuthorizationDenial::StepMismatch);
                }
                AuthorizationDecision::Allowed
            }
            ApprovalScope::ThisStepPattern => {
                if self.dimensions.target != context.dimensions.target {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TargetMismatch);
                }
                if self.origin.task_id != context.origin.task_id {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TaskMismatch);
                }
                AuthorizationDecision::Allowed
            }
            ApprovalScope::ThisTask => {
                if self.dimensions.target.app_id() != context.dimensions.target.app_id() {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TargetMismatch);
                }
                if self.origin.task_id != context.origin.task_id {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TaskMismatch);
                }
                AuthorizationDecision::Allowed
            }
            ApprovalScope::ThisAppSession => {
                if self.dimensions.target.app_id() != context.dimensions.target.app_id() {
                    return AuthorizationDecision::Denied(AuthorizationDenial::TargetMismatch);
                }
                if self.origin.app_session_id != context.origin.app_session_id {
                    return AuthorizationDecision::Denied(AuthorizationDenial::AppSessionMismatch);
                }
                AuthorizationDecision::Allowed
            }
            ApprovalScope::Persistent => {
                if self.dimensions.target == context.dimensions.target {
                    AuthorizationDecision::Allowed
                } else {
                    AuthorizationDecision::Denied(AuthorizationDenial::TargetMismatch)
                }
            }
        }
    }

    /// Consumes one use if the grant covers the proposed action.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::AuthorizationDenied`] with the same deterministic
    /// reason as [`AuthorizationGrant::covers`].
    pub fn consume_for(mut self, context: &AuthorizationContext, now_ms: i64) -> HitlResult<Self> {
        match self.covers(context, now_ms) {
            AuthorizationDecision::Allowed => {
                self.remaining_uses -= 1;
                Ok(self)
            }
            AuthorizationDecision::Denied(reason) => Err(HitlError::AuthorizationDenied {
                reason: format!("{reason:?}"),
            }),
        }
    }
}

fn validate_text(field: &'static str, value: &str) -> HitlResult<()> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(HitlError::InvalidValue {
            reason: format!("{field} must be non-empty, at most 256 bytes, and control-free"),
        });
    }
    Ok(())
}
