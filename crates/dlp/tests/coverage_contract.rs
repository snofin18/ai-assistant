#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

//! Public-contract coverage for the DLP policy and screenshot-redaction APIs.

use std::error::Error;

use assistant_dlp::{
    AppId, ContentType, EgressDecision, EgressDestinationId, EgressError, EgressLevel,
    EgressLevelScope, EgressPolicy, EgressPolicyChange, EgressRequest, ImageDimensions,
    MAX_ALLOWED_EGRESS_DESTINATIONS, MAX_APP_OVERRIDES, MAX_REDACTION_RULES, RedactError,
    RedactionRegion, RedactionRule, resolve_occlusions,
};

fn app(value: &str) -> AppId {
    AppId::new(value).unwrap()
}

fn destination(value: &str) -> EgressDestinationId {
    EgressDestinationId::new(value).unwrap()
}

fn region(x: i64, y: i64, width: u32, height: u32) -> RedactionRegion {
    RedactionRegion::new(x, y, width, height).unwrap()
}

fn evaluate(
    policy: &EgressPolicy,
    app_id: Option<&AppId>,
    content_type: ContentType,
    destination: &EgressDestinationId,
    local_model_available: bool,
) -> Result<EgressDecision, EgressError> {
    policy.evaluate(EgressRequest {
        app_id,
        content_type,
        egress_destination: destination,
        local_model_available,
    })
}

#[test]
fn test_level_ordering_is_monotonic_and_widening_is_explicit() {
    assert_eq!(
        EgressLevel::LocalOnly.strictest(EgressLevel::Full),
        EgressLevel::LocalOnly
    );
    assert_eq!(
        EgressLevel::Redacted.strictest(EgressLevel::LocalOnly),
        EgressLevel::LocalOnly
    );
    assert_eq!(
        EgressLevel::Full.strictest(EgressLevel::Redacted),
        EgressLevel::Redacted
    );
    assert_eq!(
        EgressLevel::Full.strictest(EgressLevel::Full),
        EgressLevel::Full
    );
    assert!(EgressLevel::Redacted.is_wider_than(EgressLevel::LocalOnly));
    assert!(EgressLevel::Full.is_wider_than(EgressLevel::LocalOnly));
    assert!(EgressLevel::Full.is_wider_than(EgressLevel::Redacted));
    assert!(!EgressLevel::LocalOnly.is_wider_than(EgressLevel::Full));
}

#[test]
fn test_identifier_validation_and_text_accessors() {
    let application = app("com.example.app_1");
    assert_eq!(application.as_text(), "com.example.app_1");
    let egress_destination = destination("api.example.com:8443/path");
    assert_eq!(egress_destination.as_text(), "api.example.com:8443/path");

    for invalid in ["", " leading", "trailing ", "UPPER", "-leading", ".leading"] {
        assert_eq!(AppId::new(invalid), Err(EgressError::InvalidAppId));
    }
    assert_eq!(AppId::new("bad/slash"), Err(EgressError::InvalidAppId));
    assert_eq!(AppId::new("bad:colon"), Err(EgressError::InvalidAppId));
    assert_eq!(AppId::new("bad@char"), Err(EgressError::InvalidAppId));
    assert_eq!(AppId::new("a".repeat(256)), Err(EgressError::InvalidAppId));
    assert_eq!(
        EgressDestinationId::new("bad?query"),
        Err(EgressError::InvalidEgressDestination)
    );
    assert_eq!(
        EgressDestinationId::new("bad space"),
        Err(EgressError::InvalidEgressDestination)
    );
}

#[test]
fn test_policy_constructor_bounds_and_duplicate_rejection() {
    assert!(EgressPolicy::new(EgressLevel::LocalOnly, Vec::new(), Vec::new(), Vec::new()).is_ok());

    let too_many_apps = (0..=MAX_APP_OVERRIDES)
        .map(|index| (app(&format!("app{index}")), EgressLevel::Full))
        .collect::<Vec<_>>();
    assert_eq!(
        EgressPolicy::new(EgressLevel::Redacted, too_many_apps, Vec::new(), Vec::new()),
        Err(EgressError::TooManyAppOverrides)
    );

    let too_many_content_types = vec![
        (ContentType::Clipboard, EgressLevel::Full),
        (ContentType::Clipboard, EgressLevel::Full),
        (ContentType::Screenshot, EgressLevel::Full),
        (ContentType::FileContent, EgressLevel::Full),
        (ContentType::UiTree, EgressLevel::Full),
    ];
    assert_eq!(
        EgressPolicy::new(
            EgressLevel::Redacted,
            Vec::new(),
            too_many_content_types,
            Vec::new()
        ),
        Err(EgressError::TooManyContentTypeOverrides)
    );

    let too_many_destinations = (0..=MAX_ALLOWED_EGRESS_DESTINATIONS)
        .map(|index| destination(&format!("api{index}.example.com")))
        .collect::<Vec<_>>();
    assert_eq!(
        EgressPolicy::new(
            EgressLevel::Redacted,
            Vec::new(),
            Vec::new(),
            too_many_destinations
        ),
        Err(EgressError::TooManyAllowedEgressDestinations)
    );
}

#[test]
fn test_policy_mutation_records_and_scope_resolution() {
    let mut policy = EgressPolicy::default();
    let default_change = policy.set_default_level(EgressLevel::LocalOnly);
    assert!(matches!(
        default_change,
        EgressPolicyChange::Level {
            scope: EgressLevelScope::Default,
            previous: EgressLevel::Redacted,
            current: EgressLevel::LocalOnly
        }
    ));
    assert!(!default_change.widens_egress());
    assert!(!default_change.requires_upgrade_confirmation());

    let application = app("com.example.app");
    let app_change = policy
        .set_app_override(application.clone(), EgressLevel::Full)
        .unwrap();
    assert!(app_change.widens_egress());
    assert_eq!(
        policy.effective_level(Some(&application), ContentType::Clipboard),
        EgressLevel::Full
    );
    assert_eq!(
        policy.effective_level(Some(&app("com.example.other")), ContentType::Clipboard),
        EgressLevel::LocalOnly
    );

    let update = policy
        .set_app_override(application.clone(), EgressLevel::Redacted)
        .unwrap();
    assert!(!update.widens_egress());
    assert_eq!(
        policy.effective_level(Some(&application), ContentType::Clipboard),
        EgressLevel::Redacted
    );

    let inserted = policy.set_content_type_override(ContentType::Clipboard, EgressLevel::Full);
    assert!(matches!(
        inserted,
        EgressPolicyChange::Level {
            scope: EgressLevelScope::ContentType(ContentType::Clipboard),
            previous: EgressLevel::LocalOnly,
            current: EgressLevel::Full
        }
    ));
    let updated = policy.set_content_type_override(ContentType::Clipboard, EgressLevel::Redacted);
    assert!(matches!(
        updated,
        EgressPolicyChange::Level {
            scope: EgressLevelScope::ContentType(ContentType::Clipboard),
            previous: EgressLevel::Full,
            current: EgressLevel::Redacted
        }
    ));
    assert_eq!(
        policy.effective_level(None, ContentType::Clipboard),
        EgressLevel::LocalOnly
    );
}

#[test]
fn test_destination_allowlist_records_and_evaluation_matrix() {
    let allowed = destination("api.example.com");
    let unknown = destination("api.unknown.example");
    let mut policy = EgressPolicy::new(
        EgressLevel::Redacted,
        Vec::new(),
        Vec::new(),
        vec![allowed.clone()],
    )
    .unwrap();

    let change = policy
        .allow_egress_destination(destination("backup.example.com"))
        .unwrap()
        .unwrap();
    assert!(change.widens_egress());
    assert!(matches!(
        change,
        EgressPolicyChange::EgressDestinationAllowed { .. }
    ));
    assert!(
        policy
            .allow_egress_destination(allowed.clone())
            .unwrap()
            .is_none()
    );

    let redacted = evaluate(&policy, None, ContentType::FileContent, &allowed, false).unwrap();
    assert_eq!(redacted.effective_level(), EgressLevel::Redacted);
    assert_eq!(redacted.egress_destination(), Some(&allowed));
    assert!(redacted.redaction_required());
    assert_eq!(
        evaluate(&policy, None, ContentType::UiTree, &unknown, false),
        Err(EgressError::EgressDestinationNotAllowed)
    );

    let local =
        EgressPolicy::new(EgressLevel::LocalOnly, Vec::new(), Vec::new(), Vec::new()).unwrap();
    assert_eq!(
        evaluate(&local, None, ContentType::Clipboard, &allowed, false),
        Err(EgressError::LocalModelUnavailable)
    );
    let decision = evaluate(&local, None, ContentType::Clipboard, &allowed, true).unwrap();
    assert!(decision.egress_destination().is_none());
    assert!(!decision.redaction_required());

    let full = EgressPolicy::new(
        EgressLevel::Full,
        Vec::new(),
        Vec::new(),
        vec![allowed.clone()],
    )
    .unwrap();
    let decision = evaluate(&full, None, ContentType::Screenshot, &allowed, false).unwrap();
    assert!(!decision.redaction_required());
}

#[test]
fn test_egress_error_display_and_std_error_contract() {
    let messages = [
        (EgressError::InvalidAppId, "application id"),
        (
            EgressError::InvalidEgressDestination,
            "egress_destination id",
        ),
        (EgressError::TooManyAppOverrides, "hard limit"),
        (
            EgressError::TooManyContentTypeOverrides,
            "closed variant count",
        ),
        (EgressError::TooManyAllowedEgressDestinations, "hard limit"),
        (EgressError::DuplicateAppOverride, "duplicate"),
        (EgressError::DuplicateContentTypeOverride, "duplicate"),
        (EgressError::DuplicateAllowedEgressDestination, "duplicate"),
        (EgressError::LocalModelUnavailable, "local model"),
        (EgressError::EgressDestinationNotAllowed, "allow-list"),
    ];
    for (error, expected) in messages {
        assert!(error.to_string().contains(expected));
        let dynamic: &dyn Error = &error;
        assert!(dynamic.source().is_none());
    }
}

#[test]
fn test_application_override_runtime_bound_is_enforced() {
    let mut policy = EgressPolicy::default();
    for index in 0..MAX_APP_OVERRIDES {
        policy
            .set_app_override(app(&format!("app{index}")), EgressLevel::Full)
            .unwrap();
    }
    assert_eq!(
        policy.set_app_override(app("app-overflow"), EgressLevel::Full),
        Err(EgressError::TooManyAppOverrides)
    );
}

#[test]
fn test_redaction_region_accessors_and_fail_closed_construction() {
    let bounds = ImageDimensions::new(100, 80).unwrap();
    assert_eq!(bounds.width(), 100);
    assert_eq!(bounds.height(), 80);
    assert_eq!(ImageDimensions::new(0, 1), Err(RedactError::ZeroDimension));
    assert_eq!(ImageDimensions::new(1, 0), Err(RedactError::ZeroDimension));

    let region = region(4, 5, 20, 10);
    assert_eq!(region.x(), 4);
    assert_eq!(region.y(), 5);
    assert_eq!(region.width(), 20);
    assert_eq!(region.height(), 10);
    assert_eq!(region.right(), 24);
    assert_eq!(region.bottom(), 15);
    assert_eq!(
        RedactionRegion::new(-1, 0, 1, 1),
        Err(RedactError::NegativeCoordinate)
    );
    assert_eq!(
        RedactionRegion::new(0, -1, 1, 1),
        Err(RedactError::NegativeCoordinate)
    );
    assert_eq!(
        RedactionRegion::new(0, 0, 0, 1),
        Err(RedactError::ZeroDimension)
    );
    assert_eq!(
        RedactionRegion::new(0, 0, 1, 0),
        Err(RedactError::ZeroDimension)
    );
    assert_eq!(
        RedactionRegion::new(i64::MAX, 0, 1, 1),
        Err(RedactError::CoordinateOverflow)
    );
}

#[test]
fn test_occlusion_resolution_boundaries_and_rule_builders() {
    let bounds = ImageDimensions::new(100, 80).unwrap();
    let password = RedactionRule::password_field(region(0, 0, 1, 1));
    let matched = RedactionRule::matched_region(region(99, 79, 1, 1));
    assert_eq!(password.region(), region(0, 0, 1, 1));
    assert_eq!(matched.region(), region(99, 79, 1, 1));

    let occlusions = resolve_occlusions(bounds, &[password, matched]).unwrap();
    let first = occlusions.first().copied().unwrap();
    assert_eq!(first.x(), 0);
    assert_eq!(first.y(), 0);
    assert_eq!(first.width(), 1);
    assert_eq!(first.height(), 1);
    let second = occlusions.get(1).copied().unwrap();
    assert_eq!(second.x(), 99);
    assert_eq!(second.y(), 79);
    assert_eq!(second.width(), 1);
    assert_eq!(second.height(), 1);

    let maximum = vec![RedactionRule::matched_region(region(0, 0, 1, 1)); MAX_REDACTION_RULES];
    assert_eq!(
        resolve_occlusions(bounds, &maximum).unwrap().len(),
        MAX_REDACTION_RULES
    );
    let overflow = vec![RedactionRule::matched_region(region(0, 0, 1, 1)); MAX_REDACTION_RULES + 1];
    assert_eq!(
        resolve_occlusions(bounds, &overflow),
        Err(RedactError::TooManyRules)
    );
    assert_eq!(resolve_occlusions(bounds, &[]), Err(RedactError::NoRules));
    assert_eq!(
        resolve_occlusions(
            ImageDimensions::new(10, 10).unwrap(),
            &[RedactionRule::matched_region(region(0, 10, 1, 1))]
        ),
        Err(RedactError::RegionOutOfBounds)
    );
    assert_eq!(
        resolve_occlusions(
            ImageDimensions::new(10, 10).unwrap(),
            &[RedactionRule::matched_region(region(10, 0, 1, 1))]
        ),
        Err(RedactError::RegionOutOfBounds)
    );
}

#[test]
fn test_redact_error_display_and_std_error_contract() {
    let messages = [
        (RedactError::NoRules, "without any rules"),
        (RedactError::NegativeCoordinate, "non-negative"),
        (RedactError::ZeroDimension, "non-zero"),
        (RedactError::CoordinateOverflow, "overflowed"),
        (RedactError::RegionOutOfBounds, "outside"),
        (RedactError::TooManyRules, "hard limit"),
    ];
    for (error, expected) in messages {
        assert!(error.to_string().contains(expected));
        let dynamic: &dyn Error = &error;
        assert!(dynamic.source().is_none());
    }
}
