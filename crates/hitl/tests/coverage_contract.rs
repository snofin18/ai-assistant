//! Public contract coverage for approval, authorization, errors, and diffs.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_hitl::{
    AppSessionId, ApprovalDecision, ApprovalOutcome, ApprovalRequest, ApprovalRequestId,
    ApprovalRequestInput, ApprovalRisk, ApprovalScope, AuthorizationDimensions, AuthorizationGrant,
    AuthorizationOrigin, AuthorizationRequest, DiffPreview, FieldDiff, FileChangeKind, FileDiff,
    HitlError, IrreversibleDiff, SubjectId, TargetAuthorization, TextDiffPreview,
    ToolAuthorization, UiStepDiff,
};
use assistant_policy::Effect;
use assistant_protocol::{ErrorCode, PolicyDecision, RiskLevel, serde_json};
use assistant_task_engine::{Reversibility, StepId, TaskEngineError, TaskId};

macro_rules! assert_error_code {
    ($error:expr, $code:expr) => {
        assert_eq!($error.error_code(), $code);
    };
}

fn origin(task: &str, step: &str, session: &str) -> AuthorizationOrigin {
    AuthorizationOrigin {
        task_id: TaskId::new(task).expect("task"),
        step_id: StepId::new(step).expect("step"),
        app_session_id: AppSessionId::new(session).expect("session"),
    }
}

fn dimensions(
    subject: &str,
    tool: &str,
    app: &str,
    target: &str,
    effect: Effect,
) -> AuthorizationDimensions {
    AuthorizationDimensions {
        subject: SubjectId::new(subject).expect("subject"),
        tool: ToolAuthorization::new(tool).expect("tool"),
        target: TargetAuthorization::new(app, target).expect("target"),
        effect,
    }
}

const fn low_risk() -> ApprovalRisk {
    ApprovalRisk {
        level: RiskLevel::Medium,
        reversibility: Reversibility::L1Snapshot,
    }
}

fn input() -> ApprovalRequestInput {
    ApprovalRequestInput {
        request_id: ApprovalRequestId::new("approval_coverage").expect("request"),
        origin: origin("task_coverage", "step_coverage", "session_coverage"),
        dimensions: dimensions(
            "user",
            "notepad.text.write",
            "notepad",
            "document",
            Effect::Write,
        ),
        risk: low_risk(),
        impact_summary: "replace one word".to_owned(),
        diff: Some(DiffPreview::Fields(vec![
            FieldDiff::new("text", "old", "new").expect("diff"),
        ])),
        requested_at_ms: 1_000,
        max_ttl_ms: 5_000,
        max_uses: 2,
    }
}

fn confirmation(scopes: &[&str], show_diff: Option<bool>) -> PolicyDecision {
    let mut value = serde_json::json!({
        "allow": true,
        "rule_id": "coverage_confirmation",
        "reason": "human confirmation required",
        "scope_options": scopes,
    });
    if let Some(show_diff) = show_diff {
        value
            .as_object_mut()
            .expect("object")
            .insert("show_diff".to_owned(), serde_json::json!(show_diff));
    }
    serde_json::from_value(value).expect("confirmation")
}

#[test]
fn test_scope_and_dimension_validation_are_stable_and_fail_closed() {
    let scopes = [
        ("once", ApprovalScope::Once),
        ("this_step_pattern", ApprovalScope::ThisStepPattern),
        ("this_task", ApprovalScope::ThisTask),
        ("this_app_session", ApprovalScope::ThisAppSession),
        ("persistent", ApprovalScope::Persistent),
    ];
    for (text, scope) in scopes {
        assert_eq!(scope.as_str(), text);
        assert_eq!(ApprovalScope::parse(text).expect("scope"), scope);
    }
    assert!(matches!(
        ApprovalScope::parse("unsupported"),
        Err(HitlError::InvalidPolicyConfirmation)
    ));

    assert_eq!(
        ToolAuthorization::new("notepad.text.write")
            .expect("tool")
            .as_str(),
        "notepad.text.write"
    );
    for invalid in [
        "",
        "notepad.text",
        "Notepad.text.write",
        "notepad..write",
        "notepad.text.write.extra",
    ] {
        assert!(matches!(
            ToolAuthorization::new(invalid),
            Err(HitlError::InvalidToolName { .. })
        ));
    }

    let target = TargetAuthorization::new("notepad", "document").expect("target");
    assert_eq!(target.app_id(), "notepad");
    assert_eq!(target.target_id(), "document");
    assert!(TargetAuthorization::new("", "document").is_err());
    assert!(TargetAuthorization::new("notepad", "bad\nvalue").is_err());
    assert!(TargetAuthorization::new("notepad", "x".repeat(257)).is_err());
    assert!(low_risk().validate_scope_options(&[]).is_err());
    assert!(
        low_risk()
            .validate_scope_options(&[ApprovalScope::Once])
            .is_ok()
    );
}

#[test]
fn test_approval_request_validation_rejects_every_expansion() {
    let policy = confirmation(&["once"], Some(false));
    let mut blank_summary = input();
    blank_summary.impact_summary = " ".to_owned();
    assert!(ApprovalRequest::from_policy_decision(blank_summary, &policy).is_err());
    let mut zero_ttl = input();
    zero_ttl.max_ttl_ms = 0;
    assert!(ApprovalRequest::from_policy_decision(zero_ttl, &policy).is_err());
    let mut zero_uses = input();
    zero_uses.max_uses = 0;
    assert!(ApprovalRequest::from_policy_decision(zero_uses, &policy).is_err());
    assert!(matches!(
        ApprovalRequest::from_policy_decision(input(), &confirmation(&["once"], None)),
        Err(HitlError::InvalidPolicyConfirmation)
    ));
    assert!(matches!(
        ApprovalRequest::from_policy_decision(
            input(),
            &confirmation(&["once", "once"], Some(false))
        ),
        Err(HitlError::InvalidPolicyConfirmation)
    ));
    let mut no_diff = input();
    no_diff.diff = None;
    assert!(matches!(
        ApprovalRequest::from_policy_decision(no_diff, &confirmation(&["once"], Some(true))),
        Err(HitlError::InvalidValue { .. })
    ));
    let mut overflow = input();
    overflow.requested_at_ms = i64::MAX;
    overflow.max_ttl_ms = 1;
    assert!(matches!(
        ApprovalRequest::from_policy_decision(overflow, &policy),
        Err(HitlError::InvalidValue { .. })
    ));
    let mut too_large = input();
    too_large.max_ttl_ms = u64::MAX;
    assert!(matches!(
        ApprovalRequest::from_policy_decision(too_large, &policy),
        Err(HitlError::InvalidValue { .. })
    ));
}

#[test]
fn test_approval_resolution_checks_time_scope_ttl_and_uses() {
    let request = ApprovalRequest::from_policy_decision(
        input(),
        &confirmation(&["once", "this_task"], Some(true)),
    )
    .expect("request");
    assert_eq!(request.request_id().as_str(), "approval_coverage");
    assert_eq!(request.origin().task_id.as_str(), "task_coverage");
    assert_eq!(request.dimensions().target.app_id(), "notepad");
    assert_eq!(request.risk().level, RiskLevel::Medium);
    assert_eq!(request.impact_summary(), "replace one word");
    assert!(request.diff().is_some());
    assert!(request.show_diff());
    assert_eq!(request.scope_options().len(), 2);
    assert_eq!(request.requested_at_ms(), 1_000);
    assert_eq!(request.expires_at_ms(), 6_000);
    assert_eq!(request.max_ttl_ms(), 5_000);
    assert_eq!(request.max_uses(), 2);
    assert!(!request.is_expired(5_999));
    assert!(request.is_expired(6_000));
    assert!(matches!(
        request.resolve(
            ApprovalDecision::Deny {
                reason: "later".to_owned(),
            },
            999
        ),
        Err(HitlError::ClockWentBackwards { .. })
    ));
    assert!(matches!(
        request.resolve(
            ApprovalDecision::Deny {
                reason: " ".to_owned(),
            },
            1_100
        ),
        Err(HitlError::InvalidValue { .. })
    ));
    assert!(matches!(
        request.resolve(
            ApprovalDecision::Deny {
                reason: "later".to_owned(),
            },
            6_000
        ),
        Ok(ApprovalOutcome::TimedOut)
    ));
    assert!(matches!(
        request.resolve(
            ApprovalDecision::Approve {
                subject: SubjectId::new("user").expect("subject"),
                authorization: AuthorizationRequest {
                    scope: ApprovalScope::Once,
                    ttl_ms: 6_000,
                    max_uses: 1,
                },
            },
            1_100,
        ),
        Err(HitlError::InvalidValue { .. })
    ));
    assert!(matches!(
        request.resolve(
            ApprovalDecision::Approve {
                subject: SubjectId::new("user").expect("subject"),
                authorization: AuthorizationRequest {
                    scope: ApprovalScope::Once,
                    ttl_ms: 1_000,
                    max_uses: 3,
                },
            },
            1_100,
        ),
        Err(HitlError::InvalidValue { .. })
    ));
}

#[test]
fn test_authorization_rejects_risk_and_timestamp_expansion() {
    let zero_ttl = AuthorizationRequest {
        scope: ApprovalScope::Once,
        ttl_ms: 0,
        max_uses: 1,
    };
    assert!(zero_ttl.validate_for_risk(&low_risk()).is_err());
    let zero_uses = AuthorizationRequest {
        scope: ApprovalScope::Once,
        ttl_ms: 1,
        max_uses: 0,
    };
    assert!(zero_uses.validate_for_risk(&low_risk()).is_err());
    let overflow = AuthorizationRequest {
        scope: ApprovalScope::Once,
        ttl_ms: u64::MAX,
        max_uses: 1,
    };
    assert!(matches!(
        AuthorizationGrant::issue(
            dimensions(
                "user",
                "notepad.text.write",
                "notepad",
                "document",
                Effect::Write
            ),
            origin("task", "step", "session"),
            &overflow,
            &low_risk(),
            1_000
        ),
        Err(HitlError::InvalidValue { .. })
    ));
}

#[test]
fn test_error_codes_and_diff_builders_expose_stable_failures() {
    assert_error_code!(HitlError::ApprovalNotRequired, ErrorCode::ToolInvalidArgs);
    assert_error_code!(HitlError::PolicyDenied, ErrorCode::PolicyDenied);
    assert_error_code!(
        HitlError::InvalidPolicyConfirmation,
        ErrorCode::ToolInvalidArgs
    );
    assert_error_code!(HitlError::ScopeNotOffered, ErrorCode::UserInteraction);
    assert_error_code!(
        HitlError::InvalidIdentifier {
            kind: "subject",
            reason: "empty".to_owned(),
        },
        ErrorCode::ToolInvalidArgs
    );
    assert_error_code!(
        HitlError::InvalidToolName {
            tool: "bad".to_owned(),
        },
        ErrorCode::ToolInvalidArgs
    );
    assert_error_code!(
        HitlError::InvalidValue {
            reason: "bad".to_owned(),
        },
        ErrorCode::ToolInvalidArgs
    );
    assert_error_code!(
        HitlError::ForbiddenAuthorizationScope {
            scope: "persistent".to_owned(),
        },
        ErrorCode::PolicyDenied
    );
    assert_error_code!(
        HitlError::ApprovalExpired {
            expires_at_ms: 1,
            now_ms: 2,
        },
        ErrorCode::UserInteraction
    );
    assert_error_code!(
        HitlError::AuthorizationDenied {
            reason: "Expired".to_owned(),
        },
        ErrorCode::UserInteraction
    );
    assert_error_code!(
        HitlError::ClockWentBackwards {
            previous_ms: 2,
            now_ms: 1,
        },
        ErrorCode::Transient
    );
    assert_error_code!(
        HitlError::TakeoverNotActive {
            task_id: "task".to_owned(),
        },
        ErrorCode::UserInteraction
    );
    assert_error_code!(
        HitlError::TakeoverAlreadyActive {
            task_id: "task".to_owned(),
        },
        ErrorCode::UserInteraction
    );
    assert_error_code!(
        HitlError::DiffBudgetExceeded {
            required_cells: 2,
            max_cells: 1,
        },
        ErrorCode::ToolInvalidArgs
    );
    assert_error_code!(
        HitlError::TaskEngine(TaskEngineError::TaskNotFound {
            task_id: "task".to_owned(),
        }),
        ErrorCode::TargetNotFound
    );
}

#[test]
fn test_diff_builders_expose_stable_failures() {
    assert!(FieldDiff::new("", "old", "new").is_err());
    assert!(FieldDiff::new("field", "same", "same").is_err());
    assert!(FileDiff::new("new.txt", FileChangeKind::Added, None, Some(1)).is_ok());
    assert!(FileDiff::new("old.txt", FileChangeKind::Removed, Some(1), None).is_ok());
    assert!(FileDiff::new("changed.txt", FileChangeKind::Modified, Some(1), Some(2)).is_ok());
    assert!(FileDiff::new("", FileChangeKind::Modified, Some(1), Some(2)).is_err());
    assert!(UiStepDiff::new("", "save", "target", None).is_err());
    assert!(UiStepDiff::new("step", "", "target", None).is_err());
    assert!(UiStepDiff::new("step", "save", "", None).is_err());
    assert!(IrreversibleDiff::new(" ").is_err());
    let text = DiffPreview::Text(TextDiffPreview::prepare("", "new\n", 8).expect("diff"));
    assert!(!text.requires_secondary_confirmation());
}
