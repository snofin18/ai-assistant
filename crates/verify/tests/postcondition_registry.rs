//! Contract tests for the embedded Adapter postcondition registry.

use std::collections::{BTreeMap, BTreeSet};

use assistant_protocol::serde_json::{Map, Value, json};
use assistant_verify::{POSTCONDITION_REGISTRY_JSON, VerifyError, parse_postconditions};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const FIELD_TYPE_VALUES: [&str; 10] = [
    "string",
    "optional_string",
    "non_empty_string",
    "scalar",
    "number",
    "positive_integer",
    "fingerprint",
    "any|size|mtime|digest",
    "title|text|fingerprint",
    "equals|not_equals|contains|not_contains",
];

#[derive(Clone, Copy)]
struct ExpectedRule {
    kind: &'static str,
    required: &'static [&'static str],
    allowed: &'static [&'static str],
    field_types: &'static [(&'static str, &'static str)],
    effects: &'static [&'static str],
    constraints: &'static [ExpectedConstraint],
}

#[derive(Clone, Copy)]
enum ExpectedConstraint {
    FieldValueType {
        field: &'static str,
        value_field: &'static str,
        cases: &'static [(&'static str, &'static str)],
    },
    IfFieldEquals {
        field: &'static str,
        equals: &'static str,
        then_field: &'static str,
        allowed_values: &'static [&'static str],
    },
    NumericLessThanOrEqual {
        left_field: &'static str,
        right_field: &'static str,
    },
}

const EXPECTED_RULES: [ExpectedRule; 11] = [
    ExpectedRule {
        kind: "app_reported",
        required: &["kind", "key", "value"],
        allowed: &["kind", "key", "value"],
        field_types: &[("key", "non_empty_string"), ("value", "scalar")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "element_exists",
        required: &["kind", "selector"],
        allowed: &["kind", "selector"],
        field_types: &[("selector", "non_empty_string")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "element_gone",
        required: &["kind", "selector"],
        allowed: &["kind", "selector"],
        field_types: &[("selector", "non_empty_string")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "file_changed",
        required: &["kind", "path", "expect"],
        allowed: &["kind", "path", "expect"],
        field_types: &[
            ("expect", "any|size|mtime|digest"),
            ("path", "non_empty_string"),
        ],
        effects: &["write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "state_assert",
        required: &["kind", "field", "op", "value"],
        allowed: &["kind", "field", "op", "value"],
        field_types: &[
            ("field", "title|text|fingerprint"),
            ("op", "equals|not_equals|contains|not_contains"),
            ("value", "scalar"),
        ],
        effects: &["read", "write"],
        constraints: &[
            ExpectedConstraint::FieldValueType {
                field: "field",
                value_field: "value",
                cases: &[
                    ("fingerprint", "fingerprint"),
                    ("text", "string"),
                    ("title", "string"),
                ],
            },
            ExpectedConstraint::IfFieldEquals {
                field: "field",
                equals: "fingerprint",
                then_field: "op",
                allowed_values: &["equals", "not_equals"],
            },
        ],
    },
    ExpectedRule {
        kind: "state_changed",
        required: &["kind", "within_ms"],
        allowed: &["kind", "fingerprint_scope", "within_ms"],
        field_types: &[
            ("fingerprint_scope", "optional_string"),
            ("within_ms", "positive_integer"),
        ],
        effects: &["write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "state_unchanged",
        required: &["kind"],
        allowed: &["kind", "fingerprint_scope"],
        field_types: &[("fingerprint_scope", "optional_string")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "text_contains",
        required: &["kind", "value"],
        allowed: &["kind", "value"],
        field_types: &[("value", "string")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "text_not_contains",
        required: &["kind", "value"],
        allowed: &["kind", "value"],
        field_types: &[("value", "string")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "value_equals",
        required: &["kind", "name", "value"],
        allowed: &["kind", "name", "value"],
        field_types: &[("name", "non_empty_string"), ("value", "scalar")],
        effects: &["read", "write"],
        constraints: &[],
    },
    ExpectedRule {
        kind: "value_in_range",
        required: &["kind", "name", "min", "max"],
        allowed: &["kind", "name", "min", "max"],
        field_types: &[
            ("max", "number"),
            ("min", "number"),
            ("name", "non_empty_string"),
        ],
        effects: &["read", "write"],
        constraints: &[ExpectedConstraint::NumericLessThanOrEqual {
            left_field: "min",
            right_field: "max",
        }],
    },
];

#[test]
fn test_registry_every_kind_is_accepted_by_verify_parser() -> TestResult {
    let registry: Value = assistant_protocol::serde_json::from_str(POSTCONDITION_REGISTRY_JSON)?;
    let postconditions = registry
        .get("postconditions")
        .and_then(Value::as_array)
        .ok_or("registry.postconditions must be an array")?;
    assert_eq!(postconditions.len(), 11);

    let expected_kinds: BTreeSet<String> = [
        "app_reported",
        "element_exists",
        "element_gone",
        "file_changed",
        "state_assert",
        "state_changed",
        "state_unchanged",
        "text_contains",
        "text_not_contains",
        "value_equals",
        "value_in_range",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut kinds = BTreeSet::new();
    for entry in postconditions {
        let kind = entry
            .get("kind")
            .and_then(Value::as_str)
            .ok_or("postcondition kind must be a string")?;
        assert!(kinds.insert(kind.to_owned()), "duplicate kind: {kind}");
        let sample = sample_for_entry(entry)?;
        parse_postconditions(&[sample])?;
    }
    assert_eq!(kinds, expected_kinds);
    Ok(())
}

#[test]
fn test_registry_shape_is_closed_and_deterministic() -> TestResult {
    let registry: Value = assistant_protocol::serde_json::from_str(POSTCONDITION_REGISTRY_JSON)?;
    assert_eq!(registry.get("version").and_then(Value::as_str), Some("1.0"));
    let postconditions = registry
        .get("postconditions")
        .and_then(Value::as_array)
        .ok_or("registry.postconditions must be an array")?;
    let top_level: BTreeSet<&str> = registry
        .as_object()
        .ok_or("registry root must be object")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top_level,
        BTreeSet::from(["excluded_kinds", "postconditions", "version"])
    );

    let mut previous_kind: Option<&str> = None;
    for entry in postconditions {
        let entry_fields: BTreeSet<&str> = entry
            .as_object()
            .ok_or("postcondition entry must be object")?
            .keys()
            .map(String::as_str)
            .collect();
        let allowed_entry_fields = BTreeSet::from([
            "allowed_fields",
            "constraints",
            "effect_eligibility",
            "field_types",
            "kind",
            "required_fields",
        ]);
        assert!(entry_fields.is_subset(&allowed_entry_fields));
        for required in [
            "allowed_fields",
            "constraints",
            "effect_eligibility",
            "field_types",
            "kind",
            "required_fields",
        ] {
            assert!(entry_fields.contains(required), "entry missing {required}");
        }
        let kind = entry
            .get("kind")
            .and_then(Value::as_str)
            .ok_or("postcondition kind must be a string")?;
        if let Some(previous) = previous_kind {
            assert!(previous < kind, "postconditions must be byte-order sorted");
        }
        previous_kind = Some(kind);

        assert_allowed_fields(entry, kind)?;
        assert_unique_strings(entry, kind, "effect_eligibility")?;
        assert_sorted_unique(entry, kind, "effect_eligibility")?;
        let effects = string_array(entry, "effect_eligibility")?;
        assert!(!effects.is_empty(), "{kind} must have effect eligibility");
        assert!(
            effects
                .iter()
                .all(|effect| matches!(*effect, "read" | "write")),
            "invalid effect eligibility for {kind}"
        );
    }
    Ok(())
}

#[test]
fn test_registry_exact_field_type_effect_matrix() -> TestResult {
    let registry: Value = assistant_protocol::serde_json::from_str(POSTCONDITION_REGISTRY_JSON)?;
    let postconditions = registry
        .get("postconditions")
        .and_then(Value::as_array)
        .ok_or("registry.postconditions must be an array")?;
    assert_eq!(postconditions.len(), EXPECTED_RULES.len());
    for (entry, expected) in postconditions.iter().zip(EXPECTED_RULES) {
        assert_eq!(
            entry.get("kind").and_then(Value::as_str),
            Some(expected.kind)
        );
        assert_eq!(string_array(entry, "required_fields")?, expected.required);
        assert_eq!(string_array(entry, "allowed_fields")?, expected.allowed);
        assert_eq!(string_array(entry, "effect_eligibility")?, expected.effects);
        let field_types = entry
            .get("field_types")
            .and_then(Value::as_object)
            .ok_or("field_types must be object")?;
        let actual_types: Vec<(&str, &str)> = field_types
            .iter()
            .map(|(field, value)| {
                Ok((
                    field.as_str(),
                    value.as_str().ok_or("field type value must be string")?,
                ))
            })
            .collect::<Result<_, Box<dyn std::error::Error>>>()?;
        assert_eq!(actual_types, expected.field_types);
        let constraints = entry
            .get("constraints")
            .and_then(Value::as_array)
            .ok_or("constraints must be array")?;
        assert_eq!(constraints.len(), expected.constraints.len());
        for (actual, expected) in constraints.iter().zip(expected.constraints) {
            assert_expected_constraint(actual, *expected)?;
        }
    }
    Ok(())
}

#[test]
fn test_registry_unknown_kind_keeps_unsupported_error_variant() -> TestResult {
    let error = parse_postconditions(&[json!({"kind": "not_a_postcondition"})])
        .err()
        .ok_or("unknown kind must fail")?;
    assert!(matches!(
        error,
        VerifyError::UnsupportedPostconditionKind { .. }
    ));
    Ok(())
}

#[test]
fn test_registry_constraints_are_executed_before_parser_semantics() -> TestResult {
    let fingerprint_contains = parse_postconditions(&[json!({
        "kind": "state_assert",
        "field": "fingerprint",
        "op": "contains",
        "value": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    })])
    .err()
    .ok_or("fingerprint + contains must fail")?;
    assert!(
        fingerprint_contains
            .to_string()
            .contains("postcondition registry"),
        "{fingerprint_contains}"
    );

    let title_number = parse_postconditions(&[json!({
        "kind": "state_assert",
        "field": "title",
        "op": "equals",
        "value": 1
    })])
    .err()
    .ok_or("title + number must fail")?;
    assert!(
        title_number.to_string().contains("postcondition registry"),
        "{title_number}"
    );

    let inverted_range = parse_postconditions(&[json!({
        "kind": "value_in_range",
        "name": "amount",
        "min": 2,
        "max": 1
    })])
    .err()
    .ok_or("inverted range must fail")?;
    assert!(
        inverted_range
            .to_string()
            .contains("postcondition registry"),
        "{inverted_range}"
    );
    Ok(())
}

fn assert_expected_constraint(actual: &Value, expected: ExpectedConstraint) -> TestResult {
    match expected {
        ExpectedConstraint::FieldValueType {
            field,
            value_field,
            cases,
        } => {
            assert_eq!(
                actual.get("kind").and_then(Value::as_str),
                Some("field_value_type")
            );
            assert_eq!(actual.get("field").and_then(Value::as_str), Some(field));
            assert_eq!(
                actual.get("value_field").and_then(Value::as_str),
                Some(value_field)
            );
            let actual_cases = actual
                .get("cases")
                .and_then(Value::as_object)
                .ok_or("field_value_type cases must be object")?;
            let expected_cases: BTreeMap<&str, &str> = cases.iter().copied().collect();
            let actual_cases: BTreeMap<&str, &str> = actual_cases
                .iter()
                .map(|(key, value)| {
                    let field_type = value
                        .as_str()
                        .ok_or("constraint case type must be string")?;
                    if !FIELD_TYPE_VALUES.contains(&field_type) {
                        return Err(
                            format!("unsupported constraint case type `{field_type}`").into()
                        );
                    }
                    Ok((key.as_str(), field_type))
                })
                .collect::<Result<_, Box<dyn std::error::Error>>>()?;
            assert_eq!(actual_cases, expected_cases);
        }
        ExpectedConstraint::IfFieldEquals {
            field,
            equals,
            then_field,
            allowed_values,
        } => {
            assert_eq!(
                actual.get("kind").and_then(Value::as_str),
                Some("if_field_equals")
            );
            assert_eq!(actual.get("field").and_then(Value::as_str), Some(field));
            assert_eq!(actual.get("equals").and_then(Value::as_str), Some(equals));
            assert_eq!(
                actual.get("then_field").and_then(Value::as_str),
                Some(then_field)
            );
            assert_eq!(string_array(actual, "allowed_values")?, allowed_values);
        }
        ExpectedConstraint::NumericLessThanOrEqual {
            left_field,
            right_field,
        } => {
            assert_eq!(
                actual.get("kind").and_then(Value::as_str),
                Some("numeric_less_than_or_equal")
            );
            assert_eq!(
                actual.get("left_field").and_then(Value::as_str),
                Some(left_field)
            );
            assert_eq!(
                actual.get("right_field").and_then(Value::as_str),
                Some(right_field)
            );
        }
    }
    Ok(())
}

#[test]
fn test_registry_documents_visual_assert_as_excluded() -> TestResult {
    let registry: Value = assistant_protocol::serde_json::from_str(POSTCONDITION_REGISTRY_JSON)?;
    let excluded = registry
        .get("excluded_kinds")
        .and_then(Value::as_array)
        .ok_or("registry.excluded_kinds must be an array")?;
    let visual = excluded
        .iter()
        .find(|entry| entry.get("kind").and_then(Value::as_str) == Some("visual_assert"))
        .ok_or("visual_assert must be explicitly excluded")?;
    assert!(
        visual
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty())
    );
    assert!(
        !registry
            .get("postconditions")
            .and_then(Value::as_array)
            .is_some_and(|entries| entries
                .iter()
                .any(|entry| entry.get("kind").and_then(Value::as_str) == Some("visual_assert"))),
        "visual_assert must not be listed as Adapter-supported"
    );
    Ok(())
}

#[test]
fn test_registry_file_change_enum_accepts_every_declared_value() -> TestResult {
    for expect in ["any", "size", "mtime", "digest"] {
        parse_postconditions(&[json!({
            "kind": "file_changed",
            "path": "output.txt",
            "expect": expect,
        })])?;
    }
    Ok(())
}

#[test]
fn test_registry_text_value_type_matches_parser_empty_string_semantics() -> TestResult {
    parse_postconditions(&[json!({"kind": "text_contains", "value": ""})])?;
    parse_postconditions(&[json!({"kind": "text_not_contains", "value": ""})])?;
    Ok(())
}

#[test]
fn test_registry_state_unchanged_remains_eligible_for_write() -> TestResult {
    let registry: Value = assistant_protocol::serde_json::from_str(POSTCONDITION_REGISTRY_JSON)?;
    let state_unchanged = registry
        .get("postconditions")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.get("kind").and_then(Value::as_str) == Some("state_unchanged"))
        })
        .ok_or("state_unchanged missing")?;
    assert_eq!(
        string_array(state_unchanged, "effect_eligibility")?,
        vec!["read", "write"]
    );
    Ok(())
}

fn sample_for_entry(entry: &Value) -> TestResult<Value> {
    let kind = entry
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("postcondition kind must be a string")?;
    let field_types = entry
        .get("field_types")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{kind}.field_types must be an object"))?;
    for (field, field_type) in field_types {
        let field_type = field_type
            .as_str()
            .ok_or_else(|| format!("{kind}.{field} type must be a string"))?;
        assert!(
            FIELD_TYPE_VALUES.contains(&field_type),
            "{kind}.{field} uses unsupported field type `{field_type}`"
        );
    }
    let mut object = Map::new();
    object.insert("kind".to_owned(), Value::String(kind.to_owned()));
    for (field, field_type) in field_types {
        let field_type = field_type
            .as_str()
            .ok_or_else(|| format!("{kind}.{field} type must be a string"))?;
        object.insert(field.clone(), sample_value(kind, field, field_type)?);
    }
    Ok(Value::Object(object))
}

fn sample_value(kind: &str, field: &str, field_type: &str) -> TestResult<Value> {
    let value = match (field, field_type) {
        ("value", "scalar") if kind == "state_assert" => json!("expected text"),
        ("field", "title|text|fingerprint") => json!("title"),
        ("op", "equals|not_equals|contains|not_contains") => json!("equals"),
        ("expect", "any|size|mtime|digest") => json!("size"),
        ("within_ms", "positive_integer") => json!(1000),
        (_, "string" | "non_empty_string") => json!(format!("{field}-value")),
        (_, "optional_string") => json!(format!("{field}-scope")),
        (_, "positive_integer" | "number") => json!(1),
        (_, "scalar") => json!(true),
        _ => {
            return Err(format!("unsupported registry field type: {field}={field_type}").into());
        }
    };
    Ok(value)
}

fn assert_allowed_fields(entry: &Value, kind: &str) -> TestResult {
    let required = string_array(entry, "required_fields")?;
    let allowed = string_array(entry, "allowed_fields")?;
    assert!(
        required.contains(&"kind"),
        "{kind}.required_fields lacks kind"
    );
    assert_unique_strings(entry, kind, "required_fields")?;
    assert_unique_strings(entry, kind, "allowed_fields")?;
    assert!(
        required.iter().all(|field| allowed.contains(field)),
        "{kind} required_fields must be a subset of allowed_fields"
    );

    let field_types = entry
        .get("field_types")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{kind}.field_types must be an object"))?;
    let expected: BTreeSet<&str> = allowed
        .iter()
        .copied()
        .filter(|field| *field != "kind")
        .collect();
    let actual: BTreeSet<&str> = field_types.keys().map(String::as_str).collect();
    assert_eq!(
        actual, expected,
        "{kind}.field_types keys must equal allowed_fields minus kind"
    );
    Ok(())
}

fn assert_sorted_unique(entry: &Value, kind: &str, field: &str) -> TestResult {
    let values = string_array(entry, field)?;
    assert!(
        values
            .windows(2)
            .all(|window| window.first() < window.get(1)),
        "{kind}.{field} must be unique and byte-order sorted"
    );
    Ok(())
}

fn assert_unique_strings(entry: &Value, kind: &str, field: &str) -> TestResult {
    let values = string_array(entry, field)?;
    let unique: BTreeSet<&str> = values.iter().copied().collect();
    assert_eq!(
        values.len(),
        unique.len(),
        "{kind}.{field} entries must be unique"
    );
    Ok(())
}

fn string_array<'a>(entry: &'a Value, field: &str) -> Result<Vec<&'a str>, String> {
    let values = entry
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{field} must be an array"))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        result.push(
            value
                .as_str()
                .ok_or_else(|| format!("{field} entries must be strings"))?,
        );
    }
    Ok(result)
}
