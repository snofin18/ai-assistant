//! Validated identifiers and integer units used by the model gateway.
//!
//! All values reject empty strings, NUL bytes, and non-printable ASCII before they enter routing
//! or provider state. Time and cost values use newtypes so callers cannot silently mix units.

use std::fmt;

use crate::error::{ModelGatewayError, ModelResult};

/// A stable model identifier selected by the router and registered by a provider.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelId(String);

impl ModelId {
    /// Creates a validated model identifier.
    ///
    /// Returns `InvalidRequest` when the value is empty, contains NUL, or contains a control
    /// character. This constructor has no side effects and is idempotent.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when `value` is empty or contains control characters.
    pub fn new(value: impl Into<String>) -> ModelResult<Self> {
        let value = value.into();
        validate_identifier("model_id", &value)?;
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A stable identifier for one routing rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(String);

impl RuleId {
    /// Creates a validated routing-rule identifier.
    ///
    /// Returns `InvalidRequest` for empty or control-containing values.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when `value` is empty or contains control characters.
    pub fn new(value: impl Into<String>) -> ModelResult<Self> {
        let value = value.into();
        validate_identifier("rule_id", &value)?;
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A caller-supplied stable key for provider-side prompt caching.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey(String);

impl CacheKey {
    /// Creates a validated prompt-cache key.
    ///
    /// Returns `InvalidRequest` for empty or control-containing values. Keys should identify a
    /// stable prefix and must not contain secrets or user content.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when `value` is empty or contains control characters.
    pub fn new(value: impl Into<String>) -> ModelResult<Self> {
        let value = value.into();
        validate_identifier("cache_key", &value)?;
        Ok(Self(value))
    }

    /// Returns the key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn validate_identifier(field: &'static str, value: &str) -> ModelResult<()> {
    if value.is_empty() {
        return Err(ModelGatewayError::InvalidRequest {
            field,
            reason: "must not be empty".to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ModelGatewayError::InvalidRequest {
            field,
            reason: "must not contain control characters".to_string(),
        });
    }
    Ok(())
}

/// Non-negative milliseconds used for timeouts, retry delays, and latency records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DurationMs(u64);

impl DurationMs {
    /// Creates a duration.
    #[must_use]
    pub const fn new(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns whether the duration is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// A non-negative count of model tokens.
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

    /// Adds two counts, returning `NumericOverflow` instead of wrapping.
    ///
    /// # Errors
    ///
    /// Returns `NumericOverflow` when the sum exceeds `u64::MAX`.
    pub fn checked_add(self, other: Self) -> ModelResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ModelGatewayError::NumericOverflow {
                field: "token_count",
            })
    }
}

/// An integer cost in millionths of one US dollar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CostMicroUsd(u64);

impl CostMicroUsd {
    /// Creates a cost value.
    #[must_use]
    pub const fn new(micro_usd: u64) -> Self {
        Self(micro_usd)
    }

    /// Returns the raw micro-USD value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Adds two costs, returning `NumericOverflow` instead of wrapping.
    ///
    /// # Errors
    ///
    /// Returns `NumericOverflow` when the sum exceeds `u64::MAX`.
    pub fn checked_add(self, other: Self) -> ModelResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ModelGatewayError::NumericOverflow {
                field: "cost_micro_usd",
            })
    }
}

/// Data sensitivity used by routing and DLP-aware provider selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Sensitivity {
    /// Public or already-redacted content.
    #[default]
    Public,
    /// Internal content that may use a redacted provider path.
    Internal,
    /// Sensitive content that must remain on a private/local route.
    Sensitive,
}

/// Logical model tier from architecture v2 section 11.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskStage {
    /// Conversation, clarification, and simple responses.
    Chat,
    /// Tool selection and argument filling.
    ToolSelect,
    /// Complex task decomposition and planning.
    Plan,
    /// Screenshot understanding or visual grounding.
    Vision,
    /// Postcondition and clean-context verification.
    Verify,
    /// Sensitive-data processing on a private route.
    Private,
}
