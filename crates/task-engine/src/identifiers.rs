//! Strongly typed task, plan, and step identifiers.
//!
//! The engine rejects empty, oversized, or syntactically unsafe identifiers
//! before they reach storage or diagnostics.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{TaskEngineError, TaskEngineResult};

const MAX_IDENTIFIER_BYTES: usize = 128;

macro_rules! identifier_type {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("Validated ", $kind, " identifier.")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Creates a validated ", $kind, " identifier.")]
            ///
            /// # Errors
            ///
            /// Returns [`TaskEngineError::InvalidIdentifier`] when the value is
            /// empty, longer than 128 bytes, or contains a character outside
            /// ASCII letters, digits, `_`, or `-`.
            pub fn new(value: impl Into<String>) -> TaskEngineResult<Self> {
                let value = value.into();
                validate_identifier($kind, &value)?;
                Ok(Self(value))
            }

            /// Returns the identifier as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Returns the identifier as owned text.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier_type!(TaskId, "task");
identifier_type!(StepId, "step");
identifier_type!(PlanId, "plan");

fn validate_identifier(kind: &'static str, value: &str) -> TaskEngineResult<()> {
    let is_valid = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if is_valid {
        return Ok(());
    }
    Err(TaskEngineError::InvalidIdentifier {
        kind,
        value: value.to_owned(),
    })
}
