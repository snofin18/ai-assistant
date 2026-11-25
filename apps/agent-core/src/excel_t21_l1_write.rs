//! Test-only Excel T2.2 scalar write input gate.
//!
//! Responsibilities:
//! - parse one bounded JSON scalar from its original JSON text into the existing cell vocabulary;
//! - reject formula-like text before it can reach an Excel write;
//! - prepare exactly one write against an existing L1 anchor and pre-fingerprint.
//!
//! Boundaries:
//! - no workbook, COM object, filesystem shadow copy, write, save, or restore is touched;
//! - this module is test-only and is not reachable from production assembly or `ToolBus`.
//!
//! Invariants:
//! 1. arrays and objects never degrade into text or a partial cell value;
//! 2. numbers are canonicalized without floating-point conversion;
//! 3. scalar validation completes before the anchor's one-use write preparation changes state.
//!
//! Related documents: ADR-0062, ADR-0107, Stage 2 batch 6.
use super::super::{CanonicalDecimal, CellValueType, MAX_RANGE_PAYLOAD_BYTES, invalid_arguments};
use super::{ExcelL1AnchorHandle, ExcelL1AnchorStore, ExcelL1WorkbookIdentity};
use assistant_protocol::ErrorCode;

/// One validated scalar literal ready to be submitted to a future Excel write port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExcelL1ScalarLiteral {
    value_type: CellValueType,
    value_text: String,
}

impl ExcelL1ScalarLiteral {
    /// Returns the closed Excel cell type produced by the JSON scalar.
    pub(super) const fn value_type(&self) -> CellValueType {
        self.value_type
    }

    /// Returns the canonical text that a future `value2` write must reproduce.
    pub(super) fn value_text(&self) -> &str {
        &self.value_text
    }

    pub(super) fn parse(input_json: &str) -> Result<Self, super::ExcelT21Error> {
        let input_json = input_json.trim_matches(is_json_whitespace);
        let first = input_json
            .chars()
            .next()
            .ok_or_else(|| invalid_arguments("Excel scalar JSON must not be empty"))?;
        if matches!(first, '[' | '{') {
            return Err(invalid_arguments(
                "Excel scalar write accepts one JSON scalar, not an array or object",
            ));
        }
        if input_json.len() > MAX_RANGE_PAYLOAD_BYTES {
            return Err(scalar_too_large());
        }

        let (value_type, value_text) = match first {
            '"' => {
                let text = parse_json_string(input_json)?;
                if has_formula_prefix(&text) {
                    return Err(invalid_arguments(
                        "Excel scalar text starts with a formula prefix",
                    ));
                }
                (CellValueType::Text, text)
            }
            't' if input_json == "true" => (CellValueType::Boolean, "true".to_owned()),
            'f' if input_json == "false" => (CellValueType::Boolean, "false".to_owned()),
            'n' if input_json == "null" => (CellValueType::Blank, String::new()),
            '-' | '0'..='9' => (CellValueType::Number, parse_json_number(input_json)?),
            _ => {
                return Err(invalid_arguments(
                    "Excel scalar JSON must be null, boolean, number, or string",
                ));
            }
        };
        if canonical_payload_bytes(value_type, &value_text)? > MAX_RANGE_PAYLOAD_BYTES {
            return Err(scalar_too_large());
        }

        Ok(Self {
            value_type,
            value_text,
        })
    }
}

/// Validates one scalar and prepares a single write at the anchor's captured fingerprint.
///
/// Validation happens first so an invalid scalar does not consume the anchor's one-use
/// write preparation. Identity, fingerprint, and single-use checks remain owned by the
/// existing L1 store.
fn prepare_scalar_write(
    store: &mut ExcelL1AnchorStore,
    handle: ExcelL1AnchorHandle,
    observed: &ExcelL1WorkbookIdentity,
    input_json: &str,
) -> Result<ExcelL1ScalarLiteral, super::ExcelT21Error> {
    let literal = ExcelL1ScalarLiteral::parse(input_json)?;
    store.prepare_write(handle, observed)?;
    Ok(literal)
}

fn has_formula_prefix(value: &str) -> bool {
    matches!(
        value
            .trim_start_matches(is_ignored_formula_prefix)
            .chars()
            .next(),
        Some('=' | '+' | '-' | '@')
    )
}

fn is_ignored_formula_prefix(value: char) -> bool {
    value.is_whitespace() || value.is_control() || is_default_ignorable(value)
}

fn is_default_ignorable(value: char) -> bool {
    matches!(
        value,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'..='\u{1160}'
            | '\u{17B4}'..='\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

fn is_json_whitespace(value: char) -> bool {
    matches!(value, ' ' | '\t' | '\n' | '\r')
}

fn parse_json_string(input_json: &str) -> Result<String, super::ExcelT21Error> {
    let bytes = input_json.as_bytes();
    let mut index = 1_usize;
    let mut escaped = false;
    let mut token_end = None;
    while let Some(byte) = bytes.get(index) {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'"' {
            token_end = Some(index);
            break;
        }
        index = index
            .checked_add(1)
            .ok_or_else(|| invalid_arguments("Excel scalar JSON string is too long"))?;
    }
    let token_end =
        token_end.ok_or_else(|| invalid_arguments("Excel scalar JSON string is not terminated"))?;
    let trailing_start = token_end
        .checked_add(1)
        .ok_or_else(|| invalid_arguments("Excel scalar JSON string is too long"))?;
    let trailing = input_json
        .get(trailing_start..)
        .ok_or_else(|| invalid_arguments("Excel scalar JSON string boundary is malformed"))?;
    if !trailing.chars().all(is_json_whitespace) {
        return Err(invalid_arguments(
            "Excel scalar JSON string has trailing tokens",
        ));
    }
    let token = input_json
        .get(..=token_end)
        .ok_or_else(|| invalid_arguments("Excel scalar JSON string token is malformed"))?;
    serde_json::from_str::<String>(token)
        .map_err(|_| invalid_arguments("Excel scalar JSON string is malformed"))
}

fn parse_json_number(input_json: &str) -> Result<String, super::ExcelT21Error> {
    let bytes = input_json.as_bytes();
    let mut index = 0_usize;
    if bytes.get(index) == Some(&b'-') {
        index = 1;
    }
    match bytes.get(index).copied() {
        Some(b'0') => {
            index += 1;
            if bytes.get(index).is_some_and(u8::is_ascii_digit) {
                return Err(invalid_arguments(
                    "Excel scalar JSON number has a leading zero",
                ));
            }
        }
        Some(b'1'..=b'9') => {
            index += 1;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
        }
        _ => return Err(invalid_arguments("Excel scalar JSON number is malformed")),
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return Err(invalid_arguments(
                "Excel scalar JSON number fraction is missing",
            ));
        }
    }
    if matches!(bytes.get(index).copied(), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index).copied(), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            return Err(invalid_arguments(
                "Excel scalar JSON number exponent is missing",
            ));
        }
    }
    if index != bytes.len() {
        return Err(invalid_arguments(
            "Excel scalar JSON number has trailing tokens",
        ));
    }
    CanonicalDecimal::parse(input_json)
        .and_then(|number| number.as_text())
        .map_err(|_| invalid_arguments("Excel scalar number is not canonicalizable"))
}

fn canonical_payload_bytes(
    value_type: CellValueType,
    value_text: &str,
) -> Result<usize, super::ExcelT21Error> {
    match value_type {
        CellValueType::Blank => Ok(4),
        CellValueType::Boolean if matches!(value_text, "true" | "false") => Ok(value_text.len()),
        CellValueType::Number => Ok(value_text.len()),
        CellValueType::Text => serde_json::to_string(value_text)
            .map(|encoded| encoded.len())
            .map_err(|error| {
                super::ExcelT21Error::new(
                    ErrorCode::Fatal,
                    format!("Excel scalar text serialization failed: {error}"),
                )
            }),
        CellValueType::Boolean | CellValueType::Error => Err(super::ExcelT21Error::new(
            ErrorCode::Fatal,
            "Excel scalar type became invalid",
        )),
    }
}

fn scalar_too_large() -> super::ExcelT21Error {
    invalid_arguments(format!(
        "Excel scalar exceeds {MAX_RANGE_PAYLOAD_BYTES} bytes"
    ))
}

#[cfg(test)]
#[path = "excel_t21_l1_write_tests.rs"]
mod tests;
