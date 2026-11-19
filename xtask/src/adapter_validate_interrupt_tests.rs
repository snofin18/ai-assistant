//! Unit tests for interrupt match union validation.
//!
//! Kept external so `adapter_validate_tests.rs` stays below the file hard limit.

use super::*;

fn findings(source: &str) -> Vec<String> {
    let value = crate::serde_json_lite::parse(source).expect("parse interrupt fixture");
    let mut report = Report::new("adapter-validate");
    validate(&value, &[], "adapters/test", &mut report);
    report
        .findings
        .into_iter()
        .map(|finding| finding.message)
        .collect()
}

#[test]
fn test_interrupt_match_allows_class_name_with_title_candidates() {
    let source = r##"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "default_action": "cancel",
  "interrupts": [
    {
      "id": "class-and-title",
      "priority": 10,
      "match": {
        "class_name": "#32770",
        "title_candidates": [{"value": "Save changes", "locale_dependent": true}]
      },
      "action": {"kind": "cancel"},
      "classification": "transient_popup",
      "requires_human": false
    }
  ]
}"##;
    assert_eq!(findings(source), Vec::<String>::new());
}

#[test]
fn test_interrupt_match_allows_automation_id_candidates_with_class_name_contains() {
    let source = r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "default_action": "dismiss_explicitly",
  "interrupts": [
    {
      "id": "candidate-and-class-fragment",
      "priority": 10,
      "match": {
        "automation_id_candidates": ["PrivacyTeachingTip", "PsDownloadTeachingTip"],
        "class_name_contains": "TeachingTip"
      },
      "action": {"kind": "dismiss_explicitly"},
      "classification": "transient_popup",
      "requires_human": false
    }
  ]
}"#;
    assert_eq!(findings(source), Vec::<String>::new());
}

#[test]
fn test_interrupt_match_rejects_present_empty_identity_strings() {
    let source = r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "default_action": "cancel",
  "interrupts": [
    {
      "id": "empty-class",
      "priority": 30,
      "match": {"class_name": ""},
      "action": {"kind": "cancel"},
      "classification": "transient_popup",
      "requires_human": false
    },
    {
      "id": "empty-automation-id",
      "priority": 20,
      "match": {"automation_id": "   "},
      "action": {"kind": "cancel"},
      "classification": "transient_popup",
      "requires_human": false
    },
    {
      "id": "empty-class-fragment",
      "priority": 10,
      "match": {"class_name_contains": ""},
      "action": {"kind": "cancel"},
      "classification": "transient_popup",
      "requires_human": false
    }
  ]
}"#;
    let messages = findings(source);
    assert_eq!(messages.len(), 3, "{messages:?}");
    assert!(
        messages
            .iter()
            .all(|message| message == "must be non-empty string"),
        "{messages:?}"
    );
}
