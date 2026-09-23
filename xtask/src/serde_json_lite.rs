//! Minimal zero-dep JSON parser (subset of serde_json semantics).
//! Used by both `verify-schemas` and `codegen` to avoid adding serde_json to xtask
//! workspace dep tree (per workspace policy ADR-0021).
//!
//! Supports: objects, arrays, strings (with escapes), numbers, booleans, null.
//! Does NOT support: scientific notation edge cases, comments, streaming.
//! Index/slice ops are bounds-checked in while/if; per ADR-0035 only allow what we need.
#![allow(
    clippy::indexing_slicing,
    clippy::manual_is_ascii_check,
    dead_code,
    clippy::doc_markdown,
    clippy::manual_range_contains,
    clippy::collapsible_if,
    clippy::unnecessary_map_or,
    clippy::missing_const_for_fn,
    clippy::nonminimal_bool,
    clippy::unnecessary_operation,
    clippy::uninlined_format_args,
    clippy::module_name_repetitions,
    clippy::use_self,
    clippy::if_not_else,
    clippy::option_if_let_else,
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::ptr_arg
)] // pedantic allow list (per ADR-0035)

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

/// UTF-8 前导字节 → 该字符的字节宽度（1~4）。
///
/// 非法前导字节（或孤立续字节）一律返回 1，让紧随其后的 `str::from_utf8` 去报出真正的
/// 错误 —— 这里不猜、不兜底（铁律 1：要么成功，要么带原因失败）。
const fn utf8_char_width(leading_byte: u8) -> usize {
    if leading_byte < 0x80 {
        1
    } else if leading_byte < 0xE0 {
        2
    } else if leading_byte < 0xF0 {
        3
    } else {
        4
    }
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
            } else if b < 0x80 {
                // ASCII 快路径：单字节直接入串。
                out.push(b as char);
                self.pos += 1;
            } else {
                // 非 ASCII：必须按 UTF-8 **整体**解码这个字符。
                //
                // 为什么不能沿用 `out.push(b as char)`：那会把多字节序列逐字节按 Latin-1
                // 解释（`中文` → `ä¸æ`），而 schema 的 `message_for_model` / `hint` 允许中文。
                // 后果是**静默**写坏生成物 —— `codegen` 仍 exit 0，`codegen --check` 也发现不了
                //（它拿同一个渲染器比对，两边一样"错"）。所以这里宁可报错也不能猜。
                let width = utf8_char_width(b);
                let end = self.pos + width;
                let slice = self
                    .input
                    .get(self.pos..end)
                    .ok_or_else(|| "truncated utf-8 sequence in string".to_string())?;
                let text = std::str::from_utf8(slice)
                    .map_err(|_| "invalid utf-8 sequence in string".to_string())?;
                let character = text
                    .chars()
                    .next()
                    .ok_or_else(|| "empty utf-8 sequence in string".to_string())?;
                out.push(character);
                self.pos = end;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// 取 `Value::Number` 的 f64（测试专用；避免直接比较 f64 触发 `clippy::float_cmp`）。
    fn as_number(value: &Value) -> Option<f64> {
        if let Value::Number(number) = value {
            Some(*number)
        } else {
            None
        }
    }

    /// 回归测试（2026-09-23 复核发现的静默缺陷）：非 ASCII 必须**原样**保留。
    ///
    /// 旧实现 `out.push(b as char)` 把多字节序列按 Latin-1 逐字节拆开（`中文` → `ä¸æ`），
    /// 而 `xtask codegen` 仍 exit 0 —— 这是"静默写坏生成物"，比报错危险得多。
    #[test]
    fn test_parse_string_preserves_non_ascii() {
        let source = "{\"message_for_model\": \"中文 § — emoji 🚀\"}";
        let value = parse(source).expect("合法 JSON 必须解析成功");
        assert_eq!(
            value.get("message_for_model").and_then(Value::as_str),
            Some("中文 § — emoji 🚀"),
            "多字节 UTF-8 必须整体解码，不得逐字节按 Latin-1 拆开"
        );
    }

    /// 非 ASCII 出现在**对象键**里同样不能坏（键也走 `parse_string`）。
    #[test]
    fn test_parse_object_key_preserves_non_ascii() {
        let value = parse("{\"键\": 1}").expect("合法 JSON 必须解析成功");
        assert!(value.get("键").is_some(), "非 ASCII 键必须可查");
    }

    /// 已知限制（**故意锁住行为**）：反斜杠 `u` 转义不支持，但必须**显式报错**而不是静默写错。
    #[test]
    fn test_parse_string_rejects_unicode_escape_loudly() {
        let source = "\"\\u00a7\"";
        let error = parse(source).expect_err("反斜杠-u 转义当前不支持，必须报错");
        assert!(
            error.contains("unknown escape"),
            "错误信息必须点明不支持的转义，实际：{error}"
        );
    }

    /// 转义与容器：`\n` / `\"` 混在数组与对象里，数字不得被当成字符串。
    #[test]
    fn test_parse_escapes_and_containers() {
        let source = r#"{"a":[1,true,null,"x\n\"y\""],"b":{"c":1.5e2}}"#;
        let value = parse(source).expect("合法 JSON 必须解析成功");
        let array = value.get("a").and_then(Value::as_array).expect("a 是数组");
        assert_eq!(array.len(), 4);
        assert_eq!(array.get(1).and_then(Value::as_bool), Some(true));
        assert!(
            matches!(array.get(2), Some(Value::Null)),
            "null 必须解析成 Value::Null"
        );
        assert_eq!(array.get(3).and_then(Value::as_str), Some("x\n\"y\""));
        let nested = value.get("b").and_then(|b| b.get("c"));
        assert!(
            matches!(nested, Some(Value::Number(_))),
            "1.5e2 必须解析成数字"
        );
        assert!(nested.and_then(as_number).is_some(), "数字必须可取");
    }

    /// 反斜杠转义 `\\` 与斜杠转义 `\/`。
    #[test]
    fn test_parse_string_backslash_escape() {
        let source = r#""a\\b\/c""#;
        let value = parse(source).expect("合法 JSON 必须解析成功");
        assert_eq!(value.as_str(), Some("a\\b/c"));
    }

    /// 尾部垃圾必须被拒绝（否则 `parse` 会把半份文件当成功）。
    #[test]
    fn test_parse_rejects_trailing_garbage() {
        assert!(parse("{} extra").is_err(), "尾随字符必须报错");
    }

    /// 未闭合字符串必须被拒绝，而不是返回半截内容。
    #[test]
    fn test_parse_rejects_unterminated_string() {
        assert!(parse("\"abc").is_err(), "未闭合字符串必须报错");
    }

    /// 未闭合字符串末尾是非 ASCII 时同样必须报错（不得静默返回半截内容）。
    #[test]
    fn test_parse_rejects_unterminated_non_ascii_string() {
        assert!(parse("\"中").is_err(), "非 ASCII 未闭合字符串必须报错");
    }

    /// 控制字符（未转义）必须被拒绝。
    #[test]
    fn test_parse_rejects_raw_control_char() {
        let source = format!("\"{}\"", '\u{1}');
        assert!(parse(&source).is_err(), "裸控制字符必须报错");
    }
}
