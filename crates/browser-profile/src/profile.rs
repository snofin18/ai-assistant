//! Dedicated profile lifecycle and no-copy-user-profile invariant.

use std::path::{Path, PathBuf};

use crate::error::{BrowserProfileError, BrowserProfileResult};

const MAX_PROFILE_ID_BYTES: usize = 64;

/// Supported CDP browser family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BrowserKind {
    /// Microsoft Edge.
    Edge,
    /// Google Chrome.
    Chrome,
}

impl BrowserKind {
    /// Stable lowercase browser name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Chrome => "chrome",
        }
    }
}

/// One dedicated automation profile.
///
/// The profile owns a separate `--user-data-dir`. It never reuses or copies the
/// user's normal browser profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct BrowserProfile {
    profile_id: String,
    kind: BrowserKind,
    user_data_dir: PathBuf,
    first_login_required: bool,
}

impl BrowserProfile {
    /// Creates a dedicated profile.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserProfileError::InvalidProfileId`] when `profile_id` is not one
    /// bounded path component, and [`BrowserProfileError::InvalidUserDataDir`] when
    /// `user_data_dir` is empty or contains NUL.
    pub fn new(
        profile_id: impl Into<String>,
        kind: BrowserKind,
        user_data_dir: impl AsRef<Path>,
    ) -> BrowserProfileResult<Self> {
        let profile_id = profile_id.into();
        let user_data_dir = PathBuf::from(user_data_dir.as_ref());
        validate_profile_id(&profile_id)?;
        validate_user_data_dir(&user_data_dir)?;
        Ok(Self {
            profile_id,
            kind,
            user_data_dir,
            first_login_required: false,
        })
    }

    /// Stable profile identifier.
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// Browser family.
    #[must_use]
    pub const fn browser_kind(&self) -> BrowserKind {
        self.kind
    }

    /// Dedicated user-data directory.
    #[must_use]
    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_dir
    }

    /// Launch arguments pinned to this dedicated directory.
    #[must_use]
    pub fn launch_args(&self) -> [String; 1] {
        [format!(
            "--user-data-dir={}",
            self.user_data_dir.to_string_lossy()
        )]
    }

    /// Marks whether the next session must wait for a first human login.
    pub const fn set_first_login_required(&mut self, required: bool) {
        self.first_login_required = required;
    }

    /// Whether first login is required.
    #[must_use]
    pub const fn is_first_login_required(&self) -> bool {
        self.first_login_required
    }

    /// Stable window identity suffix for this profile.
    #[must_use]
    pub fn window_title_suffix(&self) -> String {
        format!("{}-{}", self.kind.as_str(), self.profile_id)
    }

    /// Always refuses a user-profile copy.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserProfileError::CopyUserProfileForbidden`] unconditionally.
    pub const fn copy_user_profile(&self) -> BrowserProfileResult<()> {
        Err(BrowserProfileError::CopyUserProfileForbidden)
    }
}

fn validate_profile_id(profile_id: &str) -> BrowserProfileResult<()> {
    let is_invalid_component = profile_id == "."
        || profile_id == ".."
        || profile_id.ends_with('.')
        || profile_id.ends_with(' ');
    let has_invalid_character = profile_id
        .chars()
        .any(|character| character == '\u{0}' || character.is_control());
    if profile_id.trim().is_empty()
        || profile_id.len() > MAX_PROFILE_ID_BYTES
        || profile_id.contains(['/', '\\', ':'])
        || is_invalid_component
        || has_invalid_character
    {
        return Err(BrowserProfileError::InvalidProfileId);
    }
    Ok(())
}

fn validate_user_data_dir(user_data_dir: &Path) -> BrowserProfileResult<()> {
    if user_data_dir.as_os_str().is_empty() {
        return Err(BrowserProfileError::InvalidUserDataDir);
    }
    let Some(text) = user_data_dir.to_str() else {
        return Err(BrowserProfileError::InvalidUserDataDir);
    };
    if text.contains('\u{0}') {
        return Err(BrowserProfileError::InvalidUserDataDir);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedicated_profile_pins_user_data_dir_and_title_suffix() -> BrowserProfileResult<()> {
        let profile = BrowserProfile::new(
            "spike-g",
            BrowserKind::Edge,
            r"C:\assistant\profiles\spike-g",
        )?;
        assert_eq!(
            profile.user_data_dir().to_string_lossy(),
            r"C:\assistant\profiles\spike-g"
        );
        assert_eq!(profile.window_title_suffix(), "edge-spike-g");
        assert_eq!(
            profile.launch_args()[0],
            "--user-data-dir=C:\\assistant\\profiles\\spike-g"
        );
        Ok(())
    }

    #[test]
    fn test_user_profile_copy_is_forbidden() -> BrowserProfileResult<()> {
        let profile = BrowserProfile::new("isolated", BrowserKind::Chrome, "profiles/isolated")?;
        assert!(matches!(
            profile.copy_user_profile(),
            Err(BrowserProfileError::CopyUserProfileForbidden)
        ));
        Ok(())
    }

    #[test]
    fn test_empty_or_null_identifiers_are_rejected() {
        assert!(matches!(
            BrowserProfile::new("", BrowserKind::Edge, "profiles/a"),
            Err(BrowserProfileError::InvalidProfileId)
        ));
        assert!(matches!(
            BrowserProfile::new("a\0b", BrowserKind::Edge, "profiles/a"),
            Err(BrowserProfileError::InvalidProfileId)
        ));
        assert!(matches!(
            BrowserProfile::new("a", BrowserKind::Edge, ""),
            Err(BrowserProfileError::InvalidUserDataDir)
        ));
    }

    #[test]
    fn test_profile_id_must_be_one_bounded_path_component() {
        for profile_id in [
            ".",
            "..",
            "../escape",
            r"..\escape",
            "C:escape",
            "a:b",
            "a/b",
            "a\\b",
        ] {
            assert!(
                matches!(
                    BrowserProfile::new(profile_id, BrowserKind::Edge, "profiles/a"),
                    Err(BrowserProfileError::InvalidProfileId)
                ),
                "profile id should be rejected: {profile_id}"
            );
        }

        let oversized = "a".repeat(MAX_PROFILE_ID_BYTES + 1);
        assert!(matches!(
            BrowserProfile::new(oversized, BrowserKind::Edge, "profiles/a"),
            Err(BrowserProfileError::InvalidProfileId)
        ));
    }
}
