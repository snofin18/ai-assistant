//! Conservative static validation for the supported regular-expression subset.

use crate::{PolicyError, PolicyResult};
use std::iter::Peekable;
use std::str::Bytes;

const MAX_PATTERN_BYTES: usize = 256;
const MAX_GROUP_DEPTH: usize = 8;

/// A regular expression accepted by the static complexity validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegularExpression {
    pattern: String,
}

impl ValidatedRegularExpression {
    /// Returns the accepted pattern.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.pattern
    }
}

/// Validates a conservative regular-expression subset to reduce `ReDoS` risk.
///
/// The validator rejects lookaround, backreferences, brace quantifiers, nested
/// quantified groups, overlapping quantified alternation, and repeated wildcard
/// quantifiers. Unsupported syntax is rejected rather than passed through.
///
/// # Errors
///
/// Returns [`PolicyError::RegexRejected`] for unsupported or potentially
/// exponential syntax.
pub fn validate_regular_expression(pattern: &str) -> PolicyResult<ValidatedRegularExpression> {
    if pattern.is_empty() || pattern.len() > MAX_PATTERN_BYTES {
        return Err(reject("pattern is empty or exceeds the size limit"));
    }
    if !pattern.is_ascii() || pattern.chars().any(char::is_control) {
        return Err(reject("pattern must contain printable ASCII only"));
    }
    RegexScanner::default().scan(pattern)?;
    Ok(ValidatedRegularExpression {
        pattern: pattern.to_owned(),
    })
}

#[derive(Debug, Default)]
struct RegexScanner {
    groups: Vec<GroupState>,
    in_character_class: bool,
    last_token: Option<u8>,
    last_was_quantifier: bool,
    wildcard_quantifiers: usize,
    unbounded_quantifiers: usize,
}

impl RegexScanner {
    fn scan(mut self, pattern: &str) -> PolicyResult<()> {
        let mut characters = pattern.bytes().peekable();
        while let Some(character) = characters.next() {
            if self.handle_escape(character, &mut characters)? {
                continue;
            }
            if self.handle_character_class(character)? {
                continue;
            }
            self.handle_token(character, &mut characters)?;
        }
        self.finish()
    }

    fn handle_escape(
        &mut self,
        character: u8,
        characters: &mut Peekable<Bytes<'_>>,
    ) -> PolicyResult<bool> {
        if character != b'\\' {
            return Ok(false);
        }
        let escaped = characters
            .next()
            .ok_or_else(|| reject("pattern ends with an incomplete escape"))?;
        if self.in_character_class {
            validate_class_escape(escaped)?;
        } else {
            validate_escape(escaped)?;
        }
        self.record_plain_token(escaped);
        Ok(true)
    }

    fn handle_character_class(&mut self, character: u8) -> PolicyResult<bool> {
        if character == b'[' && !self.in_character_class {
            self.in_character_class = true;
            self.record_plain_token(character);
            return Ok(true);
        }
        if character == b']' && self.in_character_class {
            self.in_character_class = false;
            self.record_plain_token(character);
            return Ok(true);
        }
        if self.in_character_class {
            if character == b'[' {
                return Err(reject("nested character classes are unsupported"));
            }
            self.record_plain_token(character);
            return Ok(true);
        }
        Ok(false)
    }

    fn handle_token(
        &mut self,
        character: u8,
        characters: &mut Peekable<Bytes<'_>>,
    ) -> PolicyResult<()> {
        match character {
            b'(' => self.open_group(characters)?,
            b')' => self.close_group(characters)?,
            b'|' => {
                if let Some(group) = self.groups.last_mut() {
                    group.has_alternation = true;
                }
            }
            b'{' | b'}' => {
                return Err(reject("brace quantifiers are unsupported"));
            }
            b'*' | b'+' | b'?' => return self.handle_quantifier(character),
            _ => {}
        }
        self.record_plain_token(character);
        Ok(())
    }

    fn open_group(&mut self, characters: &mut Peekable<Bytes<'_>>) -> PolicyResult<()> {
        if characters.peek() == Some(&b'?') {
            return Err(reject("lookaround and special groups are unsupported"));
        }
        if self.groups.len() >= MAX_GROUP_DEPTH {
            return Err(reject("group nesting exceeds the supported depth"));
        }
        self.groups.push(GroupState::default());
        Ok(())
    }

    fn close_group(&mut self, characters: &mut Peekable<Bytes<'_>>) -> PolicyResult<()> {
        let group = self
            .groups
            .pop()
            .ok_or_else(|| reject("pattern has an unmatched ')'"))?;
        if characters.peek().is_some_and(|next| is_quantifier(*next))
            && (group.has_quantifier || group.has_alternation)
        {
            return Err(reject(
                "quantified groups may not contain quantifiers or alternation",
            ));
        }
        Ok(())
    }

    fn handle_quantifier(&mut self, character: u8) -> PolicyResult<()> {
        if self.last_token.is_none()
            || self.last_was_quantifier
            || matches!(self.last_token, Some(b'(' | b'|'))
        {
            return Err(reject(
                "pattern contains a misplaced or repeated quantifier",
            ));
        }
        if self.last_token == Some(b'.') && matches!(character, b'*' | b'+') {
            self.wildcard_quantifiers = self.wildcard_quantifiers.saturating_add(1);
        }
        if matches!(character, b'*' | b'+') {
            self.unbounded_quantifiers = self.unbounded_quantifiers.saturating_add(1);
            if self.unbounded_quantifiers > 4 {
                return Err(reject("too many unbounded quantifiers"));
            }
        }
        if let Some(group) = self.groups.last_mut() {
            group.has_quantifier = true;
        }
        self.last_was_quantifier = true;
        Ok(())
    }

    const fn record_plain_token(&mut self, character: u8) {
        self.last_token = Some(character);
        self.last_was_quantifier = false;
    }

    fn finish(&self) -> PolicyResult<()> {
        if self.in_character_class {
            return Err(reject("pattern has an unterminated character class"));
        }
        if !self.groups.is_empty() {
            return Err(reject("pattern has an unterminated group"));
        }
        if self.wildcard_quantifiers > 1 {
            return Err(reject("repeated wildcard quantifiers are unsupported"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct GroupState {
    has_quantifier: bool,
    has_alternation: bool,
}

fn validate_escape(value: u8) -> PolicyResult<()> {
    if value.is_ascii_alphanumeric() {
        if matches!(value, b'd' | b'D' | b's' | b'S' | b'w' | b'W' | b'b' | b'B') {
            return Ok(());
        }
        return Err(reject(
            "backreferences and unsupported escapes are forbidden",
        ));
    }
    if is_escaped_literal(value) {
        Ok(())
    } else {
        Err(reject("escape sequence is unsupported"))
    }
}

fn validate_class_escape(value: u8) -> PolicyResult<()> {
    if matches!(value, b'd' | b'D' | b's' | b'S' | b'w' | b'W') || is_escaped_literal(value) {
        Ok(())
    } else {
        Err(reject("unsupported escape inside character class"))
    }
}

const fn is_escaped_literal(value: u8) -> bool {
    matches!(
        value,
        b'.' | b'^'
            | b'$'
            | b'*'
            | b'+'
            | b'?'
            | b'{'
            | b'}'
            | b'['
            | b']'
            | b'('
            | b')'
            | b'|'
            | b'/'
            | b'-'
            | b'\\'
    )
}

const fn is_quantifier(value: u8) -> bool {
    matches!(value, b'*' | b'+' | b'?')
}

fn reject(reason: &str) -> PolicyError {
    PolicyError::RegexRejected {
        reason: reason.to_owned(),
    }
}
