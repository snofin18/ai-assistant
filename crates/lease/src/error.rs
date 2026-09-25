//! Structured lease failures and their stable protocol error categories.

use std::fmt;

use assistant_protocol::ErrorCode;

use crate::key::LeaseKey;
use crate::lease::{Lease, LeaseId};
use crate::mode::LeaseMode;

/// Result alias for lease operations.
pub type LeaseResult<T> = Result<T, LeaseError>;

/// Details of a rejected lease acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseConflict {
    key: LeaseKey,
    requested_mode: LeaseMode,
    requested_owner: String,
    blockers: Vec<Lease>,
}

impl LeaseConflict {
    pub(crate) const fn new(
        key: LeaseKey,
        requested_mode: LeaseMode,
        requested_owner: String,
        blockers: Vec<Lease>,
    ) -> Self {
        Self {
            key,
            requested_mode,
            requested_owner,
            blockers,
        }
    }

    /// Returns the target key on which acquisition failed.
    #[must_use]
    pub const fn key(&self) -> &LeaseKey {
        &self.key
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn requested_mode(&self) -> LeaseMode {
        self.requested_mode
    }

    /// Returns the requesting owner.
    #[must_use]
    pub fn requested_owner(&self) -> &str {
        &self.requested_owner
    }

    /// Returns the active leases that blocked acquisition.
    #[must_use]
    pub fn blockers(&self) -> &[Lease] {
        &self.blockers
    }
}

/// A deterministic target-lease failure.
///
/// Every variant maps to an existing protocol [`ErrorCode`]; no lease-specific protocol code is
/// invented by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LeaseError {
    /// An identifier failed validation.
    InvalidIdentifier {
        /// Identifier domain.
        kind: &'static str,
        /// Human-readable rejection reason.
        reason: String,
    },

    /// A TTL was zero or could not be represented by the manager's clock domain.
    InvalidTtl {
        /// Rejected TTL in milliseconds.
        ttl_ms: u64,
    },

    /// A timestamp was negative.
    InvalidTimestamp {
        /// Rejected timestamp.
        now_ms: i64,
    },

    /// The injected clock moved backwards.
    ClockWentBackwards {
        /// Last observed timestamp.
        previous_ms: i64,
        /// Rejected timestamp.
        current_ms: i64,
    },

    /// A counter or expiration addition overflowed.
    NumericOverflow {
        /// Field that overflowed.
        field: &'static str,
    },

    /// The requested lease conflicts with one or more active leases.
    Conflict(Box<LeaseConflict>),

    /// A lease identifier is not present.
    LeaseNotFound {
        /// Missing lease.
        lease_id: LeaseId,
    },

    /// A lease expired before renewal.
    LeaseExpired {
        /// Expired lease.
        lease_id: LeaseId,
        /// Expiration timestamp.
        expires_at_ms: i64,
        /// Current timestamp.
        now_ms: i64,
    },

    /// The caller did not own the lease it attempted to renew or release.
    OwnerMismatch {
        /// Lease being changed.
        lease_id: LeaseId,
        /// Actual owner.
        actual_owner: String,
        /// Caller-supplied owner.
        supplied_owner: String,
    },

    /// A batch request named the same target key more than once.
    DuplicateRequestKey {
        /// Duplicated key.
        key: LeaseKey,
    },

    /// An internal invariant failed and no state was committed.
    InternalInvariant {
        /// Invariant that was violated.
        reason: &'static str,
    },
}

impl LeaseError {
    /// Maps the failure to the stable protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidIdentifier { .. }
            | Self::InvalidTtl { .. }
            | Self::InvalidTimestamp { .. }
            | Self::LeaseNotFound { .. }
            | Self::OwnerMismatch { .. }
            | Self::DuplicateRequestKey { .. } => ErrorCode::ToolInvalidArgs,
            Self::ClockWentBackwards { .. }
            | Self::NumericOverflow { .. }
            | Self::InternalInvariant { .. } => ErrorCode::Fatal,
            Self::Conflict(_) | Self::LeaseExpired { .. } => ErrorCode::Transient,
        }
    }
}

impl fmt::Display for LeaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { kind, reason } => {
                write!(formatter, "invalid {kind}: {reason}")
            }
            Self::InvalidTtl { ttl_ms } => {
                write!(
                    formatter,
                    "lease TTL must be positive and fit i64, got {ttl_ms} ms"
                )
            }
            Self::InvalidTimestamp { now_ms } => {
                write!(
                    formatter,
                    "lease timestamp must be non-negative, got {now_ms}"
                )
            }
            Self::ClockWentBackwards {
                previous_ms,
                current_ms,
            } => write!(
                formatter,
                "lease clock moved backwards: previous {previous_ms}, current {current_ms}"
            ),
            Self::NumericOverflow { field } => {
                write!(formatter, "numeric overflow while updating {field}")
            }
            Self::Conflict(conflict) => {
                write!(
                    formatter,
                    "lease conflict on [{key}]: owner {requested_owner} requested {}; blockers: ",
                    conflict.requested_mode().as_str(),
                    key = conflict.key(),
                    requested_owner = conflict.requested_owner(),
                )?;
                for (index, blocker) in conflict.blockers().iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(
                        formatter,
                        "lease {} owned by {} in {} mode until {}",
                        blocker.id(),
                        blocker.owner(),
                        blocker.mode().as_str(),
                        blocker.expires_at_ms()
                    )?;
                }
                Ok(())
            }
            Self::LeaseNotFound { lease_id } => write!(formatter, "lease {lease_id} not found"),
            Self::LeaseExpired {
                lease_id,
                expires_at_ms,
                now_ms,
            } => write!(
                formatter,
                "lease {lease_id} expired at {expires_at_ms}; current time is {now_ms}"
            ),
            Self::OwnerMismatch {
                lease_id,
                actual_owner,
                supplied_owner,
            } => write!(
                formatter,
                "lease {lease_id} is owned by {actual_owner}, not {supplied_owner}"
            ),
            Self::DuplicateRequestKey { key } => {
                write!(formatter, "batch contains duplicate target key [{key}]")
            }
            Self::InternalInvariant { reason } => {
                write!(
                    formatter,
                    "lease manager internal invariant failed: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for LeaseError {}
