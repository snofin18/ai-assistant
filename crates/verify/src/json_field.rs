//! Fail-closed extraction of typed fields from a JSON object.
//!
//! Responsibility: the small, repetitive, easy-to-get-wrong part of postcondition parsing — "this
//! field must exist, must be a string, must not be empty". Every helper reports the *reason* and the
//! offending field name, because a tool author has to be able to fix the postcondition from the
//! error alone.
//!
//! Boundary: no schema knowledge. It does not know what `kind` values exist or which fields they
//! allow; that lives in [`crate::postcondition`]. This module only knows JSON types.
//!
//! ## Why a missing key is never `null`
//!
//! `object.get(key)` returns `None` for an absent key and `Some(Value::Null)` for an explicit
//! `null`. Both are rejected, but with different messages, so "I forgot the field" and "I wrote
//! `null`" are distinguishable in the error. Neither is ever coerced into a default: a defaulted
//! field is a policy the author never wrote (invariant 5 of `postcondition.rs`).

use assistant_protocol::serde_json::{Map, Value as JsonValue};

use crate::error::{VerifyError, VerifyResult, malformed};

/// The JSON type name of a value, used in error messages.
#[must_use]
pub const fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

/// Rejects any key outside `allowed`.
///
/// An unexpected key is almost always a typo (`withinMs` instead of `within_ms`), and silently
/// ignoring it would drop a constraint the author believed they had written.
pub fn only_keys(
    index: usize,
    object: &Map<String, JsonValue>,
    allowed: &[&str],
) -> VerifyResult<()> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(malformed(
                index,
                format!("unexpected field `{key}` (allowed: {allowed:?})"),
            ));
        }
    }
    Ok(())
}

/// Reads a required string field.
pub fn required_text(
    index: usize,
    object: &Map<String, JsonValue>,
    key: &str,
) -> VerifyResult<String> {
    match object.get(key) {
        Some(JsonValue::String(text)) => Ok(text.clone()),
        Some(other) => Err(malformed(
            index,
            format!("`{key}` must be a string, got {}", json_type_name(other)),
        )),
        None => Err(malformed(index, format!("`{key}` is required"))),
    }
}

/// Reads a required non-empty string field.
pub fn required_non_empty_text(
    index: usize,
    object: &Map<String, JsonValue>,
    key: &str,
) -> VerifyResult<String> {
    let text = required_text(index, object, key)?;
    if text.is_empty() {
        return Err(malformed(index, format!("`{key}` must not be empty")));
    }
    Ok(text)
}

/// Reads an optional string field; an absent key yields `None`, a wrong type is an error.
pub fn optional_text(
    index: usize,
    object: &Map<String, JsonValue>,
    key: &str,
) -> VerifyResult<Option<String>> {
    match object.get(key) {
        None => Ok(None),
        Some(JsonValue::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(malformed(
            index,
            format!("`{key}` must be a string, got {}", json_type_name(other)),
        )),
    }
}

/// Reads a required non-negative integer field.
pub fn required_u64(index: usize, object: &Map<String, JsonValue>, key: &str) -> VerifyResult<u64> {
    match object.get(key) {
        Some(JsonValue::Number(number)) => number
            .as_u64()
            .ok_or_else(|| malformed(index, format!("`{key}` must be a non-negative integer"))),
        Some(other) => Err(malformed(
            index,
            format!("`{key}` must be a number, got {}", json_type_name(other)),
        )),
        None => Err(malformed(index, format!("`{key}` is required"))),
    }
}

/// Reads a required finite number field as `f64`.
///
/// `serde_json` cannot represent NaN or infinity, so `as_f64()` returning `Some` already implies a
/// finite value; the `None` arm is kept so the function never assumes it.
pub fn required_f64(index: usize, object: &Map<String, JsonValue>, key: &str) -> VerifyResult<f64> {
    match object.get(key) {
        Some(JsonValue::Number(number)) => number
            .as_f64()
            .ok_or_else(|| malformed(index, format!("`{key}` must be a finite number"))),
        Some(other) => Err(malformed(
            index,
            format!("`{key}` must be a number, got {}", json_type_name(other)),
        )),
        None => Err(malformed(index, format!("`{key}` is required"))),
    }
}

/// Builds the error for an object that is not an object at all.
pub fn not_an_object(index: usize, value: &JsonValue) -> VerifyError {
    VerifyError::PostconditionNotAnObject {
        index,
        actual: json_type_name(value).to_owned(),
    }
}
