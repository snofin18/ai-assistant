//! Parsing postconditions from JSON into strong types, fail-closed.
//!
//! Responsibility: turn `PlanStep.postconditions: Vec<serde_json::Value>` (architecture v2
//! appendix A, `#/$defs/assertion`) into this crate's [`Postcondition`]. Anything unknown,
//! incomplete, or mistyped is rejected rather than ignored.
//!
//! Boundary: it does not evaluate anything (that is `assertion.rs`) and touches no IO. The
//! repetitive "must be a string / must not be empty" work lives in [`crate::json_field`]; this
//! module owns the schema knowledge.
//!
//! ## The 11 supported `kind` values
//!
//! `state_assert`, `text_contains`, `text_not_contains`, `state_changed`, `state_unchanged`,
//! `element_exists`, `element_gone`, `value_equals`, `value_in_range`, `file_changed`,
//! `app_reported`.
//!
//! That is exactly architecture section 7.4's list minus `visual_assert`, which needs screenshots,
//! perceptual hashing, tolerance, and `confidence_min` and therefore belongs to **TASK-042**
//! (`crates/verify/src/visual/**`). The appendix A kinds `target_resolvable` and `capability` are
//! *preconditions* (section 5.3) and are rejected here with that reason.
//!
//! ## Why the free-form `assert` string is not implemented
//!
//! Appendix A declares `assertion.assert` as a free-form string (for example
//! `target.text.contains(new_text)`). Supporting it would mean shipping an expression language
//! (lexer, parser, evaluator, scope) -- a new abstraction layer, and one the model cannot
//! enumerate, which is the same objection section 7.4 raises against `if`-`then`-`else`. This crate
//! therefore accepts only the structured form (`field` + `op` + `value`) and rejects an `assert`
//! field with a pointer to that form. The narrowing is recorded as `DRIFT-023-4`.
//!
//! ## Invariants
//!
//! 1. A postcondition that is not a JSON object is rejected.
//! 2. `kind` is required and must be a string.
//! 3. Only the fields documented for that `kind` are allowed; any other key is rejected, because a
//!    typo would otherwise be silently ignored.
//! 4. Field/operator combinations that cannot be evaluated are rejected at parse time (for example
//!    `contains` on a fingerprint).
//! 5. No implicit defaults: every policy-bearing field must be written out.
//! 6. Numbers keep `serde_json`'s representation, so equality stays exact and free of a
//!    floating-point tolerance policy; tolerance belongs to TASK-042.

use assistant_protocol::serde_json::{Map, Number, Value as JsonValue};
use serde::{Deserialize, Serialize};

use crate::error::{VerifyResult, malformed, unsupported};
use crate::json_field::{
    json_type_name, not_an_object, only_keys, optional_text, required_f64, required_non_empty_text,
    required_text, required_u64,
};

/// A scalar value an assertion compares against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AssertValue {
    /// A text value.
    Text(String),
    /// A numeric value.
    Number(Number),
    /// A boolean value.
    Bool(bool),
}

impl AssertValue {
    /// Parses a scalar from JSON, rejecting arrays, objects, and null.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::VerifyError::MalformedPostcondition`] when the value is not a string, number, or
    /// boolean.
    pub fn from_json(index: usize, field: &str, value: &JsonValue) -> VerifyResult<Self> {
        match value {
            JsonValue::String(text) => Ok(Self::Text(text.clone())),
            JsonValue::Number(number) => Ok(Self::Number(number.clone())),
            JsonValue::Bool(flag) => Ok(Self::Bool(*flag)),
            other => Err(malformed(
                index,
                format!(
                    "`{field}` must be a string, number, or boolean, got {}",
                    json_type_name(other)
                ),
            )),
        }
    }

    /// The JSON type name of this scalar, used in error and violation reports.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "string",
            Self::Number(_) => "number",
            Self::Bool(_) => "boolean",
        }
    }

    /// A short rendering used in violation reports.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Text(text) => format!("{text:?}"),
            Self::Number(number) => number.to_string(),
            Self::Bool(flag) => flag.to_string(),
        }
    }

    /// Compares two scalars of the same kind.
    ///
    /// Returns `None` when the two values have different kinds: comparing a string assertion
    /// against a numeric observation is not a falsification, it is a question this crate refuses to
    /// guess at (invariant 2 of `assertion.rs`).
    #[must_use]
    pub fn equal_to(&self, actual: &Self) -> Option<bool> {
        match (self, actual) {
            (Self::Text(expected), Self::Text(actual)) => Some(expected == actual),
            (Self::Number(expected), Self::Number(actual)) => Some(expected == actual),
            (Self::Bool(expected), Self::Bool(actual)) => Some(expected == actual),
            _ => None,
        }
    }
}

/// The field a [`Postcondition::StateAssert`] inspects.
///
/// Enumerated rather than a free-form path expression: the model must be able to enumerate what a
/// contract can say (section 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StateField {
    /// The target window or document title.
    Title,
    /// The target's visible text.
    Text,
    /// The target state fingerprint; the value must be a fingerprint string.
    Fingerprint,
}

/// The comparison operator of a [`Postcondition::StateAssert`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CompareOp {
    /// Exact equality (text compares by code point, with no case folding).
    Equals,
    /// Exact inequality.
    NotEquals,
    /// Substring containment.
    Contains,
    /// Substring absence.
    NotContains,
}

/// Which file attribute a [`Postcondition::FileChanged`] cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FileChangeKind {
    /// Any observed attribute changed.
    Any,
    /// The size changed.
    Size,
    /// The modification time changed.
    Mtime,
    /// The content digest changed; both digests must be present or the assertion is unevaluable.
    Digest,
}

/// One postcondition of a step, after parsing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Postcondition {
    /// A structured assertion over title, text, or fingerprint.
    StateAssert {
        /// Which field to inspect.
        field: StateField,
        /// How to compare.
        op: CompareOp,
        /// The expected value.
        value: AssertValue,
    },
    /// The target text contains `text`.
    TextContains {
        /// The required substring.
        text: String,
    },
    /// The target text does not contain `text`.
    TextNotContains {
        /// The forbidden substring.
        text: String,
    },
    /// The fingerprint changed within `within_ms` milliseconds.
    StateChanged {
        /// The observation scope this assertion is about, when the author pinned one.
        fingerprint_scope: Option<String>,
        /// The deadline in milliseconds.
        within_ms: u64,
    },
    /// The fingerprint did not change.
    StateUnchanged {
        /// The observation scope this assertion is about, when the author pinned one.
        fingerprint_scope: Option<String>,
    },
    /// The element identified by `selector` is present.
    ElementExists {
        /// The element selector.
        selector: String,
    },
    /// The element identified by `selector` is gone.
    ElementGone {
        /// The element selector.
        selector: String,
    },
    /// The named value equals `value`.
    ValueEquals {
        /// The observation key of the value.
        name: String,
        /// The expected value.
        value: AssertValue,
    },
    /// The named value lies inside `[min, max]`.
    ValueInRange {
        /// The observation key of the value.
        name: String,
        /// Inclusive lower bound.
        min: f64,
        /// Inclusive upper bound.
        max: f64,
    },
    /// The file at `path` changed in the expected way.
    FileChanged {
        /// The file path key used in the observation.
        path: String,
        /// Which attribute must have changed.
        expect: FileChangeKind,
    },
    /// The application reported `value` for `key` through its own interface (the most trustworthy
    /// channel when available).
    AppReported {
        /// The application-reported key.
        key: String,
        /// The expected value.
        value: AssertValue,
    },
}

/// Parses a step's postconditions.
///
/// # Errors
///
/// Returns [`crate::error::VerifyError::PostconditionNotAnObject`], [`crate::error::VerifyError::MalformedPostcondition`], or
/// [`crate::error::VerifyError::UnsupportedPostconditionKind`] for the first offending entry; parsing stops there,
/// so the reported index is the first problem to fix.
pub fn parse_postconditions(values: &[JsonValue]) -> VerifyResult<Vec<Postcondition>> {
    let mut parsed = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        parsed.push(parse_one(index, value)?);
    }
    Ok(parsed)
}

/// Reads a required scalar field.
fn required_assert_value(
    index: usize,
    object: &Map<String, JsonValue>,
    key: &str,
) -> VerifyResult<AssertValue> {
    let raw = object
        .get(key)
        .ok_or_else(|| malformed(index, format!("`{key}` is required")))?;
    AssertValue::from_json(index, key, raw)
}

/// Parses one postcondition value.
fn parse_one(index: usize, value: &JsonValue) -> VerifyResult<Postcondition> {
    let JsonValue::Object(object) = value else {
        return Err(not_an_object(index, value));
    };
    let kind = match object.get("kind") {
        Some(JsonValue::String(kind)) => kind.clone(),
        Some(other) => {
            return Err(malformed(
                index,
                format!("`kind` must be a string, got {}", json_type_name(other)),
            ));
        }
        None => return Err(malformed(index, "`kind` is required")),
    };
    parse_kind(index, &kind, object)
}

/// Dispatches on `kind`; every arm is a small function so each rejection reason stays readable.
fn parse_kind(
    index: usize,
    kind: &str,
    object: &Map<String, JsonValue>,
) -> VerifyResult<Postcondition> {
    match kind {
        "state_assert" => parse_state_assert(index, object),
        "text_contains" => {
            only_keys(index, object, &["kind", "value"])?;
            Ok(Postcondition::TextContains {
                text: required_text(index, object, "value")?,
            })
        }
        "text_not_contains" => {
            only_keys(index, object, &["kind", "value"])?;
            Ok(Postcondition::TextNotContains {
                text: required_text(index, object, "value")?,
            })
        }
        "state_changed" => parse_state_changed(index, object),
        "state_unchanged" => {
            only_keys(index, object, &["kind", "fingerprint_scope"])?;
            Ok(Postcondition::StateUnchanged {
                fingerprint_scope: optional_text(index, object, "fingerprint_scope")?,
            })
        }
        "element_exists" => {
            only_keys(index, object, &["kind", "selector"])?;
            Ok(Postcondition::ElementExists {
                selector: required_non_empty_text(index, object, "selector")?,
            })
        }
        "element_gone" => {
            only_keys(index, object, &["kind", "selector"])?;
            Ok(Postcondition::ElementGone {
                selector: required_non_empty_text(index, object, "selector")?,
            })
        }
        "value_equals" => {
            only_keys(index, object, &["kind", "name", "value"])?;
            Ok(Postcondition::ValueEquals {
                name: required_non_empty_text(index, object, "name")?,
                value: required_assert_value(index, object, "value")?,
            })
        }
        "value_in_range" => parse_value_in_range(index, object),
        "file_changed" => parse_file_changed(index, object),
        "app_reported" => {
            only_keys(index, object, &["kind", "key", "value"])?;
            Ok(Postcondition::AppReported {
                key: required_non_empty_text(index, object, "key")?,
                value: required_assert_value(index, object, "value")?,
            })
        }
        "visual_assert" => Err(unsupported(
            index,
            kind,
            "screenshots, perceptual hashing, tolerance, and `confidence_min` belong to TASK-042 \
             (`crates/verify/src/visual/**`)",
        )),
        "target_resolvable" | "capability" => Err(unsupported(
            index,
            kind,
            "this is a precondition kind (architecture section 5.3), not a postcondition",
        )),
        other => Err(unsupported(
            index,
            other,
            "unknown kind; the 11 supported kinds are listed in the crate documentation",
        )),
    }
}

fn parse_state_changed(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<Postcondition> {
    only_keys(index, object, &["kind", "within_ms", "fingerprint_scope"])?;
    let within_ms = required_u64(index, object, "within_ms")?;
    if within_ms == 0 {
        return Err(malformed(
            index,
            "`within_ms` must be greater than zero (a zero deadline can never be met)",
        ));
    }
    Ok(Postcondition::StateChanged {
        fingerprint_scope: optional_text(index, object, "fingerprint_scope")?,
        within_ms,
    })
}

fn parse_value_in_range(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<Postcondition> {
    only_keys(index, object, &["kind", "name", "min", "max"])?;
    let min = required_f64(index, object, "min")?;
    let max = required_f64(index, object, "max")?;
    if min > max {
        return Err(malformed(
            index,
            format!("`min` ({min}) must not exceed `max` ({max})"),
        ));
    }
    Ok(Postcondition::ValueInRange {
        name: required_non_empty_text(index, object, "name")?,
        min,
        max,
    })
}

fn parse_file_changed(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<Postcondition> {
    only_keys(index, object, &["kind", "path", "expect"])?;
    let expect = match required_text(index, object, "expect")?.as_str() {
        "any" => FileChangeKind::Any,
        "size" => FileChangeKind::Size,
        "mtime" => FileChangeKind::Mtime,
        "digest" => FileChangeKind::Digest,
        other => {
            return Err(malformed(
                index,
                format!("`expect` must be one of any/size/mtime/digest, got `{other}`"),
            ));
        }
    };
    Ok(Postcondition::FileChanged {
        path: required_non_empty_text(index, object, "path")?,
        expect,
    })
}

/// Parses `state_assert`, rejecting field/operator combinations that cannot be evaluated.
fn parse_state_assert(
    index: usize,
    object: &Map<String, JsonValue>,
) -> VerifyResult<Postcondition> {
    only_keys(index, object, &["kind", "field", "op", "value"])?;
    let field = match required_text(index, object, "field")?.as_str() {
        "title" => StateField::Title,
        "text" => StateField::Text,
        "fingerprint" => StateField::Fingerprint,
        other => {
            return Err(malformed(
                index,
                format!("`field` must be one of title/text/fingerprint, got `{other}`"),
            ));
        }
    };
    let op = match required_text(index, object, "op")?.as_str() {
        "equals" => CompareOp::Equals,
        "not_equals" => CompareOp::NotEquals,
        "contains" => CompareOp::Contains,
        "not_contains" => CompareOp::NotContains,
        other => {
            return Err(malformed(
                index,
                format!(
                    "`op` must be one of equals/not_equals/contains/not_contains, got `{other}`"
                ),
            ));
        }
    };
    let value = required_assert_value(index, object, "value")?;

    match field {
        StateField::Title | StateField::Text => {
            if !matches!(value, AssertValue::Text(_)) {
                return Err(malformed(
                    index,
                    "`state_assert` on `title`/`text` requires a string `value`",
                ));
            }
        }
        StateField::Fingerprint => {
            if !matches!(op, CompareOp::Equals | CompareOp::NotEquals) {
                return Err(malformed(
                    index,
                    "`state_assert` on `fingerprint` supports only `equals`/`not_equals` \
                     (substring matching on a digest is meaningless)",
                ));
            }
            let AssertValue::Text(text) = &value else {
                return Err(malformed(
                    index,
                    "`state_assert` on `fingerprint` requires a fingerprint string",
                ));
            };
            crate::fingerprint::parse_fingerprint(text).map_err(|error| {
                malformed(index, format!("`value` is not a fingerprint: {error}"))
            })?;
        }
    }

    Ok(Postcondition::StateAssert { field, op, value })
}
