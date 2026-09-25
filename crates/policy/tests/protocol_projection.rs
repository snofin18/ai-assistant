//! Policy-to-protocol decision projection tests.

use assistant_policy::{Decision, ScopeOption};

#[test]
fn test_allow_and_deny_project_to_protocol_shape() {
    let allow = Decision::Allow {
        rule_id: "allow_read_low_risk".to_owned(),
    }
    .to_protocol_decision();
    assert!(matches!(
        allow,
        Ok(decision) if decision.allow
            && decision.rule_id.as_deref() == Some("allow_read_low_risk")
    ));

    let deny = Decision::Deny {
        rule_id: "block_irreversible_unattended".to_owned(),
        reason: "irreversible actions are forbidden while unattended".to_owned(),
    }
    .to_protocol_decision();
    assert!(matches!(
        deny,
        Ok(decision) if !decision.allow
            && decision.rule_id.as_deref() == Some("block_irreversible_unattended")
            && decision.reason.as_deref().is_some_and(|reason| !reason.is_empty())
    ));
}

#[test]
fn test_confirmation_projection_keeps_rich_fields_in_local_decision() {
    let decision = Decision::AllowWithConfirmation {
        rule_id: "confirm_medium_write".to_owned(),
        scope_options: vec![ScopeOption::Once, ScopeOption::ThisTask],
        show_diff: true,
    };
    assert!(decision.requires_confirmation());
    assert!(!decision.is_allowed());
    let projected = decision.to_protocol_decision();
    assert!(matches!(
        projected,
        Ok(value) if value.allow
            && value.rule_id.as_deref() == Some("confirm_medium_write")
            && value.reason.as_deref() == Some("human confirmation required before execution")
    ));
}
