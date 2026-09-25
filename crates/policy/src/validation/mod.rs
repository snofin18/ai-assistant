//! Pure validators for untrusted tool parameters.

mod path;
mod regex;
mod text;
mod url;

pub use path::{PathValidationRequest, ValidatedPath, validate_path};
pub use regex::{ValidatedRegularExpression, validate_regular_expression};
pub use text::{TextLimits, validate_text_limits};
pub use url::{UrlValidationRequest, ValidatedUrl, validate_url};

use crate::{PolicyError, PolicyResult};

/// Validates an inclusive integer range and returns the accepted value.
///
/// # Errors
///
/// Returns [`PolicyError::InvalidRuleSet`] for an inverted range and
/// [`PolicyError::IntegerOutOfRange`] when the value is outside the bounds.
pub fn validate_integer_range(value: i64, minimum: i64, maximum: i64) -> PolicyResult<i64> {
    if minimum > maximum {
        return Err(PolicyError::InvalidRuleSet {
            reason: "integer range minimum is greater than maximum".to_owned(),
        });
    }
    if value < minimum || value > maximum {
        return Err(PolicyError::IntegerOutOfRange {
            value,
            minimum,
            maximum,
        });
    }
    Ok(value)
}
