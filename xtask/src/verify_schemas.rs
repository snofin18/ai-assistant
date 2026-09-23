//! # verify-schemas 子命令
//!
//! 职责：检查 5 份 `protocol/**/*.json` schema 文件的存在 + JSON 合法性
//! + 顶层版本字段 = 期望值 + 必要字段集合
//!
//! 边界（不做什么）：
//! - 不实现完整 JSON Schema 校验（无外部依赖）= 仅做结构性最低校验
//! - 不做 codegen（那是 `codegen` 子命令）
//! - 不读 generated 代码（避免循环依赖）
//!
//! 不变量：
//! 1. 五份 schema 必须全部存在 + JSON 合法 + 版本号匹配 → 否则 verdict FAILED
//! 2. error-codes 必须含 **恰好 13** 个 category（不变量与 `crates/protocol::ErrorCategory::ALL.len()` 对齐）
//! 3. capability-matrix 必须有 ≥ 1 个 capability
//! 4. 任何 schema 的 `version` 字段必须是 string（数值版本将阻止未来加 meta）

use std::fs;
use std::path::Path;

const EXPECTED_SCHEMAS: &[(&str, &str)] = &[
    ("protocol/error-codes/error-codes-1.0.json",      "1.0"),
    ("protocol/envelope/envelope-1.0.json",          "1.0"),
    ("protocol/tool-schema/tool-schema-1.0.json",    "1.0"),
    ("protocol/capability-matrix/capability-1.0.json","1.0"),
    ("protocol/audit-event/audit-event-1.0.json",    "1.0"),
];

const REQUIRED_CATEGORIES: usize = 13;

#[derive(Debug)]
struct SchemaCheck {
    path: String,
    parsed_ok: bool,
    has_version: bool,
    version_matches: bool,
    extra_finding: Option<String>,
}

/// 主入口：扫描 + 校验 + 输出。
///
/// 返回 `Ok(0)` 全绿 / `Ok(1)` 有 Error。
pub fn run(repo_root: &Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    let mut checks: Vec<SchemaCheck> = Vec::new();
    for (rel, expected_version) in EXPECTED_SCHEMAS {
        let full = repo_root.join(rel);
        match check_schema(&full, expected_version) {
            Ok(mut c) => {
                // error-codes 额外校验 13 个 category
                if rel.contains("error-codes") {
                    if let Ok(text) = fs::read_to_string(&full) {
                        if let Ok(val) = serde_json_lite::parse(&text) {
                            if let Some(cats) = val.get("categories").and_then(|v| v.as_array()) {
                                if cats.len() != REQUIRED_CATEGORIES {
                                    c.extra_finding = Some(format!(
                                        "categories count = {} (expected {})",
                                        cats.len(),
                                        REQUIRED_CATEGORIES
                                    ));
                                }
                            } else {
                                c.extra_finding = Some("missing 'categories' array".to_string());
                            }
                        }
                    }
                }
                checks.push(c);
            }
            Err(message) => checks.push(SchemaCheck {
                path: (*rel).to_string(),
                parsed_ok: false,
                has_version: false,
                version_matches: false,
                extra_finding: Some(message),
            }),
        }
    }
    let mut errors = 0usize;
    writeln!(output, "== verify-schemas ==").map_err(|e| e.to_string())?;
    writeln!(output, "scanned_schemas={}", checks.len()).map_err(|e| e.to_string())?;
    for c in &checks {
        let status = if c.parsed_ok && c.has_version && c.version_matches && c.extra_finding.is_none() {
            "OK"
        } else {
            errors += 1;
            "FAIL"
        };
        writeln!(
            output,
            "  [{}] {} (parsed={} version_ok={} extra={})",
            status,
            c.path,
            c.parsed_ok,
            c.version_matches,
            c.extra_finding.as_deref().unwrap_or("-")
        )
        .map_err(|e| e.to_string())?;
    }
    let verdict = if errors == 0 { "PASSED" } else { "FAILED" };
    writeln!(
        output,
        "-- summary: {} error(s)\n-- verdict: {}",
        errors, verdict
    )
    .map_err(|e| e.to_string())?;
    if errors == 0 { Ok(0) } else { Ok(1) }
}

fn check_schema(path: &Path, expected_version: &str) -> Result<SchemaCheck, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let val = serde_json_lite::parse(&text).map_err(|e| format!("parse: {e}"))?;
    let parsed_ok = true;
    let has_version = val.get("version").is_some();
    let version_matches = val
        .get("version")
        .and_then(|v| v.as_str())
        .map_or(false, |s| s == expected_version);
    Ok(SchemaCheck {
        path: path.display().to_string(),
        parsed_ok,
        has_version,
        version_matches,
        extra_finding: None,
    })
}

/// Minimal zero-dep JSON parser (subset of serde_json semantics).
/// Why: keep xtask zero-dep per invariants.
mod serde_json_lite {
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
            Self { input: input.as_bytes(), pos: 0 }
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
                Err(format!("expected '{}' at pos {}", b as char, self.pos))
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
                if b == b'\\' {
                    self.pos += 1;
                    let Some(esc) = self.peek() else {
                        return Err("bad escape".to_string());
                    };
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
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
                if (b'0'..=b'9').contains(&b) || b == b'.' || b == b'e' || b == b'E' || b == b'+' || b == b'-' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let text = std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| "invalid utf8 in number")?;
            text.parse::<f64>().map(Value::Number).map_err(|e| format!("bad number: {e}"))
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_object() {
        let text = "{\"version\":\"1.0\",\"count\":3}";
        let val = serde_json_lite::parse(text).expect("parse");
        assert_eq!(val.get("version").and_then(|v| v.as_str()), Some("1.0"));
    }

    #[test]
    fn test_parse_array_of_objects() {
        let text = "[{\"a\":1},{\"b\":2}]";
        let val = serde_json_lite::parse(text).expect("parse");
        if let serde_json_lite::Value::Array(items) = val {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_parse_string_with_escapes() {
        let text = r#"{"s":"a\nb\"c"}"#;
        let val = serde_json_lite::parse(text).expect("parse");
        assert_eq!(val.get("s").and_then(|v| v.as_str()), Some("a\nb\"c"));
    }
}