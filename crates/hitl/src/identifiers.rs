//! Validated identifiers shared by approval and authorization types.

use std::fmt;

use crate::error::{HitlError, HitlResult};

const MAX_IDENTIFIER_BYTES: usize = 256;

macro_rules! identifier_type {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("Validated ", $kind, " identifier.")]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Creates and validates an identifier.
            ///
            /// # Errors
            ///
            /// Returns [`HitlError::InvalidIdentifier`] when the value is empty,
            /// exceeds 256 bytes, or contains control characters.
            pub fn new(value: impl Into<String>) -> HitlResult<Self> {
                let value = value.into();
                validate_identifier($kind, &value)?;
                Ok(Self(value))
            }

            /// Returns the identifier as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the identifier and returns its text.
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

identifier_type!(ApprovalRequestId, "approval request");
identifier_type!(SubjectId, "subject");
identifier_type!(AppSessionId, "application session");

fn validate_identifier(kind: &'static str, value: &str) -> HitlResult<()> {
    if value.is_empty() {
        return Err(HitlError::InvalidIdentifier {
            kind,
            reason: "value must not be empty".to_owned(),
        });
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(HitlError::InvalidIdentifier {
            kind,
            reason: format!("value exceeds {MAX_IDENTIFIER_BYTES} bytes"),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(HitlError::InvalidIdentifier {
            kind,
            reason: "value contains a control character".to_owned(),
        });
    }
    Ok(())
}
