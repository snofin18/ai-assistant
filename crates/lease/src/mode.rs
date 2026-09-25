//! Lease modes and their compatibility matrix.

/// How a caller intends to use a target while holding its lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LeaseMode {
    /// Read-only use. Multiple owners may hold this mode concurrently.
    Shared,
    /// Planning or preparation for a later write. Other planners and readers may coexist.
    Intent,
    /// Write use. This mode excludes every other owner's active lease on the same key.
    Exclusive,
}

impl LeaseMode {
    /// Returns the stable lowercase mode name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Intent => "intent",
            Self::Exclusive => "exclusive",
        }
    }

    /// Returns whether this mode grants write access.
    #[must_use]
    pub const fn is_write(self) -> bool {
        matches!(self, Self::Exclusive)
    }

    /// Returns whether two modes from different owners conflict.
    ///
    /// The matrix is intentionally conservative for `intent`: an intent blocks another owner's
    /// exclusive acquisition so the planner can finish and upgrade in a deterministic order.
    #[must_use]
    pub const fn conflicts_with(self, other: Self) -> bool {
        matches!(self, Self::Exclusive) || matches!(other, Self::Exclusive)
    }
}
