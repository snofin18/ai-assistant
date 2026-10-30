use super::*;

fn field(selector: &str) -> FieldSpec {
    FieldSpec {
        selector: selector.to_owned(),
    }
}

#[test]
fn test_form_read_expression_is_read_only() {
    let expression = form_read_expression("#form", &[field("#name")]);
    assert!(expression.is_ok());
    if let Ok(expression) = expression {
        assert!(!expression.contains(".click("));
        assert!(!expression.contains(".submit("));
        assert!(!expression.contains(".requestSubmit("));
        assert!(expression.contains("querySelectorAll"));
    }
}

#[test]
fn test_parse_fields_rejects_duplicates_and_values() {
    let duplicate = Map::from_iter([(
        "fields".to_owned(),
        json!([{"selector":"#name"},{"selector":"#name"}]),
    )]);
    assert!(parse_fields(&duplicate, "edge.cdp.form_read").is_err());
    let extra_value = Map::from_iter([(
        "fields".to_owned(),
        json!([{"selector":"#name","value":"unexpected"}]),
    )]);
    assert!(parse_fields(&extra_value, "edge.cdp.form_read").is_err());
}

#[test]
fn test_form_read_payload_rejects_unrequested_field() {
    let fields = [field("#name")];
    let payload = FormReadPayload {
        error: None,
        selector: None,
        message: None,
        origin: "http://127.0.0.1:43000".to_owned(),
        action: "http://127.0.0.1:43000/submitted".to_owned(),
        fields: vec![FieldSnapshot {
            selector: "#other".to_owned(),
            value: "Ada".to_owned(),
        }],
    };
    assert!(validate_form_read_payload(&fields, &payload, "edge.cdp.form_read").is_err());
}
