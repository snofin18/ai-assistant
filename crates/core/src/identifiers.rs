//! Validated identifiers and token counters used by Core.

use std::fmt;

use crate::error::{CoreError, CoreResult};

const MAX_IDENTIFIER_BYTES: usize = 128;

fn validate_identifier(kind: &'static str, value: &str) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(CoreError::InvalidIdentifier {
            kind,
            value: value.to_string(),
        });
    }
    Ok(())
}

/// Stable identifier of one conversation session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// Creates a validated session identifier.
    ///
    /// The value must be 1..=128 ASCII letters, digits, `_`, `-`, or `.`.
    /// This constructor is pure and idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidIdentifier`] for an empty, oversized, or
    /// unsupported identifier.
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into();
        validate_identifier("session", &value)?;
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable identifier of one node in a session message tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(String);

impl MessageId {
    /// Creates a validated message identifier.
    ///
    /// The value uses the same syntax as [`SessionId`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidIdentifier`] for an empty, oversized, or
    /// unsupported identifier.
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into();
        validate_identifier("message", &value)?;
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Non-negative token count used by context budgeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TokenCount(u64);

impl TokenCount {
    /// Creates a token count.
    #[must_use]
    pub const fn new(tokens: u64) -> Self {
        Self(tokens)
    }

    /// Returns the raw token count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Adds two counts without wrapping.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NumericOverflow`] when the sum exceeds `u64::MAX`.
    pub fn checked_add(self, other: Self) -> CoreResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(CoreError::NumericOverflow {
                field: "token_count",
            })
    }
}
