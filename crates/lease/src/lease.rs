//! Lease records and acquisition requests.

use std::fmt;

use crate::key::{LeaseKey, LeaseOwner};
use crate::mode::LeaseMode;

/// Deterministic identifier of one lease record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeaseId(u64);

impl LeaseId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric identifier.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LeaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A validated request for one target lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseRequest {
    key: LeaseKey,
    mode: LeaseMode,
    ttl_ms: u64,
}

impl LeaseRequest {
    /// Creates a request.
    ///
    /// TTL zero and overflow are rejected when the manager processes the request.
    #[must_use]
    pub const fn new(key: LeaseKey, mode: LeaseMode, ttl_ms: u64) -> Self {
        Self { key, mode, ttl_ms }
    }

    /// Returns the requested target key.
    #[must_use]
    pub const fn key(&self) -> &LeaseKey {
        &self.key
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(&self) -> LeaseMode {
        self.mode
    }

    /// Returns the requested lifetime in milliseconds.
    #[must_use]
    pub const fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }
}

/// One active target lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    id: LeaseId,
    key: LeaseKey,
    mode: LeaseMode,
    owner: LeaseOwner,
    acquired_at_ms: i64,
    renewed_at_ms: i64,
    expires_at_ms: i64,
}

impl Lease {
    pub(crate) const fn new(
        id: LeaseId,
        key: LeaseKey,
        mode: LeaseMode,
        owner: LeaseOwner,
        now_ms: i64,
        expires_at_ms: i64,
    ) -> Self {
        Self {
            id,
            key,
            mode,
            owner,
            acquired_at_ms: now_ms,
            renewed_at_ms: now_ms,
            expires_at_ms,
        }
    }

    /// Returns the lease identifier.
    #[must_use]
    pub const fn id(&self) -> LeaseId {
        self.id
    }

    /// Returns the target key.
    #[must_use]
    pub const fn key(&self) -> &LeaseKey {
        &self.key
    }

    /// Returns the lease mode.
    #[must_use]
    pub const fn mode(&self) -> LeaseMode {
        self.mode
    }

    /// Returns the owner.
    #[must_use]
    pub const fn owner(&self) -> &LeaseOwner {
        &self.owner
    }

    /// Returns the acquisition timestamp.
    #[must_use]
    pub const fn acquired_at_ms(&self) -> i64 {
        self.acquired_at_ms
    }

    /// Returns the most recent renewal timestamp.
    #[must_use]
    pub const fn renewed_at_ms(&self) -> i64 {
        self.renewed_at_ms
    }

    /// Returns the exclusive expiration timestamp.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns whether this lease is active at `now_ms`.
    ///
    /// The expiration boundary is exclusive: `expires_at_ms == now_ms` is already expired.
    #[must_use]
    pub const fn is_active_at(&self, now_ms: i64) -> bool {
        now_ms < self.expires_at_ms
    }

    pub(crate) const fn renew(&mut self, now_ms: i64, expires_at_ms: i64) {
        self.renewed_at_ms = now_ms;
        self.expires_at_ms = expires_at_ms;
    }
}
