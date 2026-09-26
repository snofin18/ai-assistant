//! Approval request and resolution contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_hitl::{
    ApprovalDecision, ApprovalOutcome, ApprovalRequest, ApprovalRequestId, ApprovalRequestInput,
    ApprovalRisk, ApprovalScope, AuthorizationDimensions, AuthorizationOrigin,
    AuthorizationRequest, HitlError, SubjectId, TargetAuthorization, ToolAuthorization,
};
use assistant_policy::Effect;
use assistant_protocol::{PolicyDecision, PolicyScopeOption, RiskLevel, serde_json};
use assistant_task_engine::{Reversibility, StepId, TaskId};

use assistant_hitl::AppSessionId;

fn input() -> ApprovalRequestInput {
    ApprovalRequestInput {
        request_id: ApprovalRequestId::new("approval_1").expect("request"),
        origin: AuthorizationOrigin {
            task_id: TaskId::new("t_1").expect("task"),
            step_id: StepId::new("s_1").expect("step"),
            app_session_id: AppSessionId::new("sess_1").expect("session"),
        },
        dimensions: AuthorizationDimensions {
            subject: SubjectId::new("user").expect("subject"),
            tool: ToolAuthorization::new("notepad.text.write").expect("tool"),
            target: TargetAuthorization::new("notepad", "doc_1").expect("target"),
            effect: Effect::Write,
        },
        risk: ApprovalRisk {
            level: RiskLevel::Medium,
            reversibility: Reversibility::L1Snapshot,
        },
        impact_summary: "replace one word".to_owned(),
        diff: Some(assistant_hitl::DiffPreview::Fields(vec![
            assistant_hitl::FieldDiff::new("text", "old", "new").expect("field diff"),
        ])),
        requested_at_ms: 1_000,
        max_ttl_ms: 5_000,
        max_uses: 2,
    }
}

fn confirmation(scopes: Vec<PolicyScopeOption>, show_diff: bool) -> PolicyDecision {
    let scopes: Vec<&str> = scopes.into_iter().map(protocol_scope_name).collect();
    serde_json::from_value(serde_json::json!({
        "allow": true,
        "rule_id": "confirm_medium_write",
        "reason": "human confirmation required",
        "scope_options": scopes,
        "show_diff": show_diff,
    }))
    .expect("confirmation")
}

fn protocol_scope_name(scope: PolicyScopeOption) -> &'static str {
    match scope {
        PolicyScopeOption::Once => "once",
        PolicyScopeOption::ThisStepPattern => "this_step_pattern",
        PolicyScopeOption::ThisTask => "this_task",
        PolicyScopeOption::ThisAppSession => "this_app_session",
        PolicyScopeOption::Persistent => "persistent",
        _ => panic!("unexpected protocol scope"),
    }
}

#[test]
fn test_only_confirmation_decisions_can_create_requests() {
    let denied: PolicyDecision = serde_json::from_value(serde_json::json!({
        "allow": false,
        "rule_id": "deny",
        "reason": "blocked"
    }))
    .expect("denied");
    assert!(matches!(
        ApprovalRequest::from_policy_decision(input(), &denied),
        Err(HitlError::PolicyDenied)
    ));

    let unconditional: PolicyDecision = serde_json::from_value(serde_json::json!({
        "allow": true,
        "rule_id": "allow"
    }))
    .expect("unconditional");
    assert!(matches!(
        ApprovalRequest::from_policy_decision(input(), &unconditional),
        Err(HitlError::ApprovalNotRequired)
    ));
}

#[test]
fn test_confirmation_requires_diff_and_rejects_high_risk_broad_scope() {
    let missing_diff = confirmation(vec![PolicyScopeOption::Once], true);
    let mut no_diff = input();
    no_diff.diff = None;
    assert!(matches!(
        ApprovalRequest::from_policy_decision(no_diff, &missing_diff),
        Err(HitlError::InvalidValue { .. })
    ));

    let mut high_risk = input();
    high_risk.risk = ApprovalRisk {
        level: RiskLevel::High,
        reversibility: Reversibility::L1Snapshot,
    };
    assert!(matches!(
        ApprovalRequest::from_policy_decision(
            high_risk,
            &confirmation(vec![PolicyScopeOption::ThisTask], false)
        ),
        Err(HitlError::ForbiddenAuthorizationScope { .. })
    ));
}

#[test]
fn test_resolution_is_denied_or_timed_out_at_the_exact_boundary() {
    let request = ApprovalRequest::from_policy_decision(
        input(),
        &confirmation(vec![PolicyScopeOption::Once], false),
    )
    .expect("request");
    let denied = request
        .resolve(
            ApprovalDecision::Deny {
                reason: "not now".to_owned(),
            },
            1_100,
        )
        .expect("denied");
    assert!(matches!(denied, ApprovalOutcome::Denied { .. }));

    let timed_out = request
        .resolve(
            ApprovalDecision::Approve {
                subject: SubjectId::new("user").expect("subject"),
                authorization: AuthorizationRequest {
                    scope: ApprovalScope::Once,
                    ttl_ms: 1_000,
                    max_uses: 1,
                },
            },
            request.expires_at_ms(),
        )
        .expect("timeout");
    assert_eq!(timed_out, ApprovalOutcome::TimedOut);
}

#[test]
fn test_approval_cannot_select_unoffered_scope_or_expand_bounds() {
    let request = ApprovalRequest::from_policy_decision(
        input(),
        &confirmation(vec![PolicyScopeOption::Once], false),
    )
    .expect("request");
    let unoffered = request.resolve(
        ApprovalDecision::Approve {
            subject: SubjectId::new("user").expect("subject"),
            authorization: AuthorizationRequest {
                scope: ApprovalScope::ThisTask,
                ttl_ms: 1_000,
                max_uses: 1,
            },
        },
        1_100,
    );
    assert!(matches!(unoffered, Err(HitlError::ScopeNotOffered)));

    let too_long = request.resolve(
        ApprovalDecision::Approve {
            subject: SubjectId::new("user").expect("subject"),
            authorization: AuthorizationRequest {
                scope: ApprovalScope::Once,
                ttl_ms: 6_000,
                max_uses: 1,
            },
        },
        1_100,
    );
    assert!(matches!(too_long, Err(HitlError::InvalidValue { .. })));
}

#[test]
fn test_approved_request_creates_bounded_grant() {
    let request = ApprovalRequest::from_policy_decision(
        input(),
        &confirmation(vec![PolicyScopeOption::Once], false),
    )
    .expect("request");
    let outcome = request
        .resolve(
            ApprovalDecision::Approve {
                subject: SubjectId::new("user").expect("subject"),
                authorization: AuthorizationRequest {
                    scope: ApprovalScope::Once,
                    ttl_ms: 1_000,
                    max_uses: 1,
                },
            },
            1_100,
        )
        .expect("approve");
    let ApprovalOutcome::Approved { grant } = outcome else {
        panic!("expected approval");
    };
    assert_eq!(grant.scope(), ApprovalScope::Once);
    assert_eq!(grant.expires_at_ms(), 2_100);
    assert_eq!(grant.remaining_uses(), 1);
}
