//! Privacy retention modes for screenshot pipelines.

/// Whether a capture outcome may retain the platform-produced blob reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrivacyMode {
    /// Keep the `ImageRef` in the outcome so the caller can persist or verify it.
    PersistBlob,
    /// Drop the `ImageRef` before returning; only dimensions and decisions survive.
    NeverPersist,
}

impl PrivacyMode {
    /// Whether this mode allows the outcome to retain an `ImageRef`.
    #[must_use]
    pub const fn retains_image_ref(self) -> bool {
        match self {
            Self::PersistBlob => true,
            Self::NeverPersist => false,
        }
    }
}
