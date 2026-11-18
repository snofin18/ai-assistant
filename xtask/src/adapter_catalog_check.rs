//! Closed structural validation for `adapter-catalog-1.0.json`.
//!
//! Responsibility: reject missing, unknown, duplicate, mistyped, or internally inconsistent
//! postcondition catalog entries before codegen and adapter validation consume them.
//!
//! Boundary: this module only validates an already parsed [`Value`](crate::serde_json_lite::Value);
//! it does not read files or generate output.

use crate::serde_json_lite::Value;

const FIELD_TYPE_VALUES: [&str; 11] = [
    "non_empty_string",
    "optional_string",
    "scalar",
    "number",
    "positive_integer",
    "fingerprint",
    "ErrorCode",
    "any|size|mtime|digest",
    "title|text|fingerprint",
    "equals|not_equals|contains|not_contains",
    "string",
];

const VERIFY_FIELD_TYPE_VALUES: [&str; 10] = [
    "non_empty_string",
    "optional_string",
    "scalar",
    "number",
    "positive_integer",
    "fingerprint",
    "any|size|mtime|digest",
    "title|text|fingerprint",
    "equals|not_equals|contains|not_contains",
    "string",
];

pub fn check_adapter_catalog(value: &Value) -> Option<String> {
    let Some(object) = value.as_object() else {
        return Some("adapter catalog root must be object".to_string());
    };
    if object.keys().any(|field| {
        !["version", "capabilities", "error_codes", "postconditions"].contains(&field.as_str())
    }) {
        return Some("adapter catalog has unknown top-level field".to_string());
    }
    if object.get("version").and_then(Value::as_str) != Some("1.0") {
        return Some("adapter catalog version must be 1.0".to_string());
    }
    for field in ["capabilities", "error_codes"] {
        if let Some(error) = check_catalog_identifier_array(value, field) {
            return Some(error);
        }
    }
    let Some(postconditions) = value.get("postconditions").and_then(Value::as_array) else {
        return Some("adapter catalog missing postconditions array".to_string());
    };
    if postconditions.is_empty() {
        return Some("adapter catalog postconditions must not be empty".to_string());
    }
    let mut kinds: Vec<&str> = Vec::with_capacity(postconditions.len());
    let mut previous_kind: Option<&str> = None;
    for (index, entry) in postconditions.iter().enumerate() {
        let kind = match check_catalog_postcondition(entry, index) {
            Ok(kind) => kind,
            Err(error) => return Some(error),
        };
        if kinds.contains(&kind) {
            return Some(format!("duplicate adapter postcondition kind: {kind}"));
        }
        if previous_kind.is_some_and(|previous| previous >= kind) {
            return Some(format!(
                "adapter catalog postconditions must be byte-order sorted: {kind}"
            ));
        }
        previous_kind = Some(kind);
        kinds.push(kind);
    }
    None
}

fn check_catalog_identifier_array(value: &Value, field: &str) -> Option<String> {
    let Some(entries) = value.get(field).and_then(Value::as_array) else {
        return Some(format!("adapter catalog missing `{field}` array"));
    };
    if entries.is_empty() {
        return Some(format!("adapter catalog `{field}` must not be empty"));
    }
    let mut seen: Vec<&str> = Vec::with_capacity(entries.len());
    let mut previous: Option<&str> = None;
    for (index, entry) in entries.iter().enumerate() {
        let Some(identifier) = entry.as_str() else {
            return Some(format!(
                "adapter catalog `{field}[{index}]` must be a string"
            ));
        };
        if identifier.trim().is_empty() {
            return Some(format!(
                "adapter catalog `{field}[{index}]` must not be empty"
            ));
        }
        if seen.contains(&identifier) {
            return Some(format!(
                "duplicate adapter catalog `{field}` value: {identifier}"
            ));
        }
        if previous.is_some_and(|previous| previous >= identifier) {
            return Some(format!(
                "adapter catalog `{field}` must be byte-order sorted: {identifier}"
            ));
        }
        previous = Some(identifier);
        seen.push(identifier);
    }
    None
}

fn check_catalog_postcondition(entry: &Value, index: usize) -> Result<&str, String> {
    let Some(object) = entry.as_object() else {
        return Err(format!("adapter postcondition[{index}] must be an object"));
    };
    check_postcondition_keys(object, index)?;
    let Some(kind) = entry.get("kind").and_then(Value::as_str) else {
        return Err(format!(
            "adapter postcondition[{index}] missing string `kind`"
        ));
    };
    if kind.trim().is_empty() {
        return Err(format!("adapter postcondition[{index}] has empty `kind`"));
    }
    let (required, allowed) = check_postcondition_arrays(entry, kind)?;
    check_postcondition_field_types(entry, kind, &required, &allowed)?;
    check_postcondition_constraints(entry, kind, &allowed)?;
    Ok(kind)
}

fn check_postcondition_keys(
    object: &std::collections::BTreeMap<String, Value>,
    index: usize,
) -> Result<(), String> {
    for field in object.keys() {
        if ![
            "kind",
            "required_fields",
            "allowed_fields",
            "field_types",
            "effect_eligibility",
            "constraints",
        ]
        .contains(&field.as_str())
        {
            return Err(format!(
                "adapter postcondition[{index}] has unknown field `{field}`"
            ));
        }
    }
    Ok(())
}

fn check_postcondition_arrays<'a>(
    entry: &'a Value,
    kind: &str,
) -> Result<(Vec<&'a str>, Vec<&'a str>), String> {
    let required = catalog_string_list(entry, kind, "required_fields")?;
    let allowed = catalog_string_list(entry, kind, "allowed_fields")?;
    let effects = catalog_string_list(entry, kind, "effect_eligibility")?;
    if !required.contains(&"kind") {
        return Err(format!(
            "adapter postcondition `{kind}` required_fields must contain kind"
        ));
    }
    if let Some(missing) = required.iter().find(|field| !allowed.contains(field)) {
        return Err(format!(
            "adapter postcondition `{kind}` required field `{missing}` is not allowed"
        ));
    }
    if let Some(effect) = effects
        .iter()
        .find(|effect| !matches!(**effect, "read" | "write"))
    {
        return Err(format!(
            "adapter postcondition `{kind}` has invalid effect `{effect}`"
        ));
    }
    Ok((required, allowed))
}

fn check_postcondition_field_types(
    entry: &Value,
    kind: &str,
    required: &[&str],
    allowed: &[&str],
) -> Result<(), String> {
    let Some(field_types) = entry.get("field_types").and_then(Value::as_object) else {
        return Err(format!(
            "adapter postcondition `{kind}` missing `field_types` object"
        ));
    };
    if field_types.is_empty() {
        return Err(format!(
            "adapter postcondition `{kind}` has empty `field_types`"
        ));
    }
    let expected_fields: Vec<&str> = allowed
        .iter()
        .copied()
        .filter(|field| *field != "kind")
        .collect();
    if field_types.len() != expected_fields.len()
        || !field_types
            .keys()
            .all(|field| expected_fields.contains(&field.as_str()))
    {
        return Err(format!(
            "adapter postcondition `{kind}` field_types keys must equal allowed_fields minus kind"
        ));
    }
    for (field, kind_name) in field_types {
        if kind_name
            .as_str()
            .is_none_or(|text| !FIELD_TYPE_VALUES.contains(&text))
        {
            return Err(format!(
                "adapter postcondition `{kind}` field type `{field}` is not recognized"
            ));
        }
    }
    if let Some(field) = required
        .iter()
        .find(|field| **field != "kind" && !field_types.contains_key(**field))
    {
        return Err(format!(
            "adapter postcondition `{kind}` required field `{field}` has no field type"
        ));
    }
    Ok(())
}

fn check_postcondition_constraints(
    entry: &Value,
    kind: &str,
    allowed: &[&str],
) -> Result<(), String> {
    let Some(constraints) = entry.get("constraints").and_then(Value::as_array) else {
        return Err(format!(
            "adapter postcondition `{kind}` missing `constraints` array"
        ));
    };
    for (index, constraint) in constraints.iter().enumerate() {
        let context = format!("adapter postcondition `{kind}` constraint[{index}]");
        let Some(object) = constraint.as_object() else {
            return Err(format!("{context} must be object"));
        };
        let Some(tag) = object.get("kind").and_then(Value::as_str) else {
            return Err(format!("{context} missing string `kind`"));
        };
        match tag {
            "field_value_type" => {
                check_field_value_type_constraint(object, entry, allowed, &context)?;
            }
            "if_field_equals" => {
                check_if_field_equals_constraint(object, entry, allowed, &context)?;
            }
            "numeric_less_than_or_equal" => {
                check_numeric_constraint(object, allowed, entry, &context)?;
            }
            other => {
                return Err(format!("{context} has unknown constraint kind `{other}`"));
            }
        }
    }
    Ok(())
}

fn check_field_value_type_constraint(
    object: &std::collections::BTreeMap<String, Value>,
    entry: &Value,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    check_exact_keys(object, &["kind", "field", "value_field", "cases"], context)?;
    let field = constraint_field(object, "field", allowed, context)?;
    let _value_field = constraint_field(object, "value_field", allowed, context)?;
    let discriminator_values = declared_union_values(entry, field)
        .ok_or_else(|| format!("{context}.field `{field}` must be a string union"))?;
    let Some(cases) = object.get("cases").and_then(Value::as_object) else {
        return Err(format!("{context}.cases must be object"));
    };
    if cases.is_empty() {
        return Err(format!("{context}.cases must not be empty"));
    }
    for (case, field_type) in cases {
        if case.trim().is_empty() {
            return Err(format!("{context}.cases contains empty key"));
        }
        if !discriminator_values.contains(&case.as_str()) {
            return Err(format!(
                "{context}.cases.{case} is not declared by `{field}`"
            ));
        }
        let Some(field_type) = field_type.as_str() else {
            return Err(format!("{context}.cases.{case} must be string"));
        };
        if !VERIFY_FIELD_TYPE_VALUES.contains(&field_type) {
            return Err(format!(
                "{context}.cases.{case} uses unknown type `{field_type}`"
            ));
        }
    }
    if let Some(missing) = discriminator_values
        .iter()
        .find(|value| !cases.contains_key(**value))
    {
        return Err(format!(
            "{context}.cases missing discriminator value `{missing}`"
        ));
    }
    Ok(())
}

fn check_if_field_equals_constraint(
    object: &std::collections::BTreeMap<String, Value>,
    entry: &Value,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    check_exact_keys(
        object,
        &["kind", "field", "equals", "then_field", "allowed_values"],
        context,
    )?;
    let field = constraint_field(object, "field", allowed, context)?;
    let equals = constraint_non_empty_string(object, "equals", context)?;
    let then_field = constraint_field(object, "then_field", allowed, context)?;
    let discriminator_values = declared_union_values(entry, field)
        .ok_or_else(|| format!("{context}.field `{field}` must be a string union"))?;
    if !discriminator_values.contains(&equals) {
        return Err(format!(
            "{context}.equals `{equals}` is not declared by `{field}`"
        ));
    }
    let allowed_values = constraint_string_values(object, "allowed_values", context)?;
    if let Some(then_values) = declared_union_values(entry, then_field)
        && allowed_values
            .iter()
            .any(|value| !then_values.contains(value))
    {
        return Err(format!(
            "{context}.allowed_values contains a value not declared by `{then_field}`"
        ));
    }
    Ok(())
}

fn check_numeric_constraint(
    object: &std::collections::BTreeMap<String, Value>,
    allowed: &[&str],
    entry: &Value,
    context: &str,
) -> Result<(), String> {
    check_exact_keys(object, &["kind", "left_field", "right_field"], context)?;
    let left = constraint_field(object, "left_field", allowed, context)?;
    let right = constraint_field(object, "right_field", allowed, context)?;
    let Some(field_types) = entry.get("field_types").and_then(Value::as_object) else {
        return Err(format!("{context} missing sibling field_types object"));
    };
    for field in [left, right] {
        if field_types.get(field).and_then(Value::as_str) != Some("number") {
            return Err(format!(
                "{context} numeric field `{field}` must have type `number`"
            ));
        }
    }
    Ok(())
}

fn check_exact_keys(
    object: &std::collections::BTreeMap<String, Value>,
    expected: &[&str],
    context: &str,
) -> Result<(), String> {
    if object.len() != expected.len()
        || !object
            .keys()
            .all(|field| expected.contains(&field.as_str()))
    {
        return Err(format!(
            "{context} keys must equal [{}]",
            expected.join(", ")
        ));
    }
    Ok(())
}

fn constraint_field<'a>(
    object: &'a std::collections::BTreeMap<String, Value>,
    field: &str,
    allowed: &[&str],
    context: &str,
) -> Result<&'a str, String> {
    let value = constraint_non_empty_string(object, field, context)?;
    if !allowed.contains(&value) {
        return Err(format!("{context}.{field} `{value}` is not allowed"));
    }
    Ok(value)
}

fn constraint_non_empty_string<'a>(
    object: &'a std::collections::BTreeMap<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{context}.{field} must be non-empty string"))
}

fn constraint_string_values<'a>(
    object: &'a std::collections::BTreeMap<String, Value>,
    field: &str,
    context: &str,
) -> Result<Vec<&'a str>, String> {
    let Some(values) = object.get(field).and_then(Value::as_array) else {
        return Err(format!("{context}.{field} must be array"));
    };
    if values.is_empty() {
        return Err(format!("{context}.{field} must not be empty"));
    }
    let mut seen = Vec::with_capacity(values.len());
    for value in values {
        let Some(text) = value.as_str() else {
            return Err(format!("{context}.{field} entries must be strings"));
        };
        if text.trim().is_empty() || seen.contains(&text) {
            return Err(format!(
                "{context}.{field} entries must be unique and non-empty"
            ));
        }
        seen.push(text);
    }
    Ok(seen)
}

fn declared_union_values<'a>(entry: &'a Value, field: &str) -> Option<Vec<&'a str>> {
    let declaration = entry
        .get("field_types")
        .and_then(Value::as_object)
        .and_then(|field_types| field_types.get(field))
        .and_then(Value::as_str)?;
    if !declaration.contains('|') {
        return None;
    }
    Some(declaration.split('|').collect())
}

fn catalog_string_list<'a>(
    entry: &'a Value,
    kind: &str,
    field: &str,
) -> Result<Vec<&'a str>, String> {
    let Some(values) = entry.get(field).and_then(Value::as_array) else {
        return Err(format!(
            "adapter postcondition `{kind}` missing `{field}` array"
        ));
    };
    if values.is_empty() {
        return Err(format!(
            "adapter postcondition `{kind}` has empty `{field}`"
        ));
    }
    let mut result = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let Some(text) = value.as_str() else {
            return Err(format!(
                "adapter postcondition `{kind}` `{field}[{index}]` must be a string"
            ));
        };
        if text.trim().is_empty() {
            return Err(format!(
                "adapter postcondition `{kind}` `{field}[{index}]` must not be empty"
            ));
        }
        if result.contains(&text) {
            return Err(format!(
                "adapter postcondition `{kind}` has duplicate `{field}` value: {text}"
            ));
        }
        result.push(text);
    }
    Ok(result)
}
