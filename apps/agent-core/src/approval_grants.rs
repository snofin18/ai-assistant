//! Bounded, in-process approval grants for the reserved runtime steps.
//!
//! Responsibilities:
//! - record that a human approved a specific runtime step, with a **bounded**
//!   lifetime and use count;
//! - hand that approval to the runtime step exactly as many times as it was
//!   granted, then stop.
//!
//! Boundaries:
//! - does not decide anything: the decision comes from the human, and the policy
//!   engine stays the single allow/deny point (iron law 3);
//! - does not persist: 1a keeps approvals in memory, so a restart forgets them
//!   (fail-closed, never fail-open).
//!
//! Invariants:
//! 1. a grant always has positive uses and a positive ttl - no unbounded grant;
//! 2. an expired or exhausted grant is removed and never handed out again;
//! 3. `ApprovalScope::Persistent` is refused for runtime steps, so a step-level
//!    approval can never become a standing authorization.

use std::collections::BTreeMap;
use std::sync::Mutex;

use assistant_hitl::ApprovalScope;

/// Why a grant could not be issued.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum GrantError {
    /// The requested use count was zero.
    #[error("an approval grant must cover at least one use")]
    NoUses,
    /// The requested lifetime was zero or negative.
    #[error("an approval grant must expire in the future")]
    NoDeadline,
    /// The caller asked for a standing approval.
    #[error("a persistent approval scope cannot be granted to a runtime step")]
    PersistentScope,
    /// The grant table could not be locked.
    #[error("the approval grant table is unavailable")]
    Unavailable,
}

struct Grant {
    scope: ApprovalScope,
    expires_at_ms: i64,
    remaining_uses: u32,
}

/// One approval to record, kept as a struct so the call site names every field.
pub struct GrantRequest<'a> {
    /// Task the approval belongs to.
    pub task_id: &'a str,
    /// Step the approval covers.
    pub step_id: &'a str,
    /// How widely the human approved.
    pub scope: ApprovalScope,
    /// Current time, used to compute the deadline.
    pub now_ms: i64,
    /// Lifetime of the approval in milliseconds; must be positive.
    pub ttl_ms: i64,
    /// How many uses the approval covers; must be positive.
    pub uses: u32,
}

/// In-memory table of bounded approvals, keyed by task and step.
#[derive(Default)]
pub struct ApprovalGrants {
    grants: Mutex<BTreeMap<(String, String), Grant>>,
}

impl ApprovalGrants {
    /// Creates an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one human approval.
    ///
    /// # Errors
    ///
    /// Returns [`GrantError`] when the request is unbounded (`uses == 0`,
    /// `ttl_ms <= 0`) or asks for [`ApprovalScope::Persistent`].
    pub fn grant(&self, request: &GrantRequest<'_>) -> Result<(), GrantError> {
        if request.uses == 0 {
            return Err(GrantError::NoUses);
        }
        if request.ttl_ms <= 0 {
            return Err(GrantError::NoDeadline);
        }
        if matches!(request.scope, ApprovalScope::Persistent) {
            return Err(GrantError::PersistentScope);
        }
        let mut grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        grants.insert(
            (request.task_id.to_owned(), request.step_id.to_owned()),
            Grant {
                scope: request.scope,
                expires_at_ms: request.now_ms.saturating_add(request.ttl_ms),
                remaining_uses: request.uses,
            },
        );
        drop(grants);
        Ok(())
    }

    /// Consumes one use of the approval for that step, if it is still valid.
    ///
    /// Returns the scope that was granted, or `None` when there is nothing usable
    /// left. An expired or exhausted entry is removed rather than left behind.
    ///
    /// # Errors
    ///
    /// Returns [`GrantError::Unavailable`] when the table cannot be locked.
    pub fn consume(
        &self,
        task_id: &str,
        step_id: &str,
        now_ms: i64,
    ) -> Result<Option<ApprovalScope>, GrantError> {
        let mut grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        let key = (task_id.to_owned(), step_id.to_owned());
        let state = grants
            .get(&key)
            .map(|entry| (entry.scope, entry.expires_at_ms, entry.remaining_uses));
        let outcome = match state {
            Some((scope, expires_at_ms, remaining)) if now_ms < expires_at_ms && remaining > 0 => {
                let left = remaining - 1;
                if left == 0 {
                    grants.remove(&key);
                } else if let Some(entry) = grants.get_mut(&key) {
                    entry.remaining_uses = left;
                }
                Some(scope)
            }
            _ => {
                grants.remove(&key);
                None
            }
        };
        drop(grants);
        Ok(outcome)
    }

    /// Drops any approval recorded for that step.
    ///
    /// # Errors
    ///
    /// Returns [`GrantError::Unavailable`] when the table cannot be locked.
    pub fn revoke(&self, task_id: &str, step_id: &str) -> Result<(), GrantError> {
        let mut grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        grants.remove(&(task_id.to_owned(), step_id.to_owned()));
        drop(grants);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use assistant_hitl::ApprovalScope;

    use super::{ApprovalGrants, GrantError, GrantRequest};

    #[test]
    fn test_a_grant_is_used_exactly_as_many_times_as_granted() {
        let grants = ApprovalGrants::new();
        assert!(
            grants
                .grant(&GrantRequest {
                    task_id: "t1",
                    step_id: "s1",
                    scope: ApprovalScope::Once,
                    now_ms: 1_000,
                    ttl_ms: 60_000,
                    uses: 1,
                })
                .is_ok()
        );

        assert!(
            matches!(
                grants.consume("t1", "s1", 2_000),
                Ok(Some(ApprovalScope::Once))
            ),
            "the first use must be honoured"
        );
        assert!(
            matches!(grants.consume("t1", "s1", 2_001), Ok(None)),
            "an exhausted grant must not be handed out again"
        );
    }

    #[test]
    fn test_an_expired_grant_is_removed_and_refused() {
        let grants = ApprovalGrants::new();
        assert!(
            grants
                .grant(&GrantRequest {
                    task_id: "t1",
                    step_id: "s2",
                    scope: ApprovalScope::Once,
                    now_ms: 0,
                    ttl_ms: 100,
                    uses: 5,
                })
                .is_ok()
        );

        assert!(
            matches!(grants.consume("t1", "s2", 101), Ok(None)),
            "an expired grant must be refused"
        );
        assert!(
            matches!(grants.consume("t1", "s2", 102), Ok(None)),
            "and it must stay refused after being removed"
        );
    }

    #[test]
    fn test_unbounded_or_standing_grants_are_refused() {
        let grants = ApprovalGrants::new();
        assert_eq!(
            grants.grant(&GrantRequest {
                task_id: "t1",
                step_id: "s3",
                scope: ApprovalScope::Once,
                now_ms: 0,
                ttl_ms: 1_000,
                uses: 0,
            }),
            Err(GrantError::NoUses)
        );
        assert_eq!(
            grants.grant(&GrantRequest {
                task_id: "t1",
                step_id: "s3",
                scope: ApprovalScope::Once,
                now_ms: 0,
                ttl_ms: 0,
                uses: 1,
            }),
            Err(GrantError::NoDeadline)
        );
        assert_eq!(
            grants.grant(&GrantRequest {
                task_id: "t1",
                step_id: "s3",
                scope: ApprovalScope::Persistent,
                now_ms: 0,
                ttl_ms: 1_000,
                uses: 1,
            }),
            Err(GrantError::PersistentScope)
        );
    }

    #[test]
    fn test_revoke_removes_the_approval() {
        let grants = ApprovalGrants::new();
        assert!(
            grants
                .grant(&GrantRequest {
                    task_id: "t1",
                    step_id: "s4",
                    scope: ApprovalScope::Once,
                    now_ms: 0,
                    ttl_ms: 1_000,
                    uses: 1,
                })
                .is_ok()
        );
        assert!(grants.revoke("t1", "s4").is_ok());
        assert!(matches!(grants.consume("t1", "s4", 1), Ok(None)));
    }
}
