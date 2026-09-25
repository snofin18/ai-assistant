//! Pure text length, line-count, and control-character validation.

use crate::{PolicyError, PolicyResult};

/// Maximum accepted text dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLimits {
    /// Maximum Unicode scalar values.
    pub max_characters: usize,
    /// Maximum UTF-8 bytes.
    pub max_bytes: usize,
    /// Maximum logical lines.
    pub max_lines: usize,
}

/// Validates text against explicit limits.
///
/// Carriage returns, NUL bytes, and control characters other than tab and
/// newline are rejected to keep downstream normalization deterministic.
///
/// # Errors
///
/// Returns [`PolicyError::TextRejected`] when a limit is exceeded or the text
/// contains an unsupported control character. Invalid limits return
/// [`PolicyError::InvalidRuleSet`].
pub fn validate_text_limits(text: &str, limits: TextLimits) -> PolicyResult<()> {
    if limits.max_characters == 0 || limits.max_bytes == 0 || limits.max_lines == 0 {
        return Err(PolicyError::InvalidRuleSet {
            reason: "text limits must all be greater than zero".to_owned(),
        });
    }
    if text.len() > limits.max_bytes {
        return Err(reject(&format!(
            "text is {} bytes, limit is {}",
            text.len(),
            limits.max_bytes
        )));
    }
    if text.chars().count() > limits.max_characters {
        return Err(reject(&format!(
            "text has more than {} characters",
            limits.max_characters
        )));
    }
    if text.contains('\r') {
        return Err(reject("text contains a carriage return; use LF only"));
    }
    if text
        .chars()
        .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(reject("text contains a forbidden control character"));
    }
    let lines = if text.is_empty() {
        0
    } else {
        text.lines().count()
    };
    if lines > limits.max_lines {
        return Err(reject(&format!(
            "text has {lines} lines, limit is {}",
            limits.max_lines
        )));
    }
    Ok(())
}

fn reject(reason: &str) -> PolicyError {
    PolicyError::TextRejected {
        reason: reason.to_owned(),
    }
}
