//! Unit tests for pure egress-policy resolution.
//!
//! These tests use only in-memory inputs and assert fail-closed behavior.

use super::*;

fn app(value: &str) -> Result<AppId, EgressError> {
    AppId::new(value)
}

fn egress_destination(value: &str) -> Result<EgressDestinationId, EgressError> {
    EgressDestinationId::new(value)
}

fn policy_with_redacted() -> Result<EgressPolicy, EgressError> {
    EgressPolicy::new(
        EgressLevel::Redacted,
        Vec::new(),
        Vec::new(),
        vec![egress_destination("api.example.com")?],
    )
}

#[test]
fn test_default_level_is_redacted() {
    assert_eq!(EgressLevel::default(), EgressLevel::Redacted);
    assert_eq!(
        EgressPolicy::default().default_level(),
        EgressLevel::Redacted
    );
}

#[test]
fn test_strictest_combines_levels_monotonically() {
    assert_eq!(
        EgressLevel::Full.strictest(EgressLevel::LocalOnly),
        EgressLevel::LocalOnly
    );
    assert_eq!(
        EgressLevel::Redacted.strictest(EgressLevel::Full),
        EgressLevel::Redacted
    );
    assert_eq!(
        EgressLevel::Full.strictest(EgressLevel::Full),
        EgressLevel::Full
    );
}

#[test]
fn test_content_type_override_can_only_tighten_app_override()
-> Result<(), Box<dyn std::error::Error>> {
    let notepad = app("com.microsoft.notepad")?;
    let policy = EgressPolicy::new(
        EgressLevel::Redacted,
        vec![(notepad.clone(), EgressLevel::Full)],
        vec![(ContentType::Screenshot, EgressLevel::Redacted)],
        Vec::new(),
    )?;
    assert_eq!(
        policy.effective_level(Some(&notepad), ContentType::Screenshot),
        EgressLevel::Redacted
    );
    assert_eq!(
        policy.effective_level(Some(&notepad), ContentType::FileContent),
        EgressLevel::Full
    );
    Ok(())
}

#[test]
fn test_per_app_local_only_cannot_be_widened_by_full_content_override()
-> Result<(), Box<dyn std::error::Error>> {
    let stock = app("com.example.stock")?;
    let policy = EgressPolicy::new(
        EgressLevel::Redacted,
        vec![(stock.clone(), EgressLevel::LocalOnly)],
        vec![(ContentType::Screenshot, EgressLevel::Full)],
        Vec::new(),
    )?;
    assert_eq!(
        policy.effective_level(Some(&stock), ContentType::Screenshot),
        EgressLevel::LocalOnly
    );
    Ok(())
}

#[test]
fn test_local_only_without_local_model_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let policy = EgressPolicy::new(EgressLevel::LocalOnly, Vec::new(), Vec::new(), Vec::new())?;
    let egress_destination = egress_destination("api.example.com")?;
    let request = EgressRequest {
        app_id: None,
        content_type: ContentType::FileContent,
        egress_destination: &egress_destination,
        local_model_available: false,
    };
    assert_eq!(
        policy.evaluate(request),
        Err(EgressError::LocalModelUnavailable)
    );
    Ok(())
}

#[test]
fn test_local_only_with_local_model_never_returns_egress_destination()
-> Result<(), Box<dyn std::error::Error>> {
    let policy = EgressPolicy::new(EgressLevel::LocalOnly, Vec::new(), Vec::new(), Vec::new())?;
    let egress_destination = egress_destination("api.example.com")?;
    let decision = policy.evaluate(EgressRequest {
        app_id: None,
        content_type: ContentType::Clipboard,
        egress_destination: &egress_destination,
        local_model_available: true,
    })?;
    assert_eq!(decision.effective_level(), EgressLevel::LocalOnly);
    assert!(decision.egress_destination().is_none());
    assert!(!decision.redaction_required());
    Ok(())
}

#[test]
fn test_redacted_egress_requires_allowlisted_egress_destination_and_redaction()
-> Result<(), Box<dyn std::error::Error>> {
    let policy = policy_with_redacted()?;
    let allowed = egress_destination("api.example.com")?;
    let decision = policy.evaluate(EgressRequest {
        app_id: None,
        content_type: ContentType::FileContent,
        egress_destination: &allowed,
        local_model_available: false,
    })?;
    assert_eq!(decision.effective_level(), EgressLevel::Redacted);
    assert_eq!(decision.egress_destination(), Some(&allowed));
    assert!(decision.redaction_required());
    Ok(())
}

#[test]
fn test_non_local_egress_rejects_unknown_egress_destination()
-> Result<(), Box<dyn std::error::Error>> {
    let policy = policy_with_redacted()?;
    let unknown = egress_destination("api.unknown.example")?;
    let request = EgressRequest {
        app_id: None,
        content_type: ContentType::UiTree,
        egress_destination: &unknown,
        local_model_available: false,
    };
    assert_eq!(
        policy.evaluate(request),
        Err(EgressError::EgressDestinationNotAllowed)
    );
    Ok(())
}

#[test]
fn test_full_egress_requires_allowlisted_egress_destination_without_redaction()
-> Result<(), Box<dyn std::error::Error>> {
    let allowed = egress_destination("api.example.com")?;
    let policy = EgressPolicy::new(
        EgressLevel::Full,
        Vec::new(),
        Vec::new(),
        vec![allowed.clone()],
    )?;
    let decision = policy.evaluate(EgressRequest {
        app_id: None,
        content_type: ContentType::FileContent,
        egress_destination: &allowed,
        local_model_available: false,
    })?;
    assert_eq!(decision.effective_level(), EgressLevel::Full);
    assert!(!decision.redaction_required());
    Ok(())
}

#[test]
fn test_identifier_validation_rejects_noncanonical_values() {
    assert_eq!(AppId::new(""), Err(EgressError::InvalidAppId));
    assert_eq!(AppId::new(" Com.Example "), Err(EgressError::InvalidAppId));
    assert_eq!(AppId::new("Com.Example"), Err(EgressError::InvalidAppId));
    assert_eq!(
        EgressDestinationId::new("api example"),
        Err(EgressError::InvalidEgressDestination)
    );
    assert_eq!(
        EgressDestinationId::new("API.EXAMPLE"),
        Err(EgressError::InvalidEgressDestination)
    );
}

#[test]
fn test_policy_constructor_rejects_duplicate_keys() -> Result<(), Box<dyn std::error::Error>> {
    let duplicate_app = app("com.example.app")?;
    assert_eq!(
        EgressPolicy::new(
            EgressLevel::Redacted,
            vec![
                (duplicate_app.clone(), EgressLevel::Full),
                (duplicate_app, EgressLevel::LocalOnly),
            ],
            Vec::new(),
            Vec::new(),
        )
        .err(),
        Some(EgressError::DuplicateAppOverride)
    );
    assert_eq!(
        EgressPolicy::new(
            EgressLevel::Redacted,
            Vec::new(),
            vec![
                (ContentType::Clipboard, EgressLevel::Full),
                (ContentType::Clipboard, EgressLevel::LocalOnly),
            ],
            Vec::new(),
        )
        .err(),
        Some(EgressError::DuplicateContentTypeOverride)
    );
    let duplicate_egress_destination = egress_destination("api.example.com")?;
    assert_eq!(
        EgressPolicy::new(
            EgressLevel::Redacted,
            Vec::new(),
            Vec::new(),
            vec![
                duplicate_egress_destination.clone(),
                duplicate_egress_destination
            ],
        )
        .err(),
        Some(EgressError::DuplicateAllowedEgressDestination)
    );
    Ok(())
}

#[test]
fn test_application_override_bound_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut policy = EgressPolicy::default();
    for index in 0..MAX_APP_OVERRIDES {
        let app_id = app(&format!("app{index}"))?;
        policy.set_app_override(app_id, EgressLevel::Full)?;
    }
    let overflow = app("app-overflow")?;
    assert_eq!(
        policy.set_app_override(overflow, EgressLevel::Full),
        Err(EgressError::TooManyAppOverrides)
    );
    Ok(())
}

#[test]
fn test_egress_destination_allowlist_bound_and_idempotence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut policy = EgressPolicy::default();
    for index in 0..MAX_ALLOWED_EGRESS_DESTINATIONS {
        policy.allow_egress_destination(egress_destination(&format!("api{index}.example.com"))?)?;
    }
    assert!(
        policy
            .allow_egress_destination(egress_destination("api0.example.com")?)?
            .is_none()
    );
    assert_eq!(
        policy.allow_egress_destination(egress_destination("overflow.example.com")?),
        Err(EgressError::TooManyAllowedEgressDestinations)
    );
    Ok(())
}

#[test]
fn test_policy_change_records_widening_and_default_change() -> Result<(), Box<dyn std::error::Error>>
{
    let mut policy = EgressPolicy::default();
    let narrowing = policy.set_default_level(EgressLevel::LocalOnly);
    assert!(!narrowing.widens_egress());
    let widening = policy.set_default_level(EgressLevel::Full);
    assert!(widening.widens_egress());
    assert!(widening.requires_upgrade_confirmation());
    let app_id = app("com.example.app")?;
    let app_narrowing = policy.set_app_override(app_id.clone(), EgressLevel::LocalOnly)?;
    assert!(!app_narrowing.widens_egress());
    let app_widening = policy.set_app_override(app_id, EgressLevel::Full)?;
    assert!(app_widening.widens_egress());
    Ok(())
}
