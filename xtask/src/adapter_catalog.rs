//! Generated Adapter validator catalog for capabilities, `ErrorCode`s, and postconditions.
//!
//! Responsibility: produce a deterministic machine snapshot consumed by
//! `adapter-validate`. The snapshot joins the protocol capability catalog, the protocol
//! `ErrorCode` catalog, and the Verify postcondition machine registry.
//!
//! Boundary: this module only reads repository files in [`render`] and never writes.
//! `codegen` owns drift detection and writes.
//!
//! Invariants:
//! 1. capability and `ErrorCode` arrays are unique and byte-order sorted;
//! 2. postcondition kinds are unique and byte-order sorted;
//! 3. every object has fixed key order and the file ends with one LF;
//! 4. Verify registry constraints are copied into the generated catalog without reinterpretation;
//! 5. missing/malformed source arrays fail loudly.

use std::path::Path;

use crate::serde_json_lite::{self, Value};

/// Repository-relative generated catalog path.
pub const CATALOG_PATH: &str = "protocol/adapter-catalog/adapter-catalog-1.0.json";

const CAPABILITY_PATH: &str = "protocol/capability-matrix/capability-1.0.json";
const ERROR_CODE_PATH: &str = "protocol/error-codes/error-codes-1.0.json";
const POSTCONDITION_REGISTRY_PATH: &str = "crates/verify/src/postcondition-registry-1.0.json";

struct PostconditionRule<'a> {
    kind: &'a str,
    required: Vec<&'a str>,
    allowed: Vec<&'a str>,
    field_types: Vec<(&'a str, &'a str)>,
    effects: Vec<&'a str>,
    constraints: Vec<&'a Value>,
}

/// Render the complete generated catalog from repository protocol sources.
///
/// # Errors
///
/// Returns a contextual error when either source cannot be read or parsed, or when
/// required arrays/fields are missing.
pub fn render(repo_root: &Path) -> Result<String, String> {
    let capability_text = std::fs::read_to_string(repo_root.join(CAPABILITY_PATH))
        .map_err(|error| format!("read {CAPABILITY_PATH}: {error}"))?;
    let error_code_text = std::fs::read_to_string(repo_root.join(ERROR_CODE_PATH))
        .map_err(|error| format!("read {ERROR_CODE_PATH}: {error}"))?;
    let postcondition_registry_text =
        std::fs::read_to_string(repo_root.join(POSTCONDITION_REGISTRY_PATH))
            .map_err(|error| format!("read {POSTCONDITION_REGISTRY_PATH}: {error}"))?;
    render_from_sources(
        &capability_text,
        &error_code_text,
        &postcondition_registry_text,
    )
}

/// Pure catalog renderer used by tests and [`render`].
fn render_from_sources(
    capability_text: &str,
    error_code_text: &str,
    postcondition_registry_text: &str,
) -> Result<String, String> {
    let capability_value = serde_json_lite::parse(capability_text)
        .map_err(|error| format!("capability parse: {error}"))?;
    let error_code_value = serde_json_lite::parse(error_code_text)
        .map_err(|error| format!("error-code parse: {error}"))?;
    let postcondition_registry_value = serde_json_lite::parse(postcondition_registry_text)
        .map_err(|error| format!("postcondition registry parse: {error}"))?;
    let mut capabilities = string_array(&capability_value, "capabilities", "id")?;
    let mut error_codes = string_array(&error_code_value, "categories", "category")?;
    capabilities.sort();
    error_codes.sort();
    ensure_unique(&capabilities, "capability")?;
    ensure_unique(&error_codes, "ErrorCode")?;

    let mut rules = postcondition_rules(&postcondition_registry_value)?;
    rules.sort_by(|left, right| left.kind.cmp(right.kind));
    let mut kinds: Vec<&str> = rules.iter().map(|rule| rule.kind).collect();
    kinds.sort_unstable();
    ensure_unique(&kinds, "postcondition kind")?;

    let mut output = String::new();
    output.push_str("{\n  \"version\": \"1.0\",\n");
    render_string_array(&mut output, "capabilities", &capabilities, 2);
    output.push_str(",\n");
    render_string_array(&mut output, "error_codes", &error_codes, 2);
    output.push_str(",\n  \"postconditions\": [\n");
    for (index, rule) in rules.iter().enumerate() {
        render_postcondition_rule(&mut output, rule);
        if index + 1 != rules.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("  ]\n}\n");
    let generated = serde_json_lite::parse(&output)
        .map_err(|error| format!("generated adapter catalog parse: {error}"))?;
    if let Some(error) = crate::adapter_catalog_check::check_adapter_catalog(&generated) {
        return Err(format!("generated adapter catalog invalid: {error}"));
    }
    Ok(output)
}

fn postcondition_rules(registry: &Value) -> Result<Vec<PostconditionRule<'_>>, String> {
    if registry.get("version").and_then(Value::as_str) != Some("1.0") {
        return Err("postcondition registry version must be 1.0".to_string());
    }
    let postconditions = registry
        .get("postconditions")
        .and_then(Value::as_array)
        .ok_or_else(|| "postcondition registry missing postconditions array".to_string())?;
    if postconditions.is_empty() {
        return Err("postcondition registry postconditions must not be empty".to_string());
    }
    validate_excluded_kinds(registry)?;

    let mut rules = Vec::with_capacity(postconditions.len());
    for (index, entry) in postconditions.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| format!("postcondition registry entry[{index}] must be object"))?;
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
                    "postcondition registry entry[{index}] has unknown field `{field}`"
                ));
            }
        }
        let kind = entry
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("postcondition registry entry[{index}] missing kind"))?;
        if kind.trim().is_empty() {
            return Err(format!(
                "postcondition registry entry[{index}] has empty kind"
            ));
        }
        let mut required = plain_string_array(entry, "required_fields")?;
        let mut allowed = plain_string_array(entry, "allowed_fields")?;
        let mut field_types = plain_string_map(entry, "field_types")?;
        let effects = plain_string_array(entry, "effect_eligibility")?;
        let constraints = entry
            .get("constraints")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("postcondition `{kind}` missing constraints array"))?;
        validate_constraint_tags(kind, constraints)?;

        append_unique(&mut required, "description");
        append_unique(&mut allowed, "description");
        append_unique(&mut required, "error_code");
        append_unique(&mut allowed, "error_code");
        append_map_entry(&mut field_types, "description", "non_empty_string");
        append_map_entry(&mut field_types, "error_code", "ErrorCode");
        if !required.contains(&"kind") || required.iter().any(|field| !allowed.contains(field)) {
            return Err(format!(
                "postcondition `{kind}` required_fields must contain kind and be a subset of allowed_fields"
            ));
        }
        if effects.is_empty() {
            return Err(format!(
                "postcondition `{kind}` effect_eligibility must not be empty"
            ));
        }
        rules.push(PostconditionRule {
            kind,
            required,
            allowed,
            field_types,
            effects,
            constraints: constraints.iter().collect(),
        });
    }
    Ok(rules)
}

fn validate_excluded_kinds(registry: &Value) -> Result<(), String> {
    let excluded = registry
        .get("excluded_kinds")
        .and_then(Value::as_array)
        .ok_or_else(|| "postcondition registry missing excluded_kinds array".to_string())?;
    for (index, entry) in excluded.iter().enumerate() {
        let kind = entry
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("excluded_kinds[{index}] missing kind"))?;
        let reason = entry
            .get("reason")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("excluded_kinds[{index}] missing reason"))?;
        if kind.trim().is_empty() || reason.trim().is_empty() {
            return Err(format!(
                "excluded_kinds[{index}] kind and reason must be non-empty"
            ));
        }
    }
    Ok(())
}

fn validate_constraint_tags(kind: &str, constraints: &[Value]) -> Result<(), String> {
    for (index, constraint) in constraints.iter().enumerate() {
        let tag = constraint
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("postcondition `{kind}` constraint[{index}] missing kind"))?;
        if !matches!(
            tag,
            "field_value_type" | "if_field_equals" | "numeric_less_than_or_equal"
        ) {
            return Err(format!(
                "postcondition `{kind}` constraint[{index}] has unknown constraint kind `{tag}`"
            ));
        }
    }
    Ok(())
}

fn plain_string_array<'a>(value: &'a Value, field: &str) -> Result<Vec<&'a str>, String> {
    let entries = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing `{field}` array"))?;
    let mut result = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let text = entry
            .as_str()
            .ok_or_else(|| format!("`{field}[{index}]` must be string"))?;
        if text.trim().is_empty() {
            return Err(format!("`{field}[{index}]` must be non-empty"));
        }
        if result.contains(&text) {
            return Err(format!("`{field}` contains duplicate `{text}`"));
        }
        result.push(text);
    }
    Ok(result)
}

fn plain_string_map<'a>(value: &'a Value, field: &str) -> Result<Vec<(&'a str, &'a str)>, String> {
    let object = value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing `{field}` object"))?;
    object
        .iter()
        .map(|(key, value)| {
            let text = value
                .as_str()
                .ok_or_else(|| format!("`{field}.{key}` must be string"))?;
            Ok((key.as_str(), text))
        })
        .collect()
}

fn append_unique<'a>(values: &mut Vec<&'a str>, value: &'a str) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn append_map_entry<'a>(values: &mut Vec<(&'a str, &'a str)>, key: &'a str, value: &'a str) {
    if !values.iter().any(|(existing, _)| *existing == key) {
        values.push((key, value));
    }
}

fn string_array(value: &Value, array_field: &str, id_field: &str) -> Result<Vec<String>, String> {
    let entries = value
        .get(array_field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing {array_field} array"))?;
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        let id = entry
            .get(id_field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{array_field} entry missing {id_field}"))?;
        if id.trim().is_empty() {
            return Err(format!("{array_field} entry has blank {id_field}"));
        }
        result.push(id.to_string());
    }
    Ok(result)
}

fn ensure_unique(values: &[impl AsRef<str>], label: &str) -> Result<(), String> {
    for pair in values.windows(2) {
        let Some((left, right)) = pair.first().zip(pair.get(1)) else {
            continue;
        };
        if left.as_ref() == right.as_ref() {
            return Err(format!("duplicate {label}: {}", left.as_ref()));
        }
    }
    Ok(())
}

fn render_string_array(output: &mut String, name: &str, values: &[String], indent: usize) {
    output.push_str(&" ".repeat(indent));
    output.push('"');
    output.push_str(name);
    output.push_str("\": [\n");
    for (index, value) in values.iter().enumerate() {
        output.push_str(&" ".repeat(indent + 2));
        output.push('"');
        output.push_str(&json_escape(value));
        output.push('"');
        if index + 1 != values.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str(&" ".repeat(indent));
    output.push(']');
}

fn render_postcondition_rule(output: &mut String, rule: &PostconditionRule<'_>) {
    output.push_str("    {\n");
    output.push_str("      \"kind\": \"");
    output.push_str(&json_escape(rule.kind));
    output.push_str("\",\n");
    render_slice(output, "required_fields", &rule.required, 6);
    output.push_str(",\n");
    render_slice(output, "allowed_fields", &rule.allowed, 6);
    output.push_str(",\n      \"field_types\": {\n");
    for (index, (field, kind)) in rule.field_types.iter().enumerate() {
        output.push_str("        \"");
        output.push_str(&json_escape(field));
        output.push_str("\": \"");
        output.push_str(&json_escape(kind));
        output.push('"');
        if index + 1 != rule.field_types.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("      },\n");
    render_slice(output, "effect_eligibility", &rule.effects, 6);
    output.push_str(",\n      \"constraints\": [");
    if rule.constraints.is_empty() {
        output.push(']');
    } else {
        output.push('\n');
        for (index, constraint) in rule.constraints.iter().enumerate() {
            output.push_str("        ");
            render_json_value(output, constraint, 8);
            if index + 1 != rule.constraints.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("      ]");
    }
    output.push_str("\n    }");
}

fn render_slice(output: &mut String, name: &str, values: &[&str], indent: usize) {
    output.push_str(&" ".repeat(indent));
    output.push('"');
    output.push_str(name);
    output.push_str("\": [");
    for (index, value) in values.iter().enumerate() {
        if index == 0 {
            output.push(' ');
        }
        output.push('"');
        output.push_str(&json_escape(value));
        output.push('"');
        if index + 1 != values.len() {
            output.push_str(", ");
        }
    }
    if !values.is_empty() {
        output.push(' ');
    }
    output.push(']');
}

fn render_json_value(output: &mut String, value: &Value, indent: usize) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Integer(value) => output.push_str(&value.to_string()),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => {
            output.push('"');
            output.push_str(&json_escape(value));
            output.push('"');
        }
        Value::Array(values) => {
            if values.is_empty() {
                output.push_str("[]");
                return;
            }
            output.push_str("[\n");
            for (index, value) in values.iter().enumerate() {
                output.push_str(&" ".repeat(indent + 2));
                render_json_value(output, value, indent + 2);
                if index + 1 != values.len() {
                    output.push(',');
                }
                output.push('\n');
            }
            output.push_str(&" ".repeat(indent));
            output.push(']');
        }
        Value::Object(values) => {
            if values.is_empty() {
                output.push_str("{}");
                return;
            }
            output.push_str("{\n");
            for (index, (key, value)) in values.iter().enumerate() {
                output.push_str(&" ".repeat(indent + 2));
                output.push('"');
                output.push_str(&json_escape(key));
                output.push_str("\": ");
                render_json_value(output, value, indent + 2);
                if index + 1 != values.len() {
                    output.push(',');
                }
                output.push('\n');
            }
            output.push_str(&" ".repeat(indent));
            output.push('}');
        }
    }
}

fn json_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control <= '\u{1f}' => {
                output.push_str("\\u");
                push_hex_u16(&mut output, control as u16);
            }
            other => output.push(other),
        }
    }
    output
}

fn push_hex_u16(output: &mut String, value: u16) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for shift in [12_u16, 8_u16, 4_u16, 0_u16] {
        let nibble = ((value >> shift) & 0x0f) as usize;
        output.push(HEX.get(nibble).copied().unwrap_or(b'0') as char);
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]
#[path = "adapter_catalog_tests.rs"]
mod tests;
