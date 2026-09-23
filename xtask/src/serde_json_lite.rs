//! Minimal zero-dep JSON parser (subset of serde_json semantics).
//! Used by both `verify-schemas` and `codegen` to avoid adding serde_json to xtask
//! workspace dep tree (per workspace policy ADR-0021).
//!
//! Supports: objects, arrays, strings (with escapes), numbers, booleans, null.
//! Does NOT support: scientific notation edge cases, comments, streaming.
//! Index/slice ops are bounds-checked in while/if; per ADR-0035 only allow what we need.
#![allow(clippy::indexing_slicing, dead_code, clippy::doc_markdown, clippy::manual_range_contains, clippy::collapsible_if, clippy::unnecessary_map_or, clippy::missing_const_for_fn, clippy::nonminimal_bool, clippy::unnecessary_operation, clippy::uninlined_format_args, clippy::module_name_repetitions, clippy::use_self, clippy::if_not_else, clippy::option_if_let_else, clippy::needless_pass_by_value, clippy::similar_names, clippy::missing_errors_doc, clippy::missing_panics_doc, clippy::ptr_arg)]  // pedantic allow list (per ADR-0035)

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        if let Value::Object(map) = self {
            map.get(key)
        } else {
            None
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Value::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        if let Value::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        if let Value::Object(map) = self {
            Some(map)
        } else {
            None
        }
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos < parser.input.len() {
        return Err(format!("trailing chars at pos {}", parser.pos));
    }
    Ok(value)
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }
    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            let b = self.input[self.pos];
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }
    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected '\''{}'\'' at pos {}",
                b as char, self.pos
            ))
        }
    }
    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        let Some(b) = self.peek() else {
            return Err("unexpected EOF".to_string());
        };
        match b {
            b'n' => self.parse_null(),
            b't' | b'f' => self.parse_bool(),
            b'"' => self.parse_string().map(Value::String),
            b'[' => self.parse_array(),
            b'{' => self.parse_object(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(format!("unexpected byte {} at pos {}", b as char, self.pos)),
        }
    }
    fn parse_null(&mut self) -> Result<Value, String> {
        if self.input[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(Value::Null)
        } else {
            Err("invalid null".to_string())
        }
    }
    fn parse_bool(&mut self) -> Result<Value, String> {
        if self.input[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(Value::Bool(true))
        } else if self.input[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(Value::Bool(false))
        } else {
            Err("invalid bool".to_string())
        }
    }
    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(b) = self.peek() else {
                return Err("unterminated string".to_string());
            };
            if b == b'"' {
                self.pos += 1;
                return Ok(out);
            }
            if b == 92 {
                // backslash
                self.pos += 1;
                let Some(esc) = self.peek() else {
                    return Err("bad escape".to_string());
                };
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    92 => out.push(92 as char),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => out.push('\r'),
                    b'b' => out.push('\u{08}'),
                    b'f' => out.push('\u{0C}'),
                    _ => return Err(format!("unknown escape \\{}", esc as char)),
                }
            } else if b < 0x20 {
                return Err("control char in string".to_string());
            } else {
                out.push(b as char);
                self.pos += 1;
            }
        }
    }
    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if (b'0'..=b'9').contains(&b)
                || b == b'.'
                || b == b'e'
                || b == b'E'
                || b == b'+'
                || b == b'-'
            {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| "invalid utf8 in number")?;
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|e| format!("bad number: {e}"))
    }
    fn parse_array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err("expected ',' or ']' in array".to_string()),
            }
        }
    }
    fn parse_object(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut map = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err("expected ',' or '}' in object".to_string()),
            }
        }
    }
}
