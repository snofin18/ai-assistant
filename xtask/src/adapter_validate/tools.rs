//! D4 tool envelope and generated-catalog validation.

use std::collections::{BTreeMap, BTreeSet};

use super::tool_bounds::{tool_info, validate_capability_risk_floor};
use super::{ToolInfo, catalog_strings, check_keys, push_error, string_array};
use crate::report::Report;
use crate::serde_json_lite::Value;

const TOOL_FIELDS: [&str; 11] = [
    "version",
    "name",
    "description",
    "input",
    "output",
    "effect",
    "reversibility",
    "risk_level",
    "requires_approval",
    "idempotent",
    "tags",
];
pub(super) fn validate(
    value: &Value,
    catalog: &Value,
    base: &str,
    report: &mut Report,
) -> Vec<ToolInfo> {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/tools",
            format!("{base}/tools/tools.json"),
            "must be object",
        );
        return Vec::new();
    };
    check_keys(
        report,
        "adapter/tools",
        &format!("{base}/tools/tools.json"),
        object,
        &["schema_version", "app_id", "tools"],
    );
    let Some(entries) = object.get("tools").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/tools",
            format!("{base}/tools/tools.json"),
            "tools must be array",
        );
        return Vec::new();
    };
    let capabilities = catalog_strings(catalog, "capabilities");
    let mut names = BTreeSet::new();
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, envelope)| {
            validate_envelope(
                envelope,
                catalog,
                &capabilities,
                &mut names,
                &format!("{base}/tools/tools.json#/tools/{index}"),
                report,
            )
        })
        .collect()
}

fn validate_envelope(
    value: &Value,
    catalog: &Value,
    capabilities: &BTreeSet<String>,
    names: &mut BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) -> Option<ToolInfo> {
    let Some(envelope) = value.as_object() else {
        push_error(
            report,
            "adapter/tools",
            pointer.to_string(),
            "tool envelope must be object",
        );
        return None;
    };
    check_keys(
        report,
        "adapter/tools",
        pointer,
        envelope,
        &["schema", "capabilities", "postconditions"],
    );
    let Some(schema_value) = envelope.get("schema") else {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema"),
            "schema missing",
        );
        return None;
    };
    let Some(schema) = schema_value.as_object() else {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema"),
            "schema must be object",
        );
        return None;
    };
    validate_schema(
        schema,
        envelope.get("capabilities"),
        names,
        &format!("{pointer}/schema"),
        report,
    );
    validate_capabilities(envelope.get("capabilities"), capabilities, pointer, report);
    validate_postconditions(
        envelope.get("postconditions"),
        schema
            .get("effect")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        catalog,
        pointer,
        report,
    );
    Some(tool_info(schema))
}

fn validate_schema(
    schema: &BTreeMap<String, Value>,
    capabilities: Option<&Value>,
    names: &mut BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    check_keys(report, "adapter/tools", pointer, schema, &TOOL_FIELDS);
    for field in TOOL_FIELDS {
        if !schema.contains_key(field) {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/{field}"),
                "required schema field missing",
            );
        }
    }
    validate_schema_identity(schema, names, pointer, report);
    validate_schema_metadata(schema, pointer, report);
    validate_capability_risk_floor(schema, capabilities, pointer, report);
}

fn validate_schema_identity(
    schema: &BTreeMap<String, Value>,
    names: &mut BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let name = schema
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !is_valid_tool_name(name) || !names.insert(name.to_string()) {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/name"),
            "tool name invalid or duplicate",
        );
    }
    let version = schema
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(version, "1.0" | "1.1") {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/version"),
            "unsupported ToolSchema version",
        );
    }
    let effect = schema
        .get("effect")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(effect, "read" | "write") {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/effect"),
            "effect must be read|write",
        );
    }
    let reversibility = schema
        .get("reversibility")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(
        reversibility,
        "l0_undo_stack" | "l1_snapshot" | "l2_compensation" | "l3_irreversible" | "none_readonly"
    ) {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/reversibility"),
            "unknown reversibility",
        );
    }
    if reversibility == "none_readonly" && (version != "1.1" || effect != "read") {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/reversibility"),
            "none_readonly requires ToolSchema 1.1 + read",
        );
    }
    let risk_level = schema
        .get("risk_level")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(risk_level, "low" | "medium" | "high" | "critical") {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/risk_level"),
            "risk_level must be low|medium|high|critical",
        );
    }
}

fn validate_schema_metadata(schema: &BTreeMap<String, Value>, pointer: &str, report: &mut Report) {
    for field in ["requires_approval", "idempotent"] {
        if schema.get(field).and_then(Value::as_bool).is_none() {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/{field}"),
                "must be explicit bool",
            );
        }
    }
    validate_tags(schema.get("tags"), pointer, report);
    if schema
        .get("description")
        .and_then(Value::as_str)
        .is_none_or(|text| text.trim().is_empty())
    {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/description"),
            "must be non-empty string",
        );
    }
    for field in ["input", "output"] {
        if schema.get(field).and_then(Value::as_object).is_none() {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/{field}"),
                "must be object",
            );
        }
    }
}

fn validate_tags(value: Option<&Value>, pointer: &str, report: &mut Report) {
    let Some(tags) = value.and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/tags"),
            "must be array",
        );
        return;
    };
    for (index, tag) in tags.iter().enumerate() {
        if !tag.as_str().is_some_and(is_valid_snake_identifier) {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/tags/{index}"),
                "tag must match ^[a-z][a-z0-9_]*$",
            );
        }
    }
}

fn is_valid_tool_name(name: &str) -> bool {
    let mut segments = name.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };
    let Some(third) = segments.next() else {
        return false;
    };
    segments.next().is_none()
        && [first, second, third]
            .into_iter()
            .all(is_valid_snake_identifier)
}

fn is_valid_snake_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn validate_capabilities(
    value: Option<&Value>,
    catalog: &BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(capabilities) = value.and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/capabilities"),
            "must be array",
        );
        return;
    };
    if capabilities.is_empty() {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/capabilities"),
            "must be non-empty",
        );
    }
    let mut seen = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for capability in capabilities {
        let Some(capability) = capability.as_str() else {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/capabilities"),
                "entries must be strings",
            );
            continue;
        };
        if capability.trim().is_empty() {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/capabilities"),
                "capability id must be non-empty",
            );
        }
        if previous.is_some_and(|value| value >= capability) {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/capabilities"),
                "capabilities must be strictly byte-order sorted",
            );
        }
        previous = Some(capability);
        if !catalog.contains(capability) {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/capabilities"),
                format!("unknown capability `{capability}`"),
            );
        }
        if !seen.insert(capability.to_string()) {
            push_error(
                report,
                "adapter/tools",
                format!("{pointer}/capabilities"),
                format!("duplicate capability `{capability}`"),
            );
        }
    }
}

fn validate_postconditions(
    value: Option<&Value>,
    effect: &str,
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) {
    let Some(postconditions) = value.and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/postconditions"),
            "must be array",
        );
        return;
    };
    if postconditions.is_empty() {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/postconditions"),
            "must be non-empty",
        );
    }
    let mut has_read = false;
    let mut has_write = false;
    let mut canonical = BTreeSet::new();
    for (index, postcondition) in postconditions.iter().enumerate() {
        let post_pointer = format!("{pointer}/postconditions/{index}");
        if !canonical.insert(format!("{postcondition:?}")) {
            push_error(
                report,
                "adapter/tools",
                post_pointer.clone(),
                "duplicate canonical postcondition",
            );
        }
        if let Some(kind) =
            validate_postcondition(postcondition, effect, catalog, &post_pointer, report)
        {
            has_read |= kind == "state_unchanged";
            has_write |= kind != "state_unchanged";
        }
    }
    if effect == "read" && !has_read {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/postconditions"),
            "read tool requires state_unchanged",
        );
    }
    if effect == "write" && !has_write {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/postconditions"),
            "write tool requires executable postcondition",
        );
    }
}

pub(super) fn validate_postcondition(
    value: &Value,
    effect: &str,
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) -> Option<String> {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            "must be object",
        );
        return None;
    };
    let Some(kind) = object.get("kind").and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            "kind missing",
        );
        return None;
    };
    let Some(rule) = find_rule(catalog, kind) else {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            format!("unknown kind `{kind}`"),
        );
        return None;
    };
    validate_required_postcondition_fields(object, rule, pointer, report);
    if !validate_postcondition_effect(object, rule, effect, kind, pointer, report) {
        return None;
    }
    validate_description_and_error_code(object, catalog, pointer, report);
    validate_catalog_field_types(object, rule, catalog, pointer, report);
    validate_catalog_constraints(object, rule, catalog, pointer, report);
    Some(kind.to_string())
}

fn validate_required_postcondition_fields(
    object: &BTreeMap<String, Value>,
    rule: &Value,
    pointer: &str,
    report: &mut Report,
) {
    for field in string_array(rule.get("required_fields")) {
        if !object.contains_key(field) {
            push_error(
                report,
                "adapter/postcondition",
                pointer.to_string(),
                format!("missing `{field}`"),
            );
        }
    }
}

fn validate_postcondition_effect(
    object: &BTreeMap<String, Value>,
    rule: &Value,
    effect: &str,
    kind: &str,
    pointer: &str,
    report: &mut Report,
) -> bool {
    let effects = string_array(rule.get("effect_eligibility"));
    if !effects.contains(&effect) {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            format!("kind `{kind}` is not eligible for effect `{effect}`"),
        );
        return false;
    }
    let allowed = string_array(rule.get("allowed_fields"));
    for field in object.keys() {
        if !allowed.contains(&field.as_str()) {
            push_error(
                report,
                "adapter/postcondition",
                pointer.to_string(),
                format!("unknown field `{field}`"),
            );
        }
    }
    true
}

fn validate_description_and_error_code(
    object: &BTreeMap<String, Value>,
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) {
    if object
        .get("description")
        .and_then(Value::as_str)
        .is_none_or(|text| text.trim().is_empty())
    {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            "description must be non-empty",
        );
    }
    if !catalog_strings(catalog, "error_codes").contains(
        object
            .get("error_code")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    ) {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            "error_code must hit catalog",
        );
    }
}

fn validate_catalog_field_types(
    object: &BTreeMap<String, Value>,
    rule: &Value,
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) {
    if let Some(field_types) = rule.get("field_types").and_then(Value::as_object) {
        for (field, field_type) in field_types {
            let Some(value) = object.get(field) else {
                continue;
            };
            let Some(field_type) = field_type.as_str() else {
                push_error(
                    report,
                    "adapter/postcondition",
                    pointer.to_string(),
                    format!("catalog field type for `{field}` must be string"),
                );
                continue;
            };
            if !super::catalog_value_matches(value, field_type, catalog) {
                push_error(
                    report,
                    "adapter/postcondition",
                    format!("{pointer}/{field}"),
                    format!("does not satisfy catalog type `{field_type}`"),
                );
            }
        }
    }
}

fn validate_catalog_constraints(
    object: &BTreeMap<String, Value>,
    rule: &Value,
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) {
    let Some(constraints) = rule.get("constraints").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            "catalog postcondition missing constraints array",
        );
        return;
    };
    for (index, constraint) in constraints.iter().enumerate() {
        let context = format!("{pointer}/constraints/{index}");
        validate_catalog_constraint(object, rule, constraint, catalog, &context, report);
    }
}

fn validate_catalog_constraint(
    object: &BTreeMap<String, Value>,
    rule: &Value,
    constraint: &Value,
    catalog: &Value,
    context: &str,
    report: &mut Report,
) {
    let Some(tag) = constraint.get("kind").and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/postcondition",
            context.to_string(),
            "catalog constraint missing kind",
        );
        return;
    };
    match tag {
        "field_value_type" => {
            validate_field_value_type_constraint(object, constraint, catalog, context, report);
        }
        "if_field_equals" => {
            validate_if_field_equals_constraint(object, constraint, rule, context, report);
        }
        "numeric_less_than_or_equal" => {
            super::validate_numeric_constraint(object, constraint, context, report);
        }
        other => push_error(
            report,
            "adapter/postcondition",
            context.to_string(),
            format!("unknown catalog constraint kind `{other}`"),
        ),
    }
}

fn validate_field_value_type_constraint(
    object: &BTreeMap<String, Value>,
    constraint: &Value,
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) {
    let field = constraint
        .get("field")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let value_field = constraint
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let case = object.get(field).and_then(Value::as_str);
    let case_type = case
        .and_then(|case| {
            constraint
                .get("cases")
                .and_then(|cases| cases.get(case))
                .and_then(Value::as_str)
        })
        .unwrap_or_default();
    if case_type.is_empty() {
        push_error(
            report,
            "adapter/postcondition",
            format!("{pointer}/{field}"),
            "has no catalog field_value_type case",
        );
        return;
    }
    if !object
        .get(value_field)
        .is_some_and(|value| super::catalog_value_matches(value, case_type, catalog))
    {
        push_error(
            report,
            "adapter/postcondition",
            format!("{pointer}/{value_field}"),
            format!("violates catalog constraint type `{case_type}`"),
        );
    }
}

fn validate_if_field_equals_constraint(
    object: &BTreeMap<String, Value>,
    constraint: &Value,
    rule: &Value,
    pointer: &str,
    report: &mut Report,
) {
    let Some(metadata) = resolve_if_field_equals_metadata(constraint, rule, pointer, report) else {
        return;
    };
    if object.get(metadata.field).and_then(Value::as_str) != Some(metadata.equals) {
        return;
    }
    let actual = object.get(metadata.then_field).and_then(Value::as_str);
    if !actual.is_some_and(|value| metadata.allowed_values.contains(&value)) {
        push_error(
            report,
            "adapter/postcondition",
            format!("{pointer}/{}", metadata.then_field),
            format!(
                "violates catalog constraint `{}={}`",
                metadata.field, metadata.equals
            ),
        );
    }
}

struct IfFieldEqualsMetadata<'a> {
    field: &'a str,
    equals: &'a str,
    then_field: &'a str,
    allowed_values: Vec<&'a str>,
}

fn resolve_if_field_equals_metadata<'a>(
    constraint: &'a Value,
    rule: &'a Value,
    pointer: &str,
    report: &mut Report,
) -> Option<IfFieldEqualsMetadata<'a>> {
    let field = constraint_required_str(constraint, "field", pointer, report)?;
    let equals = constraint_required_str(constraint, "equals", pointer, report)?;
    let then_field = constraint_required_str(constraint, "then_field", pointer, report)?;
    let allowed_fields = string_array(rule.get("allowed_fields"));
    if !allowed_fields.contains(&field) || !allowed_fields.contains(&then_field) {
        push_constraint_error(report, pointer, "references undeclared field");
        return None;
    }
    let Some(field_values) = declared_union_values(rule, field) else {
        push_constraint_error(report, pointer, "requires a union discriminator");
        return None;
    };
    if !field_values.contains(&equals) {
        push_constraint_error(report, pointer, "has undeclared equals value");
        return None;
    }
    let allowed_values = constraint_string_array(constraint, "allowed_values", pointer, report)?;
    if let Some(then_values) = declared_union_values(rule, then_field)
        && allowed_values
            .iter()
            .any(|value| !then_values.contains(value))
    {
        push_constraint_error(report, pointer, "has undeclared allowed value");
        return None;
    }
    Some(IfFieldEqualsMetadata {
        field,
        equals,
        then_field,
        allowed_values,
    })
}

fn constraint_required_str<'a>(
    constraint: &'a Value,
    field: &str,
    pointer: &str,
    report: &mut Report,
) -> Option<&'a str> {
    let value = constraint_non_empty_str(constraint, field);
    if value.is_none() {
        push_constraint_error(report, pointer, &format!("missing {field}"));
    }
    value
}

fn constraint_string_array<'a>(
    constraint: &'a Value,
    field: &str,
    pointer: &str,
    report: &mut Report,
) -> Option<Vec<&'a str>> {
    let Some(values) = constraint.get(field).and_then(Value::as_array) else {
        push_constraint_error(report, pointer, &format!("{field} must be array"));
        return None;
    };
    let result: Vec<&str> = values.iter().filter_map(Value::as_str).collect();
    if result.is_empty() || result.len() != values.len() {
        push_constraint_error(report, pointer, &format!("has invalid {field}"));
        return None;
    }
    Some(result)
}

fn push_constraint_error(report: &mut Report, pointer: &str, detail: &str) {
    push_error(
        report,
        "adapter/postcondition",
        pointer.to_string(),
        format!("catalog constraint `if_field_equals` {detail}"),
    );
}

fn constraint_non_empty_str<'a>(constraint: &'a Value, field: &str) -> Option<&'a str> {
    constraint
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn declared_union_values<'a>(rule: &'a Value, field: &str) -> Option<Vec<&'a str>> {
    let declaration = rule
        .get("field_types")
        .and_then(Value::as_object)
        .and_then(|field_types| field_types.get(field))
        .and_then(Value::as_str)?;
    if !declaration.contains('|') {
        return None;
    }
    Some(declaration.split('|').collect())
}

fn find_rule<'a>(catalog: &'a Value, kind: &str) -> Option<&'a Value> {
    catalog
        .get("postconditions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|rule| rule.get("kind").and_then(Value::as_str) == Some(kind))
}
