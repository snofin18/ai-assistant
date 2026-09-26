//! Four-dimensional authorization contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_hitl::{
    ApprovalRisk, ApprovalScope, AuthorizationContext, AuthorizationDecision, AuthorizationDenial,
    AuthorizationDimensions, AuthorizationGrant, AuthorizationOrigin, AuthorizationRequest,
    HitlError, SubjectId, TargetAuthorization, ToolAuthorization,
};
use assistant_policy::Effect;
use assistant_protocol::RiskLevel;
use assistant_task_engine::{Reversibility, StepId, TaskId};

use assistant_hitl::AppSessionId;

fn dimensions(subject: &str, tool: &str, app: &str, target: &str) -> AuthorizationDimensions {
    AuthorizationDimensions {
        subject: SubjectId::new(subject).expect("subject"),
        tool: ToolAuthorization::new(tool).expect("tool"),
        target: TargetAuthorization::new(app, target).expect("target"),
        effect: Effect::Write,
    }
}

fn context(
    dimensions: AuthorizationDimensions,
    task: &str,
    step: &str,
    session: &str,
) -> AuthorizationContext {
    AuthorizationContext {
        dimensions,
        origin: AuthorizationOrigin {
            task_id: TaskId::new(task).expect("task"),
            step_id: StepId::new(step).expect("step"),
            app_session_id: AppSessionId::new(session).expect("session"),
        },
    }
}

fn grant(
    scope: ApprovalScope,
    dimensions: AuthorizationDimensions,
    task: &str,
    step: &str,
    session: &str,
    remaining_uses: u32,
) -> AuthorizationGrant {
    AuthorizationGrant::issue(
        dimensions,
        AuthorizationOrigin {
            task_id: TaskId::new(task).expect("task"),
            step_id: StepId::new(step).expect("step"),
            app_session_id: AppSessionId::new(session).expect("session"),
        },
        &AuthorizationRequest {
            scope,
            ttl_ms: 1_000,
            max_uses: remaining_uses,
        },
        &ApprovalRisk {
            level: RiskLevel::Medium,
            reversibility: Reversibility::L1Snapshot,
        },
        1_000,
    )
    .expect("grant")
}

#[test]
fn test_high_risk_only_allows_one_once_use() {
    let risk = ApprovalRisk {
        level: RiskLevel::High,
        reversibility: Reversibility::L1Snapshot,
    };
    let request = AuthorizationRequest {
        scope: ApprovalScope::ThisTask,
        ttl_ms: 1_000,
        max_uses: 1,
    };
    assert!(matches!(
        request.validate_for_risk(&risk),
        Err(HitlError::ForbiddenAuthorizationScope { .. })
    ));

    let once = AuthorizationRequest {
        scope: ApprovalScope::Once,
        ttl_ms: 1_000,
        max_uses: 1,
    };
    assert!(once.validate_for_risk(&risk).is_ok());
    assert!(risk.validate_scope_options(&[ApprovalScope::Once]).is_ok());
    assert!(
        risk.validate_scope_options(&[ApprovalScope::Persistent])
            .is_err()
    );
}

#[test]
fn test_once_requires_exact_target_step_and_not_expired() {
    let dims = dimensions("user", "notepad.text.write", "notepad", "doc_1");
    let grant = grant(ApprovalScope::Once, dims.clone(), "t_1", "s_1", "sess_1", 1);
    let exact = context(dims.clone(), "t_1", "s_1", "sess_1");
    assert!(grant.covers(&exact, 1_999).is_allowed());
    assert_eq!(
        grant.covers(&exact, 2_000),
        AuthorizationDecision::Denied(AuthorizationDenial::Expired)
    );

    let other_step = context(dims, "t_1", "s_2", "sess_1");
    assert_eq!(
        grant.covers(&other_step, 1_000),
        AuthorizationDecision::Denied(AuthorizationDenial::StepMismatch)
    );
    let other_target = context(
        dimensions("user", "notepad.text.write", "notepad", "doc_2"),
        "t_1",
        "s_1",
        "sess_1",
    );
    assert_eq!(
        grant.covers(&other_target, 1_000),
        AuthorizationDecision::Denied(AuthorizationDenial::TargetMismatch)
    );
}

#[test]
fn test_task_and_app_session_scopes_are_bounded_by_their_origin() {
    let dims = dimensions("user", "notepad.text.write", "notepad", "doc_1");
    let task_grant = grant(
        ApprovalScope::ThisTask,
        dims.clone(),
        "t_1",
        "s_1",
        "sess_1",
        2,
    );
    let other_doc = context(
        dimensions("user", "notepad.text.write", "notepad", "doc_2"),
        "t_1",
        "s_2",
        "sess_1",
    );
    assert!(task_grant.covers(&other_doc, 1_000).is_allowed());
    let other_task = context(dims.clone(), "t_2", "s_2", "sess_1");
    assert_eq!(
        task_grant.covers(&other_task, 1_000),
        AuthorizationDecision::Denied(AuthorizationDenial::TaskMismatch)
    );

    let session_grant = grant(
        ApprovalScope::ThisAppSession,
        dims.clone(),
        "t_1",
        "s_1",
        "sess_1",
        2,
    );
    assert!(session_grant.covers(&other_task, 1_000).is_allowed());
    let other_session = context(dims, "t_2", "s_2", "sess_2");
    assert_eq!(
        session_grant.covers(&other_session, 1_000),
        AuthorizationDecision::Denied(AuthorizationDenial::AppSessionMismatch)
    );
}

#[test]
fn test_persistent_keeps_exact_target_and_consumes_bounded_uses() {
    let dims = dimensions("user", "notepad.text.write", "notepad", "doc_1");
    let persistent = grant(
        ApprovalScope::Persistent,
        dims.clone(),
        "t_1",
        "s_1",
        "sess_1",
        1,
    );
    let later = context(dims, "t_9", "s_9", "sess_9");
    assert!(persistent.covers(&later, 1_000).is_allowed());
    let consumed = persistent.consume_for(&later, 1_000).expect("consume");
    assert_eq!(consumed.remaining_uses(), 0);
    assert_eq!(
        consumed.covers(&later, 1_001),
        AuthorizationDecision::Denied(AuthorizationDenial::UsesExhausted)
    );
}
