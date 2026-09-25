//! Conflict detection between the post-step fingerprint, current state, and rollback anchor.
//!
//! Responsibility: decide whether a rollback can proceed, is already complete, is blocked by user
//! changes, or lacks enough evidence to decide.
//!
//! Boundary: this module does not compute a user-facing diff. It returns the conservative decision
//! and leaves presentation and precise diff restoration to later layers.

use assistant_platform_api::Fingerprint;

use crate::anchor::Anchor;

/// How the caller wants a detectable user change to be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictResolution {
    /// Stop and produce an incident when the current state is not the post-step state.
    #[default]
    FailClosed,
    /// Restore the whole anchor even when later user changes will be overwritten.
    ///
    /// This variant must only be selected after a user explicitly chooses whole-anchor restore.
    RestoreOverall,
}

/// Result of comparing the expected post-step fingerprint with the observed current fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackConflict {
    /// Current state is exactly the post-step state, so rollback can proceed.
    NoConflict,
    /// Current state is already the pre-step state.
    AlreadyAtAnchor {
        /// Fingerprint observed at the anchor state.
        observed_fingerprint: Fingerprint,
    },
    /// Current state is neither the post-step nor the pre-step state.
    UserChanged {
        /// Fingerprint recorded after the agent's step.
        expected_post_fingerprint: Fingerprint,
        /// Fingerprint observed now.
        observed_fingerprint: Fingerprint,
    },
    /// Required evidence is missing or could not be observed.
    EvidenceMissing {
        /// What could not be established.
        reason: String,
    },
}

/// Detects whether current state is safe to roll back.
#[must_use]
pub fn detect_conflict(
    anchor: &Anchor,
    observed_current_fingerprint: Option<&Fingerprint>,
) -> RollbackConflict {
    let Some(post_fingerprint) = anchor.post_fingerprint() else {
        return RollbackConflict::EvidenceMissing {
            reason: "post-step fingerprint was not recorded".to_owned(),
        };
    };
    let Some(observed_fingerprint) = observed_current_fingerprint else {
        return RollbackConflict::EvidenceMissing {
            reason: "current fingerprint was not observed".to_owned(),
        };
    };
    if observed_fingerprint == post_fingerprint {
        RollbackConflict::NoConflict
    } else if observed_fingerprint == anchor.pre_fingerprint() {
        RollbackConflict::AlreadyAtAnchor {
            observed_fingerprint: observed_fingerprint.clone(),
        }
    } else {
        RollbackConflict::UserChanged {
            expected_post_fingerprint: post_fingerprint.clone(),
            observed_fingerprint: observed_fingerprint.clone(),
        }
    }
}

/// Returns whether the conflict blocks execution for the selected resolution.
#[must_use]
pub fn blocks_rollback(conflict: &RollbackConflict, resolution: ConflictResolution) -> bool {
    match conflict {
        RollbackConflict::NoConflict => false,
        RollbackConflict::AlreadyAtAnchor { .. } | RollbackConflict::EvidenceMissing { .. } => true,
        RollbackConflict::UserChanged { .. } => resolution == ConflictResolution::FailClosed,
    }
}
