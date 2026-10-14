//! Public contract coverage for state-fingerprint canonical names and ignore metadata.
//!
//! These tests exercise only exported `assistant-verify` APIs. They pin the stable
//! names that adapters and diagnostics inspect without reaching into private helpers.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use assistant_protocol::serde_json;
use assistant_verify::{FingerprintField, FingerprintIgnore, FingerprintSubject, canonical_form};

#[test]
fn test_fingerprint_field_canonical_names_and_all_order_are_stable() {
    let cases = [
        (FingerprintField::Title, "title", "title"),
        (FingerprintField::ControlIds, "control_ids", "control_ids"),
        (
            FingerprintField::ControlStates,
            "control_states",
            "control_states",
        ),
        (FingerprintField::Document, "document", "document"),
        (
            FingerprintField::ScrollPosition,
            "scroll",
            "scroll_position",
        ),
        (
            FingerprintField::ModalDialog,
            "modal_dialog",
            "modal_dialog",
        ),
    ];

    for (field, expected_name, expected_serde_name) in cases {
        assert_eq!(field.canonical_name(), expected_name);
        assert_eq!(
            serde_json::to_string(&field).expect("field serializes"),
            format!("\"{expected_serde_name}\"")
        );
    }

    assert_eq!(
        FingerprintField::ALL.map(FingerprintField::canonical_name),
        [
            "title",
            "control_ids",
            "control_states",
            "document",
            "scroll",
            "modal_dialog",
        ]
    );
}

#[test]
fn test_fingerprint_ignore_all_fields_preserves_canonical_order() {
    let ignore = FingerprintIgnore::excluding(FingerprintField::ALL);
    assert_eq!(ignore.fields(), FingerprintField::ALL);

    let rendered = canonical_form(&FingerprintSubject::new("scope", "title"), &ignore);
    assert_eq!(rendered.matches("<ignored>").count(), 6);
    for name in FingerprintField::ALL.map(FingerprintField::canonical_name) {
        assert!(
            rendered.contains(&format!("{name}=9:<ignored>\n")),
            "{name} must be rendered as an ignored field"
        );
    }
}

#[test]
fn test_modal_dialog_true_is_rendered_when_not_ignored() {
    let subject = FingerprintSubject {
        has_modal_dialog: true,
        ..FingerprintSubject::new("scope", "title")
    };
    let rendered = canonical_form(&subject, &FingerprintIgnore::none());
    assert!(rendered.contains("modal_dialog=4:true\n"));
}
