//! In-memory target-lease manager.

use std::collections::BTreeMap;

use crate::error::{LeaseConflict, LeaseError, LeaseResult};
use crate::key::{LeaseKey, LeaseOwner};
use crate::lease::{Lease, LeaseId, LeaseRequest};
use crate::mode::LeaseMode;

/// Result of a user takeover for one target key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreemptionReport {
    preempted: Vec<Lease>,
    expired: Vec<Lease>,
}

impl PreemptionReport {
    /// Returns the active agent leases force-released by the user.
    #[must_use]
    pub fn preempted(&self) -> &[Lease] {
        &self.preempted
    }

    /// Returns leases discovered already expired during the same operation.
    #[must_use]
    pub fn expired(&self) -> &[Lease] {
        &self.expired
    }
}

/// Deterministic in-memory manager for target leases.
///
/// The manager has no background timer. Every operation observes `now_ms`; acquisition lazily
/// reaps expired leases, and callers may use [`Self::reap_expired`] when they need an explicit
/// sweep.
#[derive(Debug)]
pub struct LeaseManager {
    leases: BTreeMap<LeaseId, Lease>,
    next_lease_id: u64,
    last_observed_ms: Option<i64>,
}

impl LeaseManager {
    /// Creates an empty manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            leases: BTreeMap::new(),
            next_lease_id: 1,
            last_observed_ms: None,
        }
    }

    /// Acquires one target lease.
    ///
    /// Expired leases are removed before conflict evaluation. A conflicting request returns a
    /// readable [`LeaseError::Conflict`] and leaves all active leases unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidTtl`] or [`LeaseError::InvalidTimestamp`] for invalid input,
    /// [`LeaseError::ClockWentBackwards`] when time regresses, [`LeaseError::NumericOverflow`]
    /// when the expiration cannot be represented, and [`LeaseError::Conflict`] when another owner
    /// holds an incompatible lease.
    pub fn acquire(
        &mut self,
        key: LeaseKey,
        mode: LeaseMode,
        owner: &LeaseOwner,
        ttl_ms: u64,
        now_ms: i64,
    ) -> LeaseResult<Lease> {
        let leases = self.acquire_many(owner, &[LeaseRequest::new(key, mode, ttl_ms)], now_ms)?;
        leases
            .into_iter()
            .next()
            .ok_or(LeaseError::InternalInvariant {
                reason: "single acquisition returned no lease",
            })
    }

    /// Acquires several target leases atomically.
    ///
    /// Requests are validated, sorted by canonical target key, and conflict-checked before any
    /// lease identifier is allocated. On failure the manager commits none of the requested leases,
    /// which removes the partial-ownership rollback window described in architecture v2 section
    /// 8.8.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::DuplicateRequestKey`] when a key appears twice, the same validation
    /// errors as [`Self::acquire`], and [`LeaseError::Conflict`] when any request conflicts with
    /// an existing active lease.
    pub fn acquire_many(
        &mut self,
        owner: &LeaseOwner,
        requests: &[LeaseRequest],
        now_ms: i64,
    ) -> LeaseResult<Vec<Lease>> {
        self.observe_time(now_ms)?;
        self.remove_expired(now_ms);

        let mut ordered = requests.to_vec();
        ordered.sort_by(|left, right| left.key().cmp(right.key()));
        validate_unique_keys(&ordered)?;
        validate_requests(&ordered, now_ms)?;

        let mut staged = Vec::with_capacity(ordered.len());
        for request in &ordered {
            let blockers = self.blockers_for(request.key(), request.mode(), owner);
            if !blockers.is_empty() {
                return Err(LeaseError::Conflict(Box::new(LeaseConflict::new(
                    request.key().clone(),
                    request.mode(),
                    owner.as_str().to_owned(),
                    blockers,
                ))));
            }
            staged.push(request.clone());
        }

        // Plan every record before mutating manager state. This keeps identifier-allocation and
        // expiration failures from leaving a partially committed batch.
        let mut next_lease_id = self.next_lease_id;
        let mut acquired = Vec::with_capacity(staged.len());
        for request in &staged {
            let lease_id = allocate_lease_id(&mut next_lease_id)?;
            let expires_at_ms = expiration(request.ttl_ms(), now_ms)?;
            let lease = Lease::new(
                lease_id,
                request.key().clone(),
                request.mode(),
                owner.clone(),
                now_ms,
                expires_at_ms,
            );
            acquired.push(lease);
        }
        for lease in &acquired {
            self.leases.insert(lease.id(), lease.clone());
        }
        self.next_lease_id = next_lease_id;
        Ok(acquired)
    }

    /// Renews a lease owned by `owner`.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::LeaseNotFound`] for an unknown identifier,
    /// [`LeaseError::OwnerMismatch`] when `owner` is not the lease owner,
    /// [`LeaseError::LeaseExpired`] when the current time is at or past expiration, and the same
    /// TTL/time/overflow errors as acquisition.
    pub fn renew(
        &mut self,
        lease_id: LeaseId,
        owner: &LeaseOwner,
        ttl_ms: u64,
        now_ms: i64,
    ) -> LeaseResult<Lease> {
        self.observe_time(now_ms)?;
        validate_ttl(ttl_ms)?;
        validate_timestamp(now_ms)?;

        let lease = self
            .leases
            .get(&lease_id)
            .cloned()
            .ok_or(LeaseError::LeaseNotFound { lease_id })?;
        ensure_owner(&lease, owner)?;
        if !lease.is_active_at(now_ms) {
            return Err(LeaseError::LeaseExpired {
                lease_id,
                expires_at_ms: lease.expires_at_ms(),
                now_ms,
            });
        }

        let expires_at_ms = expiration(ttl_ms, now_ms)?;
        let updated = self
            .leases
            .get_mut(&lease_id)
            .ok_or(LeaseError::LeaseNotFound { lease_id })?;
        updated.renew(now_ms, expires_at_ms);
        Ok(updated.clone())
    }

    /// Releases a lease owned by `owner`.
    ///
    /// Release is explicit and not idempotent: an unknown identifier is an error.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::LeaseNotFound`] for an unknown identifier and
    /// [`LeaseError::OwnerMismatch`] when `owner` is not the lease owner.
    pub fn release(&mut self, lease_id: LeaseId, owner: &LeaseOwner) -> LeaseResult<Lease> {
        let lease = self
            .leases
            .get(&lease_id)
            .cloned()
            .ok_or(LeaseError::LeaseNotFound { lease_id })?;
        ensure_owner(&lease, owner)?;
        self.leases.remove(&lease_id);
        Ok(lease)
    }

    /// Removes every lease that has expired at `now_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidTimestamp`] for a negative timestamp and
    /// [`LeaseError::ClockWentBackwards`] when time regresses.
    pub fn reap_expired(&mut self, now_ms: i64) -> LeaseResult<Vec<Lease>> {
        self.observe_time(now_ms)?;
        Ok(self.remove_expired(now_ms))
    }

    /// Force-releases all active agent leases for one target after a user takeover.
    ///
    /// The operation is idempotent for an already-empty target. Expired leases are reaped in the
    /// same call and returned separately so they are never silently discarded.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidTimestamp`] for a negative timestamp and
    /// [`LeaseError::ClockWentBackwards`] when time regresses.
    pub fn preempt_for_user(
        &mut self,
        key: &LeaseKey,
        now_ms: i64,
    ) -> LeaseResult<PreemptionReport> {
        self.observe_time(now_ms)?;
        let expired = self.remove_expired(now_ms);
        let mut preempted = Vec::new();
        self.leases.retain(|_, lease| {
            if lease.key() == key {
                preempted.push(lease.clone());
                false
            } else {
                true
            }
        });
        Ok(PreemptionReport { preempted, expired })
    }

    /// Returns the number of currently stored leases, including not-yet-reaped expired leases.
    #[must_use]
    pub fn stored_lease_count(&self) -> usize {
        self.leases.len()
    }

    /// Returns a clone of one stored lease.
    #[must_use]
    pub fn get(&self, lease_id: LeaseId) -> Option<Lease> {
        self.leases.get(&lease_id).cloned()
    }

    fn observe_time(&mut self, now_ms: i64) -> LeaseResult<()> {
        validate_timestamp(now_ms)?;
        if let Some(previous_ms) = self.last_observed_ms
            && now_ms < previous_ms
        {
            return Err(LeaseError::ClockWentBackwards {
                previous_ms,
                current_ms: now_ms,
            });
        }
        self.last_observed_ms = Some(now_ms);
        Ok(())
    }

    fn remove_expired(&mut self, now_ms: i64) -> Vec<Lease> {
        let mut expired = Vec::new();
        self.leases.retain(|_, lease| {
            if lease.is_active_at(now_ms) {
                true
            } else {
                expired.push(lease.clone());
                false
            }
        });
        expired
    }

    fn blockers_for(
        &self,
        key: &LeaseKey,
        requested_mode: LeaseMode,
        requested_owner: &LeaseOwner,
    ) -> Vec<Lease> {
        let mut blockers: Vec<Lease> = self
            .leases
            .values()
            .filter(|lease| {
                lease.key() == key
                    && lease.owner() != requested_owner
                    && requested_mode.conflicts_with(lease.mode())
            })
            .cloned()
            .collect();
        blockers.sort_by_key(Lease::id);
        blockers
    }
}

impl Default for LeaseManager {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_unique_keys(requests: &[LeaseRequest]) -> LeaseResult<()> {
    for (index, request) in requests.iter().enumerate() {
        let duplicated = requests
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key() == request.key());
        if duplicated {
            return Err(LeaseError::DuplicateRequestKey {
                key: request.key().clone(),
            });
        }
    }
    Ok(())
}

fn validate_requests(requests: &[LeaseRequest], now_ms: i64) -> LeaseResult<()> {
    validate_timestamp(now_ms)?;
    for request in requests {
        validate_ttl(request.ttl_ms())?;
        expiration(request.ttl_ms(), now_ms)?;
    }
    Ok(())
}

fn validate_ttl(ttl_ms: u64) -> LeaseResult<()> {
    if ttl_ms == 0 || i64::try_from(ttl_ms).is_err() {
        return Err(LeaseError::InvalidTtl { ttl_ms });
    }
    Ok(())
}

const fn validate_timestamp(now_ms: i64) -> LeaseResult<()> {
    if now_ms < 0 {
        return Err(LeaseError::InvalidTimestamp { now_ms });
    }
    Ok(())
}

fn expiration(ttl_ms: u64, now_ms: i64) -> LeaseResult<i64> {
    let ttl_ms = i64::try_from(ttl_ms).map_err(|_| LeaseError::InvalidTtl { ttl_ms })?;
    now_ms
        .checked_add(ttl_ms)
        .ok_or(LeaseError::NumericOverflow {
            field: "expires_at_ms",
        })
}

fn allocate_lease_id(next_lease_id: &mut u64) -> LeaseResult<LeaseId> {
    let lease_id = LeaseId::new(*next_lease_id);
    *next_lease_id = next_lease_id
        .checked_add(1)
        .ok_or(LeaseError::NumericOverflow {
            field: "next_lease_id",
        })?;
    Ok(lease_id)
}

fn ensure_owner(lease: &Lease, owner: &LeaseOwner) -> LeaseResult<()> {
    if lease.owner() == owner {
        return Ok(());
    }
    Err(LeaseError::OwnerMismatch {
        lease_id: lease.id(),
        actual_owner: lease.owner().as_str().to_owned(),
        supplied_owner: owner.as_str().to_owned(),
    })
}
