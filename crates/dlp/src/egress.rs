//! Pure egress-policy resolution for the three DLP tiers
//! (`docs/adr/0007-egress-policy-tiers-and-resolution.md`).
//!
//! Responsibilities:
//! - resolve the effective level from the global default plus per-application
//!   and per-content-type overrides;
//! - reject cloud/local-model routing when the selected tier cannot be honored;
//! - expose bounded policy mutation records so the host can audit changes.
//!
//! Boundaries:
//! - no IO, clocks, random values, network calls, or persistence;
//! - no platform access or model-provider selection;
//! - no redaction execution; decisions only state whether redaction is required.
//!
//! Invariants:
//! 1. The effective level is the strictest of all matching settings.
//! 2. `LocalOnly` without an available local model is an explicit error.
//! 3. Non-local egress must match the `egress_destination` allow-list exactly.
//! 4. Policy collections have hard bounds and reject duplicate keys.

/// Maximum number of per-application overrides.
pub const MAX_APP_OVERRIDES: usize = 128;

/// Maximum number of `egress_destination` allow-list entries.
pub const MAX_ALLOWED_EGRESS_DESTINATIONS: usize = 128;

const MAX_IDENTIFIER_BYTES: usize = 255;
const CONTENT_TYPE_COUNT: usize = 4;

/// User-selectable egress level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum EgressLevel {
    /// Local model only; no data leaves the machine.
    LocalOnly,
    /// Cloud egress is allowed only after redaction.
    #[default]
    Redacted,
    /// Cloud egress is allowed with audit only.
    Full,
}

impl EgressLevel {
    /// Returns the stricter of two levels.
    #[must_use]
    pub const fn strictest(self, other: Self) -> Self {
        match (self, other) {
            (Self::LocalOnly, _) | (_, Self::LocalOnly) => Self::LocalOnly,
            (Self::Redacted, _) | (_, Self::Redacted) => Self::Redacted,
            (Self::Full, Self::Full) => Self::Full,
        }
    }

    /// Returns whether this level widens egress compared with `other`.
    #[must_use]
    pub const fn is_wider_than(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Redacted, Self::LocalOnly) | (Self::Full, Self::LocalOnly | Self::Redacted)
        )
    }
}

/// Content category used by a DLP decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    /// Window screenshot pixels or derived visual data.
    Screenshot,
    /// Clipboard text or structured content.
    Clipboard,
    /// File content read from disk.
    FileContent,
    /// Accessibility tree or UI metadata.
    UiTree,
}

/// Validated application identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppId(String);

impl AppId {
    /// Creates a canonical, lowercase application identifier.
    ///
    /// # Errors
    /// Returns [`EgressError::InvalidAppId`] when the value is empty, too long,
    /// contains non-ASCII or disallowed characters, or is not already lowercase.
    pub fn new(value: impl Into<String>) -> Result<Self, EgressError> {
        let value = value.into();
        validate_identifier(&value, false, false).map_err(|()| EgressError::InvalidAppId)?;
        Ok(Self(value))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_text(&self) -> &str {
        &self.0
    }
}

/// Validated `egress_destination` allow-list identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EgressDestinationId(String);

impl EgressDestinationId {
    /// Creates a canonical, lowercase `egress_destination` identifier.
    ///
    /// # Errors
    /// Returns [`EgressError::InvalidEgressDestination`] when the value is empty, too long,
    /// contains non-ASCII or disallowed characters, or is not already lowercase.
    pub fn new(value: impl Into<String>) -> Result<Self, EgressError> {
        let value = value.into();
        validate_identifier(&value, true, true)
            .map_err(|()| EgressError::InvalidEgressDestination)?;
        Ok(Self(value))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_text(&self) -> &str {
        &self.0
    }
}

/// Scope of one egress-level policy change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressLevelScope {
    /// Global default level.
    Default,
    /// Per-application override.
    Application(AppId),
    /// Per-content-type override.
    ContentType(ContentType),
}

/// Auditable description of one policy mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EgressPolicyChange {
    /// A level changed at one scope.
    Level {
        /// Scope whose level changed.
        scope: EgressLevelScope,
        /// Previous effective level at that scope.
        previous: EgressLevel,
        /// New level at that scope.
        current: EgressLevel,
    },
    /// An `egress_destination` was added to the allow-list.
    EgressDestinationAllowed {
        /// Newly allowed `egress_destination`.
        egress_destination: EgressDestinationId,
    },
}

impl EgressPolicyChange {
    /// Returns whether this change widens egress.
    #[must_use]
    pub const fn widens_egress(&self) -> bool {
        match self {
            Self::Level {
                previous, current, ..
            } => current.is_wider_than(*previous),
            Self::EgressDestinationAllowed { .. } => true,
        }
    }

    /// Returns whether the host must obtain explicit upgrade confirmation.
    #[must_use]
    pub const fn requires_upgrade_confirmation(&self) -> bool {
        self.widens_egress()
    }
}

/// Egress-policy configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EgressPolicy {
    default_level: EgressLevel,
    app_overrides: Vec<(AppId, EgressLevel)>,
    content_type_overrides: Vec<(ContentType, EgressLevel)>,
    allowed_egress_destinations: Vec<EgressDestinationId>,
}

impl EgressPolicy {
    /// Creates a policy with a bounded, duplicate-free configuration.
    ///
    /// # Errors
    /// Returns [`EgressError::TooManyAppOverrides`],
    /// [`EgressError::TooManyContentTypeOverrides`], or
    /// [`EgressError::TooManyAllowedEgressDestinations`] when a collection exceeds its
    /// hard bound. Duplicate keys return the corresponding `Duplicate*` error.
    pub fn new(
        default_level: EgressLevel,
        app_overrides: Vec<(AppId, EgressLevel)>,
        content_type_overrides: Vec<(ContentType, EgressLevel)>,
        allowed_egress_destinations: Vec<EgressDestinationId>,
    ) -> Result<Self, EgressError> {
        validate_app_overrides(&app_overrides)?;
        validate_content_type_overrides(&content_type_overrides)?;
        validate_allowed_egress_destinations(&allowed_egress_destinations)?;
        Ok(Self {
            default_level,
            app_overrides,
            content_type_overrides,
            allowed_egress_destinations,
        })
    }

    /// Returns the global default level.
    #[must_use]
    pub const fn default_level(&self) -> EgressLevel {
        self.default_level
    }

    /// Resolves the strictest level for an application and content type.
    #[must_use]
    pub fn effective_level(
        &self,
        app_id: Option<&AppId>,
        content_type: ContentType,
    ) -> EgressLevel {
        let mut effective = app_id
            .and_then(|app_id| {
                self.app_overrides
                    .iter()
                    .find(|(candidate, _)| candidate == app_id)
            })
            .map_or(self.default_level, |(_, level)| *level);
        if let Some((_, level)) = self
            .content_type_overrides
            .iter()
            .find(|(candidate, _)| candidate == &content_type)
        {
            effective = effective.strictest(*level);
        }
        effective
    }

    /// Evaluates one egress request without IO or provider selection.
    ///
    /// # Errors
    /// Returns [`EgressError::LocalModelUnavailable`] when `LocalOnly` is
    /// selected without a local model. Non-local egress returns
    /// [`EgressError::EgressDestinationNotAllowed`] when the `egress_destination`
    /// is not allow-listed.
    pub fn evaluate(&self, request: EgressRequest<'_>) -> Result<EgressDecision, EgressError> {
        let effective_level = self.effective_level(request.app_id, request.content_type);
        if effective_level == EgressLevel::LocalOnly {
            if !request.local_model_available {
                return Err(EgressError::LocalModelUnavailable);
            }
            return Ok(EgressDecision {
                effective_level,
                egress_destination: None,
                redaction_required: false,
            });
        }
        if !self
            .allowed_egress_destinations
            .contains(request.egress_destination)
        {
            return Err(EgressError::EgressDestinationNotAllowed);
        }
        Ok(EgressDecision {
            effective_level,
            egress_destination: Some(request.egress_destination.clone()),
            redaction_required: effective_level == EgressLevel::Redacted,
        })
    }

    /// Changes the global default and returns an auditable change record.
    pub const fn set_default_level(&mut self, level: EgressLevel) -> EgressPolicyChange {
        let previous = self.default_level;
        self.default_level = level;
        EgressPolicyChange::Level {
            scope: EgressLevelScope::Default,
            previous,
            current: level,
        }
    }

    /// Inserts or replaces one application override.
    ///
    /// # Errors
    /// Returns [`EgressError::TooManyAppOverrides`] when inserting a new key
    /// would exceed the hard bound.
    pub fn set_app_override(
        &mut self,
        app_id: AppId,
        level: EgressLevel,
    ) -> Result<EgressPolicyChange, EgressError> {
        if let Some(entry) = self
            .app_overrides
            .iter_mut()
            .find(|(candidate, _)| candidate == &app_id)
        {
            let previous = entry.1;
            entry.1 = level;
            return Ok(EgressPolicyChange::Level {
                scope: EgressLevelScope::Application(app_id),
                previous,
                current: level,
            });
        }
        if self.app_overrides.len() >= MAX_APP_OVERRIDES {
            return Err(EgressError::TooManyAppOverrides);
        }
        let previous = self.default_level;
        self.app_overrides.push((app_id.clone(), level));
        Ok(EgressPolicyChange::Level {
            scope: EgressLevelScope::Application(app_id),
            previous,
            current: level,
        })
    }

    /// Inserts or replaces one content-type override.
    ///
    /// # Errors
    /// This method does not exceed its bound because [`ContentType`] has a
    /// closed variant set; duplicate entries are rejected during construction.
    pub fn set_content_type_override(
        &mut self,
        content_type: ContentType,
        level: EgressLevel,
    ) -> EgressPolicyChange {
        if let Some(entry) = self
            .content_type_overrides
            .iter_mut()
            .find(|(candidate, _)| candidate == &content_type)
        {
            let previous = entry.1;
            entry.1 = level;
            return EgressPolicyChange::Level {
                scope: EgressLevelScope::ContentType(content_type),
                previous,
                current: level,
            };
        }
        let previous = self.default_level;
        self.content_type_overrides.push((content_type, level));
        EgressPolicyChange::Level {
            scope: EgressLevelScope::ContentType(content_type),
            previous,
            current: level,
        }
    }

    /// Adds one `egress_destination` to the allow-list.
    ///
    /// # Errors
    /// Returns [`EgressError::TooManyAllowedEgressDestinations`] when the allow-list has
    /// reached its hard bound. Returns `Ok(None)` when the `egress_destination` is already
    /// present, making the operation idempotent.
    pub fn allow_egress_destination(
        &mut self,
        egress_destination: EgressDestinationId,
    ) -> Result<Option<EgressPolicyChange>, EgressError> {
        if self
            .allowed_egress_destinations
            .contains(&egress_destination)
        {
            return Ok(None);
        }
        if self.allowed_egress_destinations.len() >= MAX_ALLOWED_EGRESS_DESTINATIONS {
            return Err(EgressError::TooManyAllowedEgressDestinations);
        }
        self.allowed_egress_destinations
            .push(egress_destination.clone());
        Ok(Some(EgressPolicyChange::EgressDestinationAllowed {
            egress_destination,
        }))
    }
}

/// One egress decision request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EgressRequest<'request> {
    /// Application identifier when the request is bound to an application.
    pub app_id: Option<&'request AppId>,
    /// Content category being routed.
    pub content_type: ContentType,
    /// Requested `egress_destination` for non-local egress.
    pub egress_destination: &'request EgressDestinationId,
    /// Whether a local model is actually available.
    pub local_model_available: bool,
}

/// Pure result of an egress-policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressDecision {
    effective_level: EgressLevel,
    egress_destination: Option<EgressDestinationId>,
    redaction_required: bool,
}

impl EgressDecision {
    /// Returns the resolved level.
    #[must_use]
    pub const fn effective_level(&self) -> EgressLevel {
        self.effective_level
    }

    /// Returns the `egress_destination` for non-local egress.
    #[must_use]
    pub const fn egress_destination(&self) -> Option<&EgressDestinationId> {
        self.egress_destination.as_ref()
    }

    /// Returns whether content must be redacted before egress.
    #[must_use]
    pub const fn redaction_required(&self) -> bool {
        self.redaction_required
    }
}

/// Explicit egress-policy error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EgressError {
    /// Application identifier is empty, too long, or malformed.
    InvalidAppId,
    /// Egress-destination identifier is empty, too long, or malformed.
    InvalidEgressDestination,
    /// Per-application override count exceeds the hard bound.
    TooManyAppOverrides,
    /// Per-content-type override count exceeds the closed variant count.
    TooManyContentTypeOverrides,
    /// Egress-destination allow-list count exceeds the hard bound.
    TooManyAllowedEgressDestinations,
    /// Application overrides contain a duplicate application.
    DuplicateAppOverride,
    /// Content-type overrides contain a duplicate category.
    DuplicateContentTypeOverride,
    /// Egress-destination allow-list contains a duplicate.
    DuplicateAllowedEgressDestination,
    /// `LocalOnly` was selected but no local model is available.
    LocalModelUnavailable,
    /// Non-local egress targeted an `egress_destination` outside the allow-list.
    EgressDestinationNotAllowed,
}

impl std::fmt::Display for EgressError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidAppId => "application id is malformed",
            Self::InvalidEgressDestination => "egress_destination id is malformed",
            Self::TooManyAppOverrides => "application override count exceeds the hard limit",
            Self::TooManyContentTypeOverrides => {
                "content-type override count exceeds the closed variant count"
            }
            Self::TooManyAllowedEgressDestinations => {
                "egress_destination allow-list exceeds the hard limit"
            }
            Self::DuplicateAppOverride => "application overrides contain a duplicate",
            Self::DuplicateContentTypeOverride => "content-type overrides contain a duplicate",
            Self::DuplicateAllowedEgressDestination => {
                "egress_destination allow-list contains a duplicate"
            }
            Self::LocalModelUnavailable => {
                "local_only requires an available local model; refusing silent failover"
            }
            Self::EgressDestinationNotAllowed => {
                "egress_destination is not on the egress allow-list"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EgressError {}

fn validate_identifier(
    value: &str,
    allow_path_characters: bool,
    allow_colon: bool,
) -> Result<(), ()> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES || value.trim() != value {
        return Err(());
    }
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(());
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(());
    }
    let valid = characters.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '.'
            || character == '-'
            || character == '_'
            || (allow_path_characters && character == '/')
            || (allow_colon && character == ':')
    });
    if valid { Ok(()) } else { Err(()) }
}

fn validate_app_overrides(overrides: &[(AppId, EgressLevel)]) -> Result<(), EgressError> {
    if overrides.len() > MAX_APP_OVERRIDES {
        return Err(EgressError::TooManyAppOverrides);
    }
    for (index, (app_id, _)) in overrides.iter().enumerate() {
        if overrides
            .iter()
            .skip(index.saturating_add(1))
            .any(|(candidate, _)| candidate == app_id)
        {
            return Err(EgressError::DuplicateAppOverride);
        }
    }
    Ok(())
}

fn validate_content_type_overrides(
    overrides: &[(ContentType, EgressLevel)],
) -> Result<(), EgressError> {
    if overrides.len() > CONTENT_TYPE_COUNT {
        return Err(EgressError::TooManyContentTypeOverrides);
    }
    for (index, (content_type, _)) in overrides.iter().enumerate() {
        if overrides
            .iter()
            .skip(index.saturating_add(1))
            .any(|(candidate, _)| candidate == content_type)
        {
            return Err(EgressError::DuplicateContentTypeOverride);
        }
    }
    Ok(())
}

fn validate_allowed_egress_destinations(
    egress_destinations: &[EgressDestinationId],
) -> Result<(), EgressError> {
    if egress_destinations.len() > MAX_ALLOWED_EGRESS_DESTINATIONS {
        return Err(EgressError::TooManyAllowedEgressDestinations);
    }
    for (index, egress_destination) in egress_destinations.iter().enumerate() {
        if egress_destinations
            .iter()
            .skip(index.saturating_add(1))
            .any(|candidate| candidate == egress_destination)
        {
            return Err(EgressError::DuplicateAllowedEgressDestination);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
