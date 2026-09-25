//! Target leases for architecture v2 section 8.8.
//!
//! Responsibilities:
//! - validate target lease keys and owners before they enter manager state;
//! - model `shared`, `intent`, and `exclusive` lease modes;
//! - enforce one writer per target while allowing compatible readers/planners;
//! - apply caller-injected timestamps for TTL expiry and renewal;
//! - force-release agent leases when the user takes over a target;
//! - acquire multiple target leases in canonical key order with all-or-nothing commit.
//!
//! Boundaries:
//! - no platform API, target resolution, process access, filesystem, network, or persistence;
//! - no background timer. Callers inject `now_ms` and may call [`LeaseManager::reap_expired`];
//! - no policy decision. Whether an operation may proceed is decided by `crates/policy`;
//! - no identifier generation from randomness. Lease identifiers are deterministic sequence numbers.
//!
//! Invariants:
//! 1. At most one active `exclusive` lease exists for a target key across owners.
//! 2. An `exclusive` lease is incompatible with every other owner's active lease.
//! 3. Expired leases never block acquisition and are removed by lazy cleanup or explicit reaping.
//! 4. A renewal or release by a different owner is rejected.
//! 5. Multi-lease acquisition sorts requests by canonical key and commits no leases if any request
//!    conflicts.
//! 6. User preemption releases every active agent lease for the selected key; it never silently
//!    drops expired leases from the report.
//!
//! Typical use:
//! ```rust
//! use assistant_lease::{LeaseKey, LeaseManager, LeaseMode, LeaseOwner};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let key = LeaseKey::new("com.microsoft.notepad")?.with_document_id("doc_1")?;
//! let owner = LeaseOwner::parse("task_1")?;
//! let mut manager = LeaseManager::new();
//! let lease = manager.acquire(key, LeaseMode::Exclusive, &owner, 30_000, 1_000)?;
//! assert!(lease.is_active_at(30_999));
//! assert!(!lease.is_active_at(31_000));
//! # Ok(())
//! # }
//! ```
//!
//! Related documents: architecture v2 section 8.8, `docs/spec/naming.md` section 7,
//! `docs/spec/error-codes.md`, and `tasks/TASK-025-lease-target-exclusive-shared-intent.md`.

#![deny(unsafe_code)]

mod error;
mod key;
mod lease;
mod manager;
mod mode;

pub use error::{LeaseConflict, LeaseError, LeaseResult};
pub use key::{LeaseKey, LeaseOwner};
pub use lease::{Lease, LeaseId, LeaseRequest};
pub use manager::{LeaseManager, PreemptionReport};
pub use mode::LeaseMode;
