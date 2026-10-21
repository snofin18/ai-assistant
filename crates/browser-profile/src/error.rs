//! Dedicated browser-profile failures.

use std::fmt;

/// Stable failure categories for dedicated-profile lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BrowserProfileError {
    /// The profile identifier is empty or contains an invalid character.
    InvalidProfileId,
    /// The user-data directory is empty or absolute-only whitespace.
    InvalidUserDataDir,
    /// The requested operation would copy or reuse the user profile.
    CopyUserProfileForbidden,
    /// The browser kind is not supported by this manager.
    UnsupportedBrowserKind,
}

impl BrowserProfileError {
    /// Stable lowercase label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidProfileId => "invalid_profile_id",
            Self::InvalidUserDataDir => "invalid_user_data_dir",
            Self::CopyUserProfileForbidden => "copy_user_profile_forbidden",
            Self::UnsupportedBrowserKind => "unsupported_browser_kind",
        }
    }
}

impl fmt::Display for BrowserProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for BrowserProfileError {}

/// Alias for public profile results.
pub type BrowserProfileResult<T> = Result<T, BrowserProfileError>;
