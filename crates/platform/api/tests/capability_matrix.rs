//! 能力矩阵的 4 条不变量：正向 + **每条一个负向用例**（ADR-0019 N1）。
//!
//! 为什么放在 `tests/`：不变量是**对外契约**，用公开 API 就能验；放这里同时让 `src/matrix.rs`
//! 保持在 gov §5.4 的 600 行硬上限之下。
//!
//! 断言风格：比较**整个 `Result`**（`.ok()` / `.err()` / `matches!`），
//! 因此本文件不需要 `tests/common` 的取 `Ok` 值工具。

use assistant_platform_api::{
    ApprovalRequirement, CapabilityEntry, CapabilityMatrix, CapabilityMatrixError,
    ChannelAvailability, ChannelState, Degradation, ErrorCode, PlatformError, ProbeContext,
    ResourceAccess, RiskLevel, SideEffect,
};
use std::collections::BTreeMap;

fn entry(id: &str, risk: RiskLevel, approval: ApprovalRequirement) -> CapabilityEntry {
    CapabilityEntry::new(
        id.to_string(),
        ResourceAccess::Read,
        SideEffect::None,
        risk,
        approval,
    )
}

fn probe() -> ProbeContext {
    ProbeContext::new(
        "2026-09-24T00:00:00Z".to_string(),
        "windows".to_string(),
        false,
        false,
        false,
    )
}

fn matrix(capabilities: Vec<CapabilityEntry>) -> CapabilityMatrix {
    CapabilityMatrix::new(probe(), BTreeMap::new(), capabilities, vec![])
}

#[test]
fn test_resource_access_is_exactly_five() {
    assert_eq!(ResourceAccess::ALL.len(), 5);
}

#[test]
fn test_resource_access_parse_rejects_vague_words() {
    // 负向：`modify` 这类模糊词被 spec §4 不变量 3 明令禁止。
    for raw in ["modify", "READ", "", "delete"] {
        assert!(
            matches!(ResourceAccess::parse(raw), Err(error) if error.code() == ErrorCode::CapabilityMissing),
            "`{raw}` 必须被拒绝"
        );
    }
    for access in ResourceAccess::ALL {
        assert_eq!(
            ResourceAccess::parse(access.as_str()).ok(),
            Some(access),
            "`{}` 必须能往返",
            access.as_str()
        );
    }
}

#[test]
fn test_side_effect_parse_requires_explicit_value() {
    assert!(matches!(
        SideEffect::parse(""),
        Err(error) if error.code() == ErrorCode::CapabilityMissing
    ));
    assert!(SideEffect::parse("focus change").is_err(), "措辞必须规范化");
    assert_eq!(SideEffect::parse("none").ok(), Some(SideEffect::None));
    assert_eq!(
        SideEffect::parse("network_egress").ok(),
        Some(SideEffect::NetworkEgress)
    );
}

#[test]
fn test_risk_escalate_rejects_downgrade() {
    assert_eq!(
        RiskLevel::escalate(RiskLevel::L3, RiskLevel::L2).err(),
        Some(CapabilityMatrixError::RiskDowngrade {
            from: RiskLevel::L3,
            to: RiskLevel::L2
        })
    );
    assert_eq!(
        RiskLevel::escalate(RiskLevel::L3, RiskLevel::L4).ok(),
        Some(RiskLevel::L4)
    );
    assert_eq!(
        RiskLevel::escalate(RiskLevel::L3, RiskLevel::L3).ok(),
        Some(RiskLevel::L3)
    );
}

#[test]
fn test_required_approval_mapping_matches_spec() {
    assert_eq!(RiskLevel::L1.required_approval(), ApprovalRequirement::Auto);
    assert_eq!(RiskLevel::L2.required_approval(), ApprovalRequirement::Auto);
    assert_eq!(
        RiskLevel::L3.required_approval(),
        ApprovalRequirement::Required
    );
    assert_eq!(
        RiskLevel::L4.required_approval(),
        ApprovalRequirement::Required
    );
    assert_eq!(
        RiskLevel::L5.required_approval(),
        ApprovalRequirement::Forbidden
    );
    assert_eq!(RiskLevel::L1.rank(), 1);
    assert_eq!(RiskLevel::L5.rank(), 5);
}

#[test]
fn test_probe_context_is_carried_into_the_matrix() {
    let context = ProbeContext::new(
        "2026-09-24T00:00:00Z".to_string(),
        "linux".to_string(),
        true,
        true,
        false,
    );
    assert_eq!(context.probed_at(), "2026-09-24T00:00:00Z");
    assert_eq!(context.platform_os(), "linux");
    assert!(context.session_locked());
    assert!(context.session_remote());
    assert!(!context.session_headless());

    let matrix = CapabilityMatrix::new(
        context.clone(),
        BTreeMap::new(),
        vec![entry(
            "WindowRead",
            RiskLevel::L1,
            ApprovalRequirement::Auto,
        )],
        vec![],
    );
    // 读取侧不因内部重组而漂移：`probe()` 与 5 个直读访问器必须同源。
    assert_eq!(matrix.probe(), &context);
    assert_eq!(matrix.probed_at(), context.probed_at());
    assert_eq!(matrix.platform_os(), context.platform_os());
    assert_eq!(matrix.session_locked(), context.session_locked());
    assert_eq!(matrix.session_remote(), context.session_remote());
    assert_eq!(matrix.session_headless(), context.session_headless());
}

#[test]
fn test_validate_accepts_a_spec_shaped_matrix() {
    let validated = matrix(vec![
        entry("WindowEnumerate", RiskLevel::L1, ApprovalRequirement::Auto),
        entry("FileWrite", RiskLevel::L2, ApprovalRequirement::Auto),
        entry("FileDelete", RiskLevel::L3, ApprovalRequirement::Required),
        entry("MoneySend", RiskLevel::L5, ApprovalRequirement::Forbidden),
    ])
    .validate();
    assert!(validated.is_ok(), "spec 形状的矩阵必须通过: {validated:?}");
}

#[test]
fn test_validate_rejects_empty_capability_list() {
    assert_eq!(
        matrix(vec![]).validate().err(),
        Some(CapabilityMatrixError::EmptyCapabilityList)
    );
}

#[test]
fn test_validate_rejects_duplicate_and_blank_ids() {
    let duplicated = matrix(vec![
        entry("FileRead", RiskLevel::L1, ApprovalRequirement::Auto),
        entry("FileRead", RiskLevel::L2, ApprovalRequirement::Auto),
    ]);
    assert_eq!(
        duplicated.validate().err(),
        Some(CapabilityMatrixError::DuplicateCapabilityId(
            "FileRead".to_string()
        ))
    );
    let blank = matrix(vec![entry("  ", RiskLevel::L1, ApprovalRequirement::Auto)]);
    assert_eq!(
        blank.validate().err(),
        Some(CapabilityMatrixError::BlankCapabilityId)
    );
}

#[test]
fn test_validate_rejects_l3_without_required_approval() {
    let bad = matrix(vec![entry(
        "ProcessSpawn",
        RiskLevel::L3,
        ApprovalRequirement::Auto,
    )]);
    assert!(matches!(
        bad.validate(),
        Err(CapabilityMatrixError::ApprovalMismatch { .. })
    ));
}

#[test]
fn test_validate_rejects_l5_that_is_not_forbidden() {
    for approval in [ApprovalRequirement::Auto, ApprovalRequirement::Required] {
        let bad = matrix(vec![entry("MoneySend", RiskLevel::L5, approval)]);
        assert!(
            bad.validate().is_err(),
            "L5 with {approval:?} must fail（铁律 6）"
        );
    }
}

#[test]
fn test_validate_rejects_l1_declared_as_forbidden() {
    let bad = matrix(vec![entry(
        "WindowRead",
        RiskLevel::L1,
        ApprovalRequirement::Forbidden,
    )]);
    assert!(bad.validate().is_err(), "L1 当永久拒绝 = 把能力错误地关死");
}

#[test]
fn test_matrix_error_converts_to_platform_error_with_code() {
    let error: PlatformError = CapabilityMatrixError::EmptyCapabilityList.into();
    assert_eq!(error.code(), ErrorCode::CapabilityMissing);
    assert!(error.message().contains("capability"));
}

#[test]
fn test_channel_state_and_degradation_are_inspectable() {
    let state = ChannelState::new(
        ChannelAvailability::NeedsConsent,
        Some("libei/EIS".to_string()),
    );
    assert_eq!(state.availability(), ChannelAvailability::NeedsConsent);
    assert_eq!(state.detail(), Some("libei/EIS"));
    let degradation = Degradation::new("no_silent_screenshot".to_string(), "需授权".to_string());
    assert_eq!(degradation.id(), "no_silent_screenshot");
    assert!(!degradation.impact().is_empty());
}
