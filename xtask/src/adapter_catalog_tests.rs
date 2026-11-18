//! Unit tests for the generated Adapter validator catalog.

use super::*;

const CAPABILITY_SOURCE: &str = r#"{"capabilities":[{"id":"z.capability"},{"id":"a.capability"}]}"#;
const ERROR_CODE_SOURCE: &str =
    r#"{"categories":[{"category":"VerifyFailed"},{"category":"Fatal"}]}"#;
const POSTCONDITION_SOURCE: &str = r#"{
  "version": "1.0",
  "postconditions": [
    {
      "kind": "state_assert",
      "required_fields": ["kind", "field", "op", "value"],
      "allowed_fields": ["kind", "field", "op", "value"],
      "field_types": {
        "field": "title|text|fingerprint",
        "op": "equals|not_equals|contains|not_contains",
        "value": "scalar"
      },
      "effect_eligibility": ["read", "write"],
      "constraints": [
        {
          "kind": "field_value_type",
          "field": "field",
          "value_field": "value",
          "cases": {"fingerprint": "fingerprint", "text": "string", "title": "string"}
        },
        {
          "kind": "if_field_equals",
          "field": "field",
          "equals": "fingerprint",
          "then_field": "op",
          "allowed_values": ["equals", "not_equals"]
        }
      ]
    },
    {
      "kind": "value_in_range",
      "required_fields": ["kind", "name", "min", "max"],
      "allowed_fields": ["kind", "name", "min", "max"],
      "field_types": {
        "name": "non_empty_string",
        "min": "number",
        "max": "number"
      },
      "effect_eligibility": ["read", "write"],
      "constraints": [
        {
          "kind": "numeric_less_than_or_equal",
          "left_field": "min",
          "right_field": "max"
        }
      ]
    }
  ],
  "excluded_kinds": [
    {"kind": "visual_assert", "reason": "Adapter catalog 1.0 cannot encode visual variants."}
  ]
}"#;

#[test]
fn test_render_from_sources_sorts_ids_and_merges_envelope_fields() {
    let rendered = render_from_sources(CAPABILITY_SOURCE, ERROR_CODE_SOURCE, POSTCONDITION_SOURCE)
        .expect("catalog render should succeed");
    assert!(rendered.contains("\"a.capability\""));
    assert!(
        rendered.find("\"a.capability\"").expect("a present")
            < rendered.find("\"z.capability\"").expect("z present")
    );
    assert!(
        rendered.find("\"Fatal\"").expect("Fatal present")
            < rendered
                .find("\"VerifyFailed\"")
                .expect("VerifyFailed present")
    );
    assert!(rendered.contains("\"description\""));
    assert!(rendered.contains("\"error_code\""));
    assert!(rendered.contains("\"field_value_type\""));
    assert!(rendered.contains("\"if_field_equals\""));
    assert!(rendered.contains("\"numeric_less_than_or_equal\""));
    assert!(rendered.ends_with("]\n}\n"));
}

#[test]
fn test_render_from_real_registry_copies_constraints_and_read_write_eligibility() {
    let rendered = render_from_sources(
        CAPABILITY_SOURCE,
        ERROR_CODE_SOURCE,
        include_str!("../../crates/verify/src/postcondition-registry-1.0.json"),
    )
    .expect("real registry should render");
    let value = serde_json_lite::parse(&rendered).expect("generated catalog parses");
    let postconditions = value
        .get("postconditions")
        .and_then(Value::as_array)
        .expect("postconditions array");
    let state_unchanged = postconditions
        .iter()
        .find(|entry| entry.get("kind").and_then(Value::as_str) == Some("state_unchanged"))
        .expect("state_unchanged present");
    assert_eq!(
        state_unchanged
            .get("effect_eligibility")
            .and_then(Value::as_array)
            .expect("effects")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["read", "write"]
    );
    assert_eq!(
        state_unchanged
            .get("constraints")
            .and_then(Value::as_array)
            .expect("constraints")
            .len(),
        0
    );
    let value_in_range = postconditions
        .iter()
        .find(|entry| entry.get("kind").and_then(Value::as_str) == Some("value_in_range"))
        .expect("value_in_range present");
    assert_eq!(
        value_in_range
            .get("constraints")
            .and_then(Value::as_array)
            .and_then(|constraints| constraints.first())
            .and_then(|constraint| constraint.get("kind"))
            .and_then(Value::as_str),
        Some("numeric_less_than_or_equal")
    );
}

#[test]
fn test_render_from_sources_rejects_missing_capability_array() {
    let error =
        render_from_sources("{}", ERROR_CODE_SOURCE, POSTCONDITION_SOURCE).expect_err("must fail");
    assert!(error.contains("capabilities"), "{error}");
}

#[test]
fn test_render_from_sources_rejects_duplicate_error_code() {
    let duplicate = r#"{"categories":[{"category":"Fatal"},{"category":"Fatal"}]}"#;
    let error = render_from_sources(CAPABILITY_SOURCE, duplicate, POSTCONDITION_SOURCE)
        .expect_err("must fail");
    assert!(error.contains("duplicate ErrorCode"), "{error}");
}

#[test]
fn test_render_from_sources_rejects_unknown_constraint_tag() {
    let malformed = POSTCONDITION_SOURCE.replace("field_value_type", "unknown_constraint");
    let error = render_from_sources(CAPABILITY_SOURCE, ERROR_CODE_SOURCE, &malformed)
        .expect_err("unknown constraint must fail");
    assert!(error.contains("unknown constraint kind"), "{error}");
}

#[test]
fn test_render_from_sources_rejects_missing_constraints_array() {
    let malformed = POSTCONDITION_SOURCE.replacen("\"constraints\": [", "\"constraint\": [", 1);
    let error = render_from_sources(CAPABILITY_SOURCE, ERROR_CODE_SOURCE, &malformed)
        .expect_err("missing constraints must fail");
    assert!(error.contains("unknown field"), "{error}");
}

#[test]
fn test_render_from_sources_rejects_invalid_constraint_body() {
    let malformed = POSTCONDITION_SOURCE.replace(
        "\"cases\": {\"fingerprint\": \"fingerprint\", \"text\": \"string\", \"title\": \"string\"}",
        "\"cases\": {}",
    );
    let error = render_from_sources(CAPABILITY_SOURCE, ERROR_CODE_SOURCE, &malformed)
        .expect_err("invalid constraint body must fail");
    assert!(error.contains("cases must not be empty"), "{error}");
}

#[test]
fn test_render_from_sources_rejects_invalid_effect_eligibility() {
    let malformed = POSTCONDITION_SOURCE.replacen(
        "\"effect_eligibility\": [\"read\", \"write\"]",
        "\"effect_eligibility\": [\"read\", \"execute\"]",
        1,
    );
    let error = render_from_sources(CAPABILITY_SOURCE, ERROR_CODE_SOURCE, &malformed)
        .expect_err("invalid effect must fail");
    assert!(error.contains("invalid effect"), "{error}");
}

#[test]
fn test_render_slice_escapes_string_values() {
    let mut output = String::new();
    render_slice(&mut output, "fields", &["bad\"field"], 2);
    assert!(output.contains("bad\\\"field"), "{output}");
}
