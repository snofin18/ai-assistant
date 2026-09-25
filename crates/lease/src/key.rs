//! Validated lease keys and owners.
//!
//! Responsibility: reject ambiguous strings before they can become manager state.
//! Boundary: this module does not parse platform handles or create target identifiers.

use std::fmt;

use crate::error::{LeaseError, LeaseResult};

const MAX_IDENTIFIER_BYTES: usize = 256;

/// Owner of one or more target leases.
///
/// The value is an agent-side stable identity such as a task or session identifier. It is never a
/// platform handle and is not persisted by this crate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeaseOwner(String);

impl LeaseOwner {
    /// Parses a lease owner.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidIdentifier`] when the value is empty, exceeds 256 bytes, or
    /// contains NUL.
    pub fn parse(value: impl Into<String>) -> LeaseResult<Self> {
        validate_identifier("lease_owner", value).map(Self)
    }

    /// Returns the owner as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LeaseOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Canonical key of a target lease.
///
/// The key always includes an application identifier and may narrow to a window, document, or a
/// named resource. The ordering is derived from the component order and is used by
/// [`crate::LeaseManager::acquire_many`] to avoid lock-order deadlocks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeaseKey {
    app_id: String,
    window_id: Option<String>,
    document_id: Option<String>,
    resource: Option<String>,
}

impl LeaseKey {
    /// Creates a lease key scoped to one application.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidIdentifier`] when `app_id` is empty, exceeds 256 bytes, or
    /// contains NUL.
    pub fn new(app_id: impl Into<String>) -> LeaseResult<Self> {
        Ok(Self {
            app_id: validate_identifier("app_id", app_id)?,
            window_id: None,
            document_id: None,
            resource: None,
        })
    }

    /// Narrows the key to a window.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidIdentifier`] when the value is invalid.
    pub fn with_window_id(mut self, window_id: impl Into<String>) -> LeaseResult<Self> {
        self.window_id = Some(validate_identifier("window_id", window_id)?);
        Ok(self)
    }

    /// Narrows the key to a document.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidIdentifier`] when the value is invalid.
    pub fn with_document_id(mut self, document_id: impl Into<String>) -> LeaseResult<Self> {
        self.document_id = Some(validate_identifier("document_id", document_id)?);
        Ok(self)
    }

    /// Narrows the key to a named resource inside a target.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::InvalidIdentifier`] when the value is invalid.
    pub fn with_resource(mut self, resource: impl Into<String>) -> LeaseResult<Self> {
        self.resource = Some(validate_identifier("resource", resource)?);
        Ok(self)
    }

    /// Returns the application identifier.
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Returns the optional window identifier.
    #[must_use]
    pub fn window_id(&self) -> Option<&str> {
        self.window_id.as_deref()
    }

    /// Returns the optional document identifier.
    #[must_use]
    pub fn document_id(&self) -> Option<&str> {
        self.document_id.as_deref()
    }

    /// Returns the optional resource name.
    #[must_use]
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }
}

impl fmt::Display for LeaseKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "app={}", self.app_id)?;
        write_optional_component(formatter, "window", self.window_id.as_deref())?;
        write_optional_component(formatter, "document", self.document_id.as_deref())?;
        write_optional_component(formatter, "resource", self.resource.as_deref())
    }
}

fn write_optional_component(
    formatter: &mut fmt::Formatter<'_>,
    label: &str,
    value: Option<&str>,
) -> fmt::Result {
    if let Some(value) = value {
        write!(formatter, " {label}={value}")?;
    }
    Ok(())
}

fn validate_identifier(kind: &'static str, value: impl Into<String>) -> LeaseResult<String> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(LeaseError::InvalidIdentifier {
            kind,
            reason: "must not be empty or whitespace-only".to_owned(),
        });
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(LeaseError::InvalidIdentifier {
            kind,
            reason: format!("must be at most {MAX_IDENTIFIER_BYTES} bytes"),
        });
    }
    if value.contains('\0') {
        return Err(LeaseError::InvalidIdentifier {
            kind,
            reason: "must not contain NUL".to_owned(),
        });
    }
    Ok(value)
}
