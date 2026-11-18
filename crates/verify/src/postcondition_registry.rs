//! Machine-readable registry for Adapter-supported postcondition shapes.
//!
//! Responsibility: expose the versioned JSON registry compiled into `assistant-verify` and
//! validate closed field/type/constraint structure for `parse_postconditions`.
//!
//! Boundary: this module does not construct a [`crate::Postcondition`] or evaluate an assertion.
//! Those remain in [`crate::parse_postconditions`] and the evaluation engine.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use assistant_platform_api::Fingerprint;
use assistant_protocol::serde_json::{Map, Value as JsonValue};
use serde::Deserialize;

/// Versioned machine registry for Adapter-supported postcondition kinds.
///
/// The embedded JSON is closed by schema tests and is the only source for generated Adapter
/// catalogs. It includes an explicit `excluded_kinds` entry for `visual_assert` while Adapter
/// catalog 1.0 cannot represent visual field/op/tolerance variants.
pub const POSTCONDITION_REGISTRY_JSON: &str = include_str!("postcondition-registry-1.0.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    version: String,
    postconditions: Vec<RegistryPostcondition>,
    excluded_kinds: Vec<ExcludedKind>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryPostcondition {
    kind: String,
    required_fields: Vec<String>,
    allowed_fields: Vec<String>,
    field_types: BTreeMap<String, String>,
    effect_eligibility: Vec<String>,
    constraints: Vec<RegistryConstraint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExcludedKind {
    kind: String,
    reason: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RegistryConstraint {
    FieldValueType {
        field: String,
        value_field: String,
        cases: BTreeMap<String, String>,
    },
    IfFieldEquals {
        field: String,
        equals: String,
        then_field: String,
        allowed_values: Vec<String>,
    },
    NumericLessThanOrEqual {
        left_field: String,
        right_field: String,
    },
}

static REGISTRY: OnceLock<Result<Registry, String>> = OnceLock::new();

/// Return whether `kind` is in the Adapter-supported machine registry.
pub fn is_registered(index: usize, kind: &str) -> Result<bool, crate::VerifyError> {
    Ok(registry(index)?
        .postconditions
        .iter()
        .any(|rule| rule.kind == kind))
}

/// Validate a non-visual postcondition's closed fields, basic types, and tagged cross-field
/// constraints from the registry. The specialized parser still constructs typed variants and may
/// re-check a relationship while doing so, but the registry is the machine source for Adapter
/// catalog generation.
pub fn validate_shape(
    index: usize,
    kind: &str,
    object: &Map<String, JsonValue>,
) -> Result<(), crate::VerifyError> {
    let registry = registry(index)?;
    let rule = registry
        .postconditions
        .iter()
        .find(|rule| rule.kind == kind)
        .ok_or_else(|| registry_error(index, &format!("unknown kind `{kind}`")))?;
    if registry
        .excluded_kinds
        .iter()
        .any(|excluded| excluded.kind == kind && excluded.reason.trim().is_empty())
    {
        return Err(registry_error(
            index,
            &format!("excluded kind `{kind}` has empty reason"),
        ));
    }
    if !rule
        .effect_eligibility
        .iter()
        .all(|effect| matches!(effect.as_str(), "read" | "write"))
    {
        return Err(registry_error(
            index,
            &format!("kind `{kind}` has invalid effect eligibility"),
        ));
    }
    for key in object.keys() {
        if !rule.allowed_fields.contains(key) {
            return Err(registry_error(
                index,
                &format!("kind `{kind}` has unexpected field `{key}`"),
            ));
        }
    }
    for field in &rule.required_fields {
        if !object.contains_key(field) {
            return Err(registry_error(
                index,
                &format!("kind `{kind}` missing required field `{field}`"),
            ));
        }
    }
    for (field, field_type) in &rule.field_types {
        if let Some(value) = object.get(field)
            && !matches_field_type(value, field_type)
        {
            return Err(registry_error(
                index,
                &format!("kind `{kind}` field `{field}` does not satisfy `{field_type}`"),
            ));
        }
    }
    for constraint in &rule.constraints {
        validate_constraint(index, kind, object, constraint)?;
    }
    Ok(())
}

fn registry(index: usize) -> Result<&'static Registry, crate::VerifyError> {
    REGISTRY
        .get_or_init(|| {
            assistant_protocol::serde_json::from_str::<Registry>(POSTCONDITION_REGISTRY_JSON)
                .map_err(|error| format!("postcondition registry parse: {error}"))
        })
        .as_ref()
        .map_err(|reason| registry_error(index, reason))?;
    let registry = REGISTRY
        .get()
        .ok_or_else(|| registry_error(index, "postcondition registry missing"))?
        .as_ref()
        .map_err(|reason| registry_error(index, reason))?;
    if registry.version != "1.0" {
        return Err(registry_error(
            index,
            "postcondition registry version must be 1.0",
        ));
    }
    Ok(registry)
}

fn matches_field_type(value: &JsonValue, field_type: &str) -> bool {
    match field_type {
        "string" | "optional_string" => value.is_string(),
        "non_empty_string" => value.as_str().is_some_and(|text| !text.is_empty()),
        "scalar" => value.is_string() || value.is_number() || value.is_boolean(),
        "number" => value.is_number(),
        "positive_integer" => value.as_u64().is_some_and(|number| number > 0),
        "fingerprint" => value
            .as_str()
            .is_some_and(|text| Fingerprint::parse(text).is_ok()),
        other => other
            .split('|')
            .any(|allowed| value.as_str() == Some(allowed)),
    }
}

fn validate_constraint(
    index: usize,
    kind: &str,
    object: &Map<String, JsonValue>,
    constraint: &RegistryConstraint,
) -> Result<(), crate::VerifyError> {
    match constraint {
        RegistryConstraint::FieldValueType {
            field,
            value_field,
            cases,
        } => {
            let Some(case) = object
                .get(field)
                .and_then(JsonValue::as_str)
                .and_then(|case| cases.get(case))
            else {
                return Err(registry_error(
                    index,
                    &format!("kind `{kind}` field `{field}` has no constraint case"),
                ));
            };
            if !object
                .get(value_field)
                .is_some_and(|value| matches_field_type(value, case))
            {
                return Err(registry_error(
                    index,
                    &format!("kind `{kind}` field `{value_field}` violates `{case}`"),
                ));
            }
        }
        RegistryConstraint::IfFieldEquals {
            field,
            equals,
            then_field,
            allowed_values,
        } => {
            if object.get(field).and_then(JsonValue::as_str) == Some(equals) {
                let actual = object.get(then_field).and_then(JsonValue::as_str);
                if !actual
                    .is_some_and(|value| allowed_values.iter().any(|allowed| allowed == value))
                {
                    return Err(registry_error(
                        index,
                        &format!("kind `{kind}` has invalid `{then_field}` for `{field}={equals}`"),
                    ));
                }
            }
        }
        RegistryConstraint::NumericLessThanOrEqual {
            left_field,
            right_field,
        } => {
            let left = object.get(left_field).and_then(JsonValue::as_f64);
            let right = object.get(right_field).and_then(JsonValue::as_f64);
            if !left.zip(right).is_some_and(|(left, right)| left <= right) {
                return Err(registry_error(
                    index,
                    &format!("kind `{kind}` requires `{left_field} <= {right_field}`"),
                ));
            }
        }
    }
    Ok(())
}

fn registry_error(index: usize, reason: &str) -> crate::VerifyError {
    crate::error::malformed(index, format!("postcondition registry: {reason}"))
}
