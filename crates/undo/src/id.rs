//! Stable newtype identifiers used by rollback anchors and incidents.
//!
//! Responsibility: reject empty or NUL-containing identifiers before they reach a recipe or an
//! incident report.
//!
//! Boundary: this module does not generate identifiers. Clock, randomness, and UUID generation
//! stay outside this crate so rollback remains reproducible.

use crate::error::{UndoError, UndoResult};

fn validate_identifier(kind: &'static str, value: impl Into<String>) -> UndoResult<String> {
    let value = value.into();
    if value.is_empty() {
        return Err(UndoError::InvalidIdentifier {
            kind,
            reason: "must not be empty".to_owned(),
        });
    }
    if value.contains('\0') {
        return Err(UndoError::InvalidIdentifier {
            kind,
            reason: "must not contain NUL".to_owned(),
        });
    }
    Ok(value)
}

/// Identifier of one rollback anchor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorId(String);

impl AnchorId {
    /// Parses a non-empty anchor identifier.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidIdentifier`] when the value is empty or contains NUL.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        validate_identifier("anchor_id", value).map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identifier of the plan step whose effect may be rolled back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StepId(String);

impl StepId {
    /// Parses a non-empty step identifier.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidIdentifier`] when the value is empty or contains NUL.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        validate_identifier("step_id", value).map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identifier of the target whose state is captured by an anchor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetId(String);

impl TargetId {
    /// Parses a non-empty target identifier.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidIdentifier`] when the value is empty or contains NUL.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        validate_identifier("target_id", value).map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identifier of one incident report.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IncidentId(String);

impl IncidentId {
    /// Parses a non-empty incident identifier.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidIdentifier`] when the value is empty or contains NUL.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        validate_identifier("incident_id", value).map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
