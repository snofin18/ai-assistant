//! Bounded JSON parser and encoder for loopback CDP messages.
//!
//! This module deliberately supports only the JSON data model required by CDP:
//! null, booleans, numbers, strings, arrays and string-keyed objects. It rejects
//! duplicate object keys, invalid UTF-8 escapes, excess nesting and trailing
//! input. The parser is private to the CDP transport so platform handles remain
//! non-serializable at the crate boundary.

use std::collections::BTreeMap;

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

const MAX_JSON_DEPTH: usize = 64;

/// One bounded JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// JSON number, retained as validated source text.
    Number(String),
    /// JSON string.
    String(String),
    /// JSON array.
    Array(Vec<Self>),
    /// JSON object.
    Object(BTreeMap<String, Self>),
}

impl JsonValue {
    /// Read an object field.
    #[must_use]
    pub fn get(&self, field: &str) -> Option<&Self> {
        self.as_object()?.get(field)
    }

    /// Read an object.
    #[must_use]
    pub const fn as_object(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    /// Read an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    /// Read a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Read a non-negative integer exactly representable by `u64`.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        let Self::Number(value) = self else {
            return None;
        };
        value.parse::<u64>().ok()
    }

    /// Read a signed integer exactly representable by `i64`.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        let Self::Number(value) = self else {
            return None;
        };
        value.parse::<i64>().ok()
    }
}

/// Parse one complete JSON document.
pub fn parse(input: &[u8]) -> PlatformResult<JsonValue> {
    let mut parser = Parser {
        input,
        position: 0,
        depth: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.position != input.len() {
        return Err(json_error("JSON has trailing non-whitespace input"));
    }
    Ok(value)
}

/// Encode one value as compact JSON.
pub fn encode(value: &JsonValue) -> String {
    let mut output = String::new();
    encode_into(value, &mut output);
    output
}

/// Construct a JSON object.
#[must_use]
pub fn object(entries: impl IntoIterator<Item = (impl Into<String>, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    )
}

/// Construct an empty JSON object.
#[must_use]
pub const fn empty_object() -> JsonValue {
    JsonValue::Object(BTreeMap::new())
}

/// Construct a JSON string.
#[must_use]
pub fn string(value: impl Into<String>) -> JsonValue {
    JsonValue::String(value.into())
}

/// Construct a JSON unsigned number.
#[must_use]
pub fn unsigned(value: u64) -> JsonValue {
    JsonValue::Number(value.to_string())
}

/// Construct a JSON boolean.
#[must_use]
pub const fn boolean(value: bool) -> JsonValue {
    JsonValue::Bool(value)
}

struct Parser<'input> {
    input: &'input [u8],
    position: usize,
    depth: usize,
}

impl Parser<'_> {
    fn parse_value(&mut self) -> PlatformResult<JsonValue> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(byte) => Err(json_error(format!(
                "JSON value starts with unexpected byte {byte:#04x}"
            ))),
            None => Err(json_error("JSON value is missing")),
        }
    }

    fn parse_array(&mut self) -> PlatformResult<JsonValue> {
        self.enter_container()?;
        self.consume_expected(b'[')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.peek() == Some(b']') {
            self.consume_expected(b']')?;
            self.depth = self.depth.saturating_sub(1);
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            match self.next() {
                Some(b',') => {
                    self.skip_whitespace();
                }
                Some(b']') => {
                    self.depth = self.depth.saturating_sub(1);
                    return Ok(JsonValue::Array(values));
                }
                _ => return Err(json_error("JSON array is missing `,` or `]`")),
            }
        }
    }

    fn parse_object(&mut self) -> PlatformResult<JsonValue> {
        self.enter_container()?;
        self.consume_expected(b'{')?;
        self.skip_whitespace();
        let mut values = BTreeMap::new();
        if self.peek() == Some(b'}') {
            self.consume_expected(b'}')?;
            self.depth = self.depth.saturating_sub(1);
            return Ok(JsonValue::Object(values));
        }
        loop {
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.consume_expected(b':')?;
            let value = self.parse_value()?;
            if values.insert(key.clone(), value).is_some() {
                return Err(json_error(format!("JSON object repeats key `{key}`")));
            }
            self.skip_whitespace();
            match self.next() {
                Some(b',') => {
                    self.skip_whitespace();
                }
                Some(b'}') => {
                    self.depth = self.depth.saturating_sub(1);
                    return Ok(JsonValue::Object(values));
                }
                _ => return Err(json_error("JSON object is missing `,` or `}`")),
            }
        }
    }

    fn parse_string(&mut self) -> PlatformResult<String> {
        self.consume_expected(b'"')?;
        let mut output = Vec::new();
        loop {
            let byte = self
                .next()
                .ok_or_else(|| json_error("JSON string is unterminated"))?;
            match byte {
                b'"' => {
                    return String::from_utf8(output)
                        .map_err(|_| json_error("JSON string contains invalid UTF-8"));
                }
                b'\\' => self.parse_escape(&mut output)?,
                0x00..=0x1f => {
                    return Err(json_error(
                        "JSON string contains an unescaped control character",
                    ));
                }
                other => output.push(other),
            }
        }
    }

    fn parse_escape(&mut self, output: &mut Vec<u8>) -> PlatformResult<()> {
        let escaped = self
            .next()
            .ok_or_else(|| json_error("JSON escape is incomplete"))?;
        match escaped {
            b'"' => output.push(b'"'),
            b'\\' => output.push(b'\\'),
            b'/' => output.push(b'/'),
            b'b' => output.push(0x08),
            b'f' => output.push(0x0c),
            b'n' => output.push(b'\n'),
            b'r' => output.push(b'\r'),
            b't' => output.push(b'\t'),
            b'u' => {
                let first = self.parse_hex_quad()?;
                let code_point = if (0xd800..=0xdbff).contains(&first) {
                    self.consume_expected(b'\\')?;
                    self.consume_expected(b'u')?;
                    let second = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(json_error("JSON high surrogate lacks a low surrogate"));
                    }
                    0x1_0000 + (u32::from(first - 0xd800) << 10) + u32::from(second - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(json_error(
                        "JSON low surrogate appears without a high surrogate",
                    ));
                } else {
                    u32::from(first)
                };
                let character = char::from_u32(code_point)
                    .ok_or_else(|| json_error("JSON unicode escape is not a scalar value"))?;
                let mut encoded = [0_u8; 4];
                output.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
            _ => return Err(json_error("JSON string contains an invalid escape")),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> PlatformResult<u16> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let byte = self
                .next()
                .ok_or_else(|| json_error("JSON unicode escape is truncated"))?;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a') + 10,
                b'A'..=b'F' => u16::from(byte - b'A') + 10,
                _ => return Err(json_error("JSON unicode escape contains a non-hex digit")),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> PlatformResult<JsonValue> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.next();
        }
        match self.peek() {
            Some(b'0') => {
                self.next();
                if self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    return Err(json_error("JSON number has a leading zero"));
                }
            }
            Some(b'1'..=b'9') => {
                self.consume_digits();
            }
            _ => return Err(json_error("JSON number has no integer digits")),
        }
        if self.peek() == Some(b'.') {
            self.next();
            if !self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                return Err(json_error("JSON fraction has no digits"));
            }
            self.consume_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.next();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.next();
            }
            if !self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                return Err(json_error("JSON exponent has no digits"));
            }
            self.consume_digits();
        }
        let raw = self
            .input
            .get(start..self.position)
            .ok_or_else(|| json_error("JSON number range is invalid"))?;
        let text = std::str::from_utf8(raw)
            .map_err(|_| json_error("JSON number is not ASCII"))?
            .to_string();
        if !text.parse::<f64>().is_ok_and(f64::is_finite) {
            return Err(json_error("JSON number is outside the finite range"));
        }
        Ok(JsonValue::Number(text))
    }

    fn consume_digits(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.next();
        }
    }

    fn expect_literal(&mut self, literal: &[u8]) -> PlatformResult<()> {
        for expected in literal {
            if self.next() != Some(*expected) {
                return Err(json_error("JSON literal is invalid"));
            }
        }
        Ok(())
    }

    fn consume_expected(&mut self, expected: u8) -> PlatformResult<()> {
        if self.next() == Some(expected) {
            return Ok(());
        }
        Err(json_error(format!("JSON expected byte {expected:#04x}")))
    }

    fn enter_container(&mut self) -> PlatformResult<()> {
        if self.depth >= MAX_JSON_DEPTH {
            return Err(json_error("JSON nesting exceeds 64 levels"));
        }
        self.depth = self.depth.saturating_add(1);
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.next();
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.position = self.position.saturating_add(1);
        Some(byte)
    }
}

fn encode_into(value: &JsonValue, output: &mut String) {
    match value {
        JsonValue::Null => output.push_str("null"),
        JsonValue::Bool(boolean) => output.push_str(if *boolean { "true" } else { "false" }),
        JsonValue::Number(number) => output.push_str(number),
        JsonValue::String(text) => encode_string(text, output),
        JsonValue::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                encode_into(value, output);
            }
            output.push(']');
        }
        JsonValue::Object(values) => {
            output.push('{');
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                encode_string(key, output);
                output.push(':');
                encode_into(value, output);
            }
            output.push('}');
        }
    }
}

fn encode_string(text: &str, output: &mut String) {
    output.push('"');
    for character in text.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if u32::from(character) < 0x20 => {
                output.push_str("\\u");
                let scalar = u32::from(character);
                for shift in [12_u32, 8, 4, 0] {
                    let nibble = ((scalar >> shift) & 0xf) as u8;
                    let encoded = match nibble {
                        0..=9 => b'0' + nibble,
                        _ => b'a' + (nibble - 10),
                    };
                    output.push(char::from(encoded));
                }
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn json_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(
        ErrorCode::Transient,
        format!("CDP JSON contract failed: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_round_trip_supports_cdp_shapes() -> PlatformResult<()> {
        let value = parse(
            br#"{"id":3,"method":"Page.navigate","params":{"url":"http://127.0.0.1/a","flag":false},"items":[null,1,-2,1.5,"\u4e2d"]}"#,
        )?;
        let encoded = encode(&value);
        let reparsed = parse(encoded.as_bytes())?;
        assert_eq!(reparsed.get("id").and_then(JsonValue::as_u64), Some(3));
        assert_eq!(
            reparsed
                .get("params")
                .and_then(|params| params.get("flag"))
                .and_then(|flag| match flag {
                    JsonValue::Bool(value) => Some(*value),
                    _ => None,
                }),
            Some(false)
        );
        Ok(())
    }

    #[test]
    fn test_json_rejects_duplicate_keys_and_trailing_input() {
        assert!(parse(br#"{"a":1,"a":2}"#).is_err());
        assert!(parse(br#"{"a":1} trailing"#).is_err());
    }
}
