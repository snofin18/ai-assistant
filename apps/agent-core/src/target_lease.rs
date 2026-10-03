//! Task-scoped exclusive target leases for synthetic input.
//!
//! Responsibilities:
//! - acquire an exclusive `assistant-lease` entry before a synthetic input action;
//! - keep the lease for the complete provider call, not just the acquisition;
//! - release on success, provider failure, and lease failure;
//! - expose stable protocol error categories without inventing an `ErrorCode`.
//!
//! Boundaries:
//! - no platform API calls, target resolution, policy decision, or UI queue;
//! - no cross-process or persistent lease state;
//! - read-only operations do not use this gate.
//!
//! Invariants:
//! 1. a conflicting owner never reaches the guarded operation;
//! 2. every acquired lease is explicitly released before `run` returns;
//! 3. release failure is returned instead of being swallowed.

use std::sync::{Arc, Mutex, MutexGuard};

use assistant_lease::{LeaseError, LeaseKey, LeaseManager, LeaseMode, LeaseOwner};
use assistant_platform_api::LocalHandleId;
use assistant_protocol::ErrorCode;
use assistant_storage::Clock;
use assistant_tool_bus::ToolBusError;

/// TTL for one synthetic-input lease. The lease is held only around one provider call.
const INPUT_LEASE_TTL_MS: u64 = 30_000;

/// Cloneable process-local lease registry shared by task hosts in one process.
#[derive(Clone, Debug)]
pub struct TargetLeaseRegistry {
    leases: Arc<Mutex<LeaseManager>>,
}

impl TargetLeaseRegistry {
    /// Creates an empty process-local registry.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            leases: Arc::new(Mutex::new(LeaseManager::new())),
        }
    }

    /// Creates a gate sharing this registry with every other clone.
    #[must_use]
    pub(crate) fn gate(&self, clock: Arc<dyn Clock>) -> TargetLeaseGate {
        TargetLeaseGate {
            registry: self.clone(),
            clock,
        }
    }

    /// Returns whether two registry handles point at the same manager.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn shares_manager_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.leases, &other.leases)
    }
}

impl Default for TargetLeaseRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal gate around one bounded synthetic-input operation.
pub struct TargetLeaseGate {
    registry: TargetLeaseRegistry,
    clock: Arc<dyn Clock>,
}

impl TargetLeaseGate {
    /// Creates a gate whose time source is injected for deterministic tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            registry: TargetLeaseRegistry::new(),
            clock,
        }
    }

    /// Runs `operation` only while the task owns an exclusive lease for `key`.
    ///
    /// The platform call is intentionally inside `operation`: a conflict therefore
    /// returns before any input is sent. The lease is released after both successful
    /// and failed operations because the release path is outside the operation branch.
    ///
    /// # Errors
    ///
    /// Returns the existing lease error categories (`ToolInvalidArgs` for malformed
    /// owners, `Transient` for a conflict or expiration, `Fatal` for internal clock or
    /// lock failures) and returns the original operation error after releasing the lease.
    pub(crate) fn run<T>(
        &self,
        task_id: &str,
        key: LeaseKey,
        tool: &str,
        operation: impl FnOnce() -> Result<T, ToolBusError>,
    ) -> Result<T, ToolBusError> {
        let owner = LeaseOwner::parse(task_id).map_err(|error| map_lease_error(tool, &error))?;
        let now_ms = self.clock.now_unix_ms();
        let lease = {
            let mut leases = self.lock(tool)?;
            leases
                .acquire(
                    key,
                    LeaseMode::Exclusive,
                    &owner,
                    INPUT_LEASE_TTL_MS,
                    now_ms,
                )
                .map_err(|error| map_lease_error(tool, &error))?
        };

        let operation_result = operation();
        let release_result = self.lock(tool).and_then(|mut leases| {
            leases
                .release(lease.id(), &owner)
                .map(|_released| ())
                .map_err(|error| map_lease_error(tool, &error))
        });

        match operation_result {
            Ok(value) => release_result.map(|()| value),
            Err(error) => match release_result {
                Ok(()) => Err(error),
                Err(release_error) => Err(release_error),
            },
        }
    }

    fn lock(&self, tool: &str) -> Result<MutexGuard<'_, LeaseManager>, ToolBusError> {
        self.registry
            .leases
            .lock()
            .map_err(|_| ToolBusError::EnvelopeAssembly {
                tool: tool.to_owned(),
                reason: "target lease manager lock is poisoned".to_owned(),
            })
    }
}

/// Builds the window-scoped lease key used by key and pointer input.
///
/// # Errors
///
/// Returns `ToolInvalidArgs` when the application or window identifier cannot form a
/// valid lease key.
pub fn window_lease_key(
    app_id: &str,
    window_id: LocalHandleId,
    tool: &str,
) -> Result<LeaseKey, ToolBusError> {
    LeaseKey::new(app_id.to_owned())
        .and_then(|key| key.with_window_id(window_id.value().to_string()))
        .map_err(|error| map_lease_error(tool, &error))
}

fn map_lease_error(tool: &str, error: &LeaseError) -> ToolBusError {
    match error.error_code() {
        ErrorCode::ToolInvalidArgs => ToolBusError::InvalidArguments {
            tool: tool.to_owned(),
            reason: format!("target lease rejected: {error}"),
        },
        ErrorCode::Transient => ToolBusError::Mcp {
            code: -32_000,
            message: format!("{tool}: target lease unavailable: {error}"),
        },
        _ => ToolBusError::EnvelopeAssembly {
            tool: tool.to_owned(),
            reason: format!("target lease failed: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::sync::Arc;

    use assistant_lease::LeaseKey;
    use assistant_protocol::ErrorCode;
    use assistant_storage::Clock;
    use assistant_tool_bus::ToolBusError;

    use super::{TargetLeaseGate, TargetLeaseRegistry};

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_unix_ms(&self) -> i64 {
            1_700_000_000_000
        }
    }

    fn key() -> LeaseKey {
        LeaseKey::new("com.microsoft.notepad")
            .and_then(|key| key.with_window_id("42"))
            .expect("valid fixture key")
    }

    fn gate() -> TargetLeaseGate {
        TargetLeaseGate::new(Arc::new(FixedClock))
    }

    fn count(gate: &TargetLeaseGate) -> usize {
        gate.registry
            .leases
            .lock()
            .expect("lease manager lock")
            .stored_lease_count()
    }

    #[test]
    fn test_two_tasks_conflict_then_release_allows_retry() {
        let gate = gate();
        let key = key();
        let mut second_operation_ran = false;

        let conflict = gate.run("task-a", key.clone(), "fixture.key", || {
            gate.run("task-b", key.clone(), "fixture.key", || {
                second_operation_ran = true;
                Ok(())
            })
        });

        let error = conflict.expect_err("the second task must fail while task-a holds the lease");
        assert_eq!(error.error_code(), ErrorCode::Transient);
        assert!(
            !second_operation_ran,
            "a conflicting input must not be sent"
        );
        assert_eq!(
            count(&gate),
            0,
            "the failed outer call must release its lease"
        );

        gate.run("task-b", key, "fixture.key", || Ok(()))
            .expect("the released target must be acquirable by task-b");
        assert_eq!(
            count(&gate),
            0,
            "successful input must also release its lease"
        );
    }

    #[test]
    fn test_failed_operation_releases_lease() {
        let gate = gate();
        let key = key();

        let failure = gate.run("task-a", key.clone(), "fixture.key", || {
            Err::<(), _>(ToolBusError::InvalidArguments {
                tool: "fixture.key".to_owned(),
                reason: "fixture refused the input".to_owned(),
            })
        });
        assert_eq!(
            failure.expect_err("the operation must fail").error_code(),
            ErrorCode::ToolInvalidArgs
        );
        assert_eq!(count(&gate), 0, "failure must release the lease");

        gate.run("task-b", key, "fixture.key", || Ok(()))
            .expect("the failed task must not leave a lease behind");
        assert_eq!(count(&gate), 0);
    }

    #[test]
    fn test_conflicting_operation_is_not_invoked() {
        let gate = gate();
        let key = key();
        let mut nested_operation_ran = false;

        let outer = gate.run("task-a", key.clone(), "fixture.key", || {
            let nested = gate.run("task-b", key.clone(), "fixture.key", || {
                nested_operation_ran = true;
                Ok(())
            });
            assert_eq!(
                nested.expect_err("nested task conflicts").error_code(),
                ErrorCode::Transient
            );
            Ok::<(), ToolBusError>(())
        });
        outer.expect("outer operation still succeeds and releases");
        assert!(!nested_operation_ran);
        assert_eq!(count(&gate), 0);
    }

    #[test]
    fn test_pointer_operation_uses_the_same_exclusive_gate() {
        let gate = gate();
        let key = key();
        let mut pointer_sent = false;

        let conflict = gate.run("task-a", key.clone(), "pointer_action", || {
            gate.run("task-b", key.clone(), "pointer_action", || {
                pointer_sent = true;
                Ok(())
            })
        });
        assert_eq!(
            conflict
                .expect_err("pointer conflict must fail")
                .error_code(),
            ErrorCode::Transient
        );
        assert!(!pointer_sent, "the conflicting pointer must not be sent");
        assert_eq!(count(&gate), 0);

        gate.run("task-b", key, "pointer_action", || {
            pointer_sent = true;
            Ok(())
        })
        .expect("pointer input must succeed after release");
        assert!(pointer_sent);
        assert_eq!(count(&gate), 0);
    }

    #[test]
    fn test_two_task_gates_share_the_registry_and_conflict() {
        let registry = TargetLeaseRegistry::new();
        let gate_a = registry.gate(Arc::new(FixedClock));
        let gate_b = registry.gate(Arc::new(FixedClock));
        let key = key();
        let mut second_operation_ran = false;

        let conflict = gate_a.run("task-a", key.clone(), "fixture.key", || {
            gate_b.run("task-b", key.clone(), "fixture.key", || {
                second_operation_ran = true;
                Ok(())
            })
        });
        assert_eq!(
            conflict.expect_err("task-b must conflict").error_code(),
            ErrorCode::Transient
        );
        assert!(!second_operation_ran);
        assert_eq!(count(&gate_a), 0);

        gate_b
            .run("task-b", key, "fixture.key", || Ok(()))
            .expect("task-b succeeds after task-a releases");
        assert_eq!(count(&gate_a), 0);
    }
}
