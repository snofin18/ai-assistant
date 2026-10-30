//! Direct tests for the bounded, read-only form snapshot handler.
use super::*;

fn field(selector: &str) -> String {
    selector.to_owned()
}

fn arguments(fields: Value) -> Map<String, Value> {
    Map::from_iter([("fields".to_owned(), fields)])
}

fn error_code(input: &str) -> Option<ErrorCode> {
    let payload: FormReadPayload = serde_json::from_str(input).ok()?;
    validate_form_read_payload(&[field("#name")], &payload, "edge.cdp.form_read")
        .err()
        .map(|error| error.error_code())
}

#[test]
fn test_form_read_expression_is_read_only() {
    let expression = form_read_expression("#form", &[field("#name")]).unwrap_or_default();
    assert!(!expression.contains(".click("));
    assert!(!expression.contains(".submit("));
    assert!(!expression.contains(".requestSubmit("));
    assert!(expression.contains("querySelectorAll"));
}

#[test]
fn test_parse_fields_rejects_duplicates_and_values() {
    let duplicate = arguments(json!([{"selector":"#name"},{"selector":"#name"}]));
    assert!(parse_fields(&duplicate, "edge.cdp.form_read").is_err());
    let extra_value = arguments(json!([{"selector":"#name","value":"unexpected"}]));
    assert!(parse_fields(&extra_value, "edge.cdp.form_read").is_err());
}

#[test]
fn test_form_read_payload_rejects_unrequested_field() {
    let input = r##"{"origin":"http://127.0.0.1:43000","action":"http://127.0.0.1:43000/submitted","fields":[{"selector":"#other","value":"Ada"}]}"##;
    assert!(error_code(input).is_some());
}

#[test]
fn test_form_read_error_payloads_map_before_success_fields() {
    const NOT_FOUND: &str = r##"{"error":"not_found","selector":"#missing"}"##;
    const INVALID_ACTION: &str = r##"{"error":"invalid_action","selector":"#form"}"##;
    for (input, expected) in [
        (NOT_FOUND, ErrorCode::TargetNotFound),
        (INVALID_ACTION, ErrorCode::ToolInvalidArgs),
    ] {
        assert_eq!(error_code(input), Some(expected));
    }
}
