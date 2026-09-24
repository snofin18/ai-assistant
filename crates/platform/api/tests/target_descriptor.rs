//! `TargetDescriptor` 的一致性校验：正向 + 负向（ADR-0019 N1）。
//!
//! 为什么放在 `tests/`：校验只依赖公开 API，放这里让 `src/target.rs` 保持在软上限附近。
//! 断言风格见 `tests/common/mod.rs`。

mod common;

use assistant_platform_api::{
    ErrorCode, OnAmbiguous, OnNotFound, ResolutionPolicy, SelectorCandidate, SelectorKind,
    SelectorValue, TargetDescriptor,
};
use common::ok_or_fail;

fn candidate(id: &str, score: f64, locale_dependent: bool) -> SelectorCandidate {
    ok_or_fail(
        SelectorCandidate::new(
            id,
            SelectorKind::AutomationId,
            SelectorValue::Text(id.to_string()),
            score,
            locale_dependent,
        ),
        "测试用的候选必须合法",
    )
}

fn policy() -> ResolutionPolicy {
    ok_or_fail(
        ResolutionPolicy::new(
            OnAmbiguous::ErrorAndAsk,
            OnNotFound::new(vec![200, 500], true),
            5000,
            0.3,
        ),
        "测试用的解析策略必须合法",
    )
}

fn descriptor(
    window_candidates: Vec<SelectorCandidate>,
    element_candidates: Vec<SelectorCandidate>,
) -> TargetDescriptor {
    TargetDescriptor::new(
        "2.0".to_string(),
        "com.microsoft.notepad".to_string(),
        window_candidates,
        element_candidates,
        policy(),
    )
}

#[test]
fn test_candidate_rejects_blank_id_and_bad_score() {
    for (id, score) in [("", 0.5), ("a", 1.5), ("a", f64::NAN)] {
        assert!(
            SelectorCandidate::new(
                id,
                SelectorKind::AutomationId,
                SelectorValue::Text("x".into()),
                score,
                false
            )
            .is_err(),
            "id={id:?} score={score} 必须被拒绝"
        );
    }
}

#[test]
fn test_locale_dependent_candidate_is_halved_only_on_locale_mismatch() {
    let localized = candidate("w4", 0.4, true);
    assert!((localized.effective_score("zh-CN", "zh-CN") - 0.4).abs() < 1e-9);
    assert!((localized.effective_score("zh-CN", "en-US") - 0.2).abs() < 1e-9);
    let stable = candidate("w2", 0.4, false);
    assert!((stable.effective_score("zh-CN", "en-US") - 0.4).abs() < 1e-9);
}

#[test]
fn test_policy_rejects_zero_timeout_and_bad_threshold() {
    assert!(
        ResolutionPolicy::new(
            OnAmbiguous::ErrorAndAsk,
            OnNotFound::new(vec![], false),
            0,
            0.3
        )
        .is_err(),
        "超时 0 必须被拒绝"
    );
    assert!(
        ResolutionPolicy::new(
            OnAmbiguous::ErrorAndAsk,
            OnNotFound::new(vec![], false),
            100,
            1.1
        )
        .is_err(),
        "置信度阈值 > 1 必须被拒绝"
    );
}

#[test]
fn test_validate_accepts_well_formed_descriptor() {
    let validated = descriptor(
        vec![candidate("w1", 0.99, false)],
        vec![candidate("e1", 0.98, false)],
    )
    .validate();
    assert!(validated.is_ok(), "合法描述符必须通过: {validated:?}");
}

#[test]
fn test_validate_rejects_empty_window_chain() {
    assert_eq!(
        descriptor(vec![], vec![])
            .validate()
            .err()
            .map(|error| error.code()),
        Some(ErrorCode::ToolInvalidArgs)
    );
}

#[test]
fn test_validate_rejects_duplicate_candidate_id() {
    let validated = descriptor(
        vec![candidate("w1", 0.9, false), candidate("w1", 0.5, false)],
        vec![],
    )
    .validate();
    assert!(
        validated
            .as_ref()
            .err()
            .is_some_and(|error| error.message().contains("duplicate")),
        "重复 id 必须报出 duplicate: {validated:?}"
    );
}

#[test]
fn test_validate_rejects_blank_app_id() {
    let broken = TargetDescriptor::new(
        "2.0".to_string(),
        "   ".to_string(),
        vec![candidate("w1", 0.9, false)],
        vec![],
        policy(),
    );
    assert!(broken.validate().is_err());
}

#[test]
fn test_ttl_is_carried_through() {
    let candidate = candidate("w1", 0.99, false).with_ttl_ms(0);
    assert_eq!(candidate.ttl_ms(), Some(0));
    assert_eq!(candidate.kind(), SelectorKind::AutomationId);
    assert!(!candidate.is_locale_dependent());
}
