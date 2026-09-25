//! The four-level reversibility model from architecture v2 section 9.1.
//!
//! Responsibility: represent L0 through L3, parse their stable contract strings, and compute the
//! worst level in a task.
//!
//! Boundary: read-only tools are not a fifth rollback level. They never produce a write anchor,
//! so this module rejects `none_readonly` instead of silently pretending it is L3.

use crate::error::{UndoError, UndoResult};

/// Reversibility of one step or of a whole task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reversibility {
    /// The application has an undo stack, so a bounded number of steps can be undone.
    L0UndoStack,
    /// Complete content or a file shadow copy can restore the target.
    L1Snapshot,
    /// A semantic compensating action can reverse the effect.
    L2Compensating,
    /// No rollback exists; the step requires human confirmation and evidence only.
    L3Irreversible,
}

impl Reversibility {
    /// Parses a stable tool-schema value.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::CapabilityMissing`] for `none_readonly` because a read-only step has no
    /// rollback behavior to model, and returns [`UndoError::InvalidIdentifier`] for unknown strings
    /// so a typo cannot silently become an irreversible action.
    pub fn parse(value: &str) -> UndoResult<Self> {
        match value {
            "L0_undo_stack" => Ok(Self::L0UndoStack),
            "L1_snapshot" => Ok(Self::L1Snapshot),
            "L2_compensating" => Ok(Self::L2Compensating),
            "L3_irreversible" => Ok(Self::L3Irreversible),
            "none_readonly" => Err(UndoError::CapabilityMissing {
                capability: "rollback_anchor",
                context: "read-only steps do not produce rollback anchors".to_owned(),
            }),
            other => Err(UndoError::InvalidIdentifier {
                kind: "reversibility",
                reason: format!(
                    "unknown value `{other}`; expected L0_undo_stack, L1_snapshot, \
                     L2_compensating, or L3_irreversible"
                ),
            }),
        }
    }

    /// Returns the stable tool-schema string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L0UndoStack => "L0_undo_stack",
            Self::L1Snapshot => "L1_snapshot",
            Self::L2Compensating => "L2_compensating",
            Self::L3Irreversible => "L3_irreversible",
        }
    }

    /// Returns the monotonic severity used by the bucket rule.
    #[must_use]
    pub const fn severity(self) -> u8 {
        match self {
            Self::L0UndoStack => 0,
            Self::L1Snapshot => 1,
            Self::L2Compensating => 2,
            Self::L3Irreversible => 3,
        }
    }

    /// Returns whether this level permanently forbids unattended execution.
    #[must_use]
    pub const fn is_irreversible(self) -> bool {
        matches!(self, Self::L3Irreversible)
    }

    /// Returns whether the level requires explicit human confirmation.
    #[must_use]
    pub const fn requires_human_confirmation(self) -> bool {
        self.is_irreversible()
    }

    /// Returns the worst level among the supplied steps.
    #[must_use]
    pub fn worst(levels: impl IntoIterator<Item = Self>) -> Option<Self> {
        levels.into_iter().max_by_key(|level| level.severity())
    }
}
