//! Contract tests for state fingerprints, idempotency classification, and `on_violation` dispatch.
//!
//! Three concerns that all feed the same decision — "did the previous step happen, and if it did not,
//! what do we do about it?" — so they share one file. The properties under test are section 7.3's
//! (reproducible, per-adapter ignores, no separator collisions), section 8.5's (never guess
//! `Applied`), and section 7.4's (`retry_once` only for transient failures).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_platform_api::Fingerprint;
use assistant_protocol::ErrorCode;
use assistant_protocol::serde_json;
use assistant_verify::{
    ApplicationEvidence, ApplicationVerdict, ControlState, DocumentDigest, FingerprintField,
    FingerprintIgnore, FingerprintSubject, OnViolation, ScrollPosition, ViolationAction,
    ViolationKind, canonical_form, classify_application, dispatch_on_violation, parse_on_violation,
    state_fingerprint,
};

/// Builds a fingerprint from a repeated hex digit.
fn fingerprint(hex_digit: char) -> Fingerprint {
    let value = format!("sha256:{}", hex_digit.to_string().repeat(64));
    Fingerprint::parse(value).expect("a repeated hex digit is a valid digest")
}

fn subject() -> FingerprintSubject {
    FingerprintSubject {
        control_ids: vec!["edit1".to_owned(), "button2".to_owned()],
        control_states: vec![ControlState {
            id: "edit1".to_owned(),
            role: "Edit".to_owned(),
            enabled: true,
            focused: true,
            selected: false,
        }],
        document: Some(DocumentDigest {
            length: 42,
            head: "hello".to_owned(),
            tail: "world".to_owned(),
            sampled_hash: "deadbeef".to_owned(),
        }),
        scroll: Some(ScrollPosition {
            horizontal: 0,
            vertical: 120,
        }),
        ..FingerprintSubject::new("document.body", "untitled - Notepad")
    }
}

// ------------------------------------------------------------ fingerprint

#[test]
fn test_canonical_form_is_deterministic() {
    let ignore = FingerprintIgnore::none();
    assert_eq!(
        canonical_form(&subject(), &ignore),
        canonical_form(&subject(), &ignore)
    );
}

#[test]
fn test_state_fingerprint_is_stable_and_well_formed() {
    let ignore = FingerprintIgnore::none();
    let first = state_fingerprint(&subject(), &ignore).expect("hash");
    let second = state_fingerprint(&subject(), &ignore).expect("hash");
    assert_eq!(first, second);
    assert!(first.as_str().starts_with("sha256:"));
    assert_eq!(first.digest().len(), 64);
}

#[test]
fn test_ignored_field_does_not_change_the_digest() {
    // Section 7.3: per-adapter ignores are how jitter is removed. Ignoring the title must make a
    // title-only change invisible.
    let ignore = FingerprintIgnore::excluding([FingerprintField::Title]);
    let base = state_fingerprint(&subject(), &ignore).expect("hash");
    let mut changed = subject();
    changed.title = "untitled * - Notepad".to_owned();
    assert_eq!(base, state_fingerprint(&changed, &ignore).expect("hash"));
}

#[test]
fn test_non_ignored_field_changes_the_digest() {
    let ignore = FingerprintIgnore::none();
    let base = state_fingerprint(&subject(), &ignore).expect("hash");
    let mut changed = subject();
    changed.title = "untitled * - Notepad".to_owned();
    assert_ne!(base, state_fingerprint(&changed, &ignore).expect("hash"));
}

#[test]
fn test_different_ignore_sets_produce_different_digests() {
    let strict = state_fingerprint(&subject(), &FingerprintIgnore::none()).expect("hash");
    let lenient = state_fingerprint(
        &subject(),
        &FingerprintIgnore::excluding([FingerprintField::Title]),
    )
    .expect("hash");
    assert_ne!(strict, lenient);
}

#[test]
fn test_separator_injection_cannot_forge_a_collision() {
    // Two different subjects must not collide just because a value contains the separators the
    // encoding uses; length prefixes are what prevent it.
    let first = FingerprintSubject::new("scope", "a|title=1:x\n");
    let second = FingerprintSubject {
        title: "a".to_owned(),
        ..FingerprintSubject::new("scope", "a")
    };
    let ignore = FingerprintIgnore::none();
    assert_ne!(
        canonical_form(&first, &ignore),
        canonical_form(&second, &ignore)
    );
}

#[test]
fn test_ignore_duplicates_collapse() {
    let ignore = FingerprintIgnore::excluding([
        FingerprintField::Title,
        FingerprintField::Title,
        FingerprintField::Document,
    ]);
    assert_eq!(
        ignore.fields(),
        vec![FingerprintField::Title, FingerprintField::Document]
    );
    assert!(ignore.ignores(FingerprintField::Title));
    assert!(!ignore.ignores(FingerprintField::ScrollPosition));
}

#[test]
fn test_absent_optionals_are_distinguishable_from_ignored() {
    let ignore = FingerprintIgnore::none();
    let absent = FingerprintSubject::new("scope", "title");
    let rendered = canonical_form(&absent, &ignore);
    assert!(rendered.contains("<absent>"));
    assert!(!rendered.contains("<ignored>"));
}

#[test]
fn test_control_id_order_matters() {
    let ignore = FingerprintIgnore::none();
    let base = state_fingerprint(&subject(), &ignore).expect("hash");
    let mut reordered = subject();
    reordered.control_ids.reverse();
    assert_ne!(
        base,
        state_fingerprint(&reordered, &ignore).expect("hash"),
        "the ordered control id list is part of the fingerprint"
    );
}

// ----------------------------------------------------------- idempotency

#[test]
fn test_missing_fingerprints_are_unknown() {
    assert_eq!(
        classify_application(&ApplicationEvidence::default()),
        ApplicationVerdict::Unknown
    );
    assert_eq!(
        classify_application(&ApplicationEvidence {
            before: Some(fingerprint('a')),
            after: None,
            marker_observed: None,
        }),
        ApplicationVerdict::Unknown
    );
    assert_eq!(
        classify_application(&ApplicationEvidence {
            before: None,
            after: Some(fingerprint('a')),
            marker_observed: None,
        }),
        ApplicationVerdict::Unknown
    );
}

#[test]
fn test_identical_fingerprints_mean_not_applied() {
    let evidence = ApplicationEvidence::from_fingerprints(fingerprint('a'), fingerprint('a'));
    assert_eq!(
        classify_application(&evidence),
        ApplicationVerdict::NotApplied
    );
}

#[test]
fn test_changed_fingerprints_mean_applied() {
    let evidence = ApplicationEvidence::from_fingerprints(fingerprint('a'), fingerprint('b'));
    assert_eq!(classify_application(&evidence), ApplicationVerdict::Applied);
}

#[test]
fn test_application_report_beats_the_fingerprint() {
    // The application says the effect is absent; a fingerprint difference (from a component the
    // adapter did not ignore) must not override it.
    let evidence = ApplicationEvidence {
        marker_observed: Some(false),
        ..ApplicationEvidence::from_fingerprints(fingerprint('a'), fingerprint('b'))
    };
    assert_eq!(
        classify_application(&evidence),
        ApplicationVerdict::NotApplied
    );
}

#[test]
fn test_application_report_alone_is_enough() {
    assert_eq!(
        classify_application(&ApplicationEvidence::from_marker(true)),
        ApplicationVerdict::Applied
    );
    assert_eq!(
        classify_application(&ApplicationEvidence::from_marker(false)),
        ApplicationVerdict::NotApplied
    );
}

// ---------------------------------------------------------- on_violation

#[test]
fn test_all_schema_values_parse() {
    for strategy in OnViolation::ALL {
        assert_eq!(
            parse_on_violation(strategy.as_str()).expect("schema value"),
            strategy
        );
    }
}

#[test]
fn test_unknown_strategy_is_rejected() {
    let error = parse_on_violation("retry_forever").err();
    let error = error.expect("an unknown strategy must be rejected");
    assert!(error.to_string().contains("retry_forever"), "{error}");
}

#[test]
fn test_retry_once_only_for_transient_or_missing_target() {
    for (code, expected) in [
        (ErrorCode::Transient, ViolationAction::RetryOnce),
        (ErrorCode::TargetNotFound, ViolationAction::RetryOnce),
        (ErrorCode::VerifyFailed, ViolationAction::EscalateToUser),
        (ErrorCode::PolicyDenied, ViolationAction::EscalateToUser),
        (ErrorCode::TargetAmbiguous, ViolationAction::EscalateToUser),
    ] {
        assert_eq!(
            dispatch_on_violation(OnViolation::RetryOnce, ViolationKind::from_error_code(code)),
            expected,
            "for {code:?}"
        );
    }
}

#[test]
fn test_other_strategies_pass_through_unchanged() {
    for (strategy, expected) in [
        (
            OnViolation::RetryWithAlternative,
            ViolationAction::RetryWithAlternative,
        ),
        (OnViolation::EscalateToUser, ViolationAction::EscalateToUser),
        (OnViolation::Rollback, ViolationAction::Rollback),
        (OnViolation::AbortTask, ViolationAction::AbortTask),
    ] {
        assert_eq!(
            dispatch_on_violation(strategy, ViolationKind::Persistent),
            expected,
            "for {strategy:?}"
        );
    }
}

#[test]
fn test_violation_kind_from_error_code_covers_the_retryable_pair() {
    assert!(ViolationKind::from_error_code(ErrorCode::Transient).allows_single_retry());
    assert!(ViolationKind::from_error_code(ErrorCode::TargetNotFound).allows_single_retry());
    assert!(!ViolationKind::from_error_code(ErrorCode::Fatal).allows_single_retry());
}

#[test]
fn test_serialized_names_match_the_schema() {
    assert_eq!(
        serde_json::to_string(&OnViolation::RetryWithAlternative).expect("serialize"),
        "\"retry_with_alternative\""
    );
    assert_eq!(
        serde_json::to_string(&ApplicationVerdict::NotApplied).expect("serialize"),
        "\"not_applied\""
    );
}
