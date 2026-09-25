//! Contract tests for reversibility selection and anchor validation.

use assistant_platform_api::{ErrorCode, Fingerprint};
use assistant_undo::{
    Anchor, AnchorAvailability, AnchorId, AnchorKind, AnchorStrategy, AnchorTargetKind,
    ContentDigest, EvidenceAvailability, Reversibility, ShadowCopy, ShadowCopyPath, StepId,
    TargetId, UndoBudget, UndoStepCount, UndoValidity, plan_anchor_strategy,
};

fn fingerprint(hex: char) -> Result<Fingerprint, Box<dyn std::error::Error>> {
    let digest = hex.to_string().repeat(64);
    Ok(Fingerprint::parse(format!("sha256:{digest}"))?)
}

fn digest(hex: char) -> Result<ContentDigest, Box<dyn std::error::Error>> {
    Ok(ContentDigest::parse(hex.to_string().repeat(64))?)
}

const fn availability(
    undo_stack: EvidenceAvailability,
    complete_content: EvidenceAvailability,
    file_shadow_copy: EvidenceAvailability,
    compensating_recipe: EvidenceAvailability,
    target_kind: AnchorTargetKind,
) -> AnchorAvailability {
    AnchorAvailability::new(
        undo_stack,
        complete_content,
        file_shadow_copy,
        compensating_recipe,
        target_kind,
    )
}

#[test]
fn test_reversibility_parse_accepts_all_four_levels() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("L0_undo_stack", Reversibility::L0UndoStack),
        ("L1_snapshot", Reversibility::L1Snapshot),
        ("L2_compensating", Reversibility::L2Compensating),
        ("L3_irreversible", Reversibility::L3Irreversible),
    ];
    for (value, expected) in cases {
        assert_eq!(Reversibility::parse(value)?, expected);
        assert_eq!(expected.as_str(), value);
    }
    Ok(())
}

#[test]
fn test_reversibility_worst_uses_bucket_rule() {
    let worst = Reversibility::worst([
        Reversibility::L0UndoStack,
        Reversibility::L2Compensating,
        Reversibility::L1Snapshot,
    ]);
    assert_eq!(worst, Some(Reversibility::L2Compensating));
    assert!(Reversibility::L3Irreversible.requires_human_confirmation());
    assert!(!Reversibility::L2Compensating.requires_human_confirmation());
}

#[test]
fn test_reversibility_parse_rejects_unknown_and_read_only() {
    assert_eq!(
        Reversibility::parse("L4_future").map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        Reversibility::parse("none_readonly").map_err(|error| error.error_code()),
        Err(ErrorCode::CapabilityMissing)
    );
}

#[test]
fn test_plan_anchor_selects_l0_only_with_undo_capability() -> Result<(), Box<dyn std::error::Error>>
{
    let selected = plan_anchor_strategy(
        Reversibility::L0UndoStack,
        &availability(
            EvidenceAvailability::Available,
            EvidenceAvailability::Available,
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Unavailable,
            AnchorTargetKind::Document,
        ),
    )?;
    assert_eq!(selected, AnchorStrategy::UndoStack);

    let missing = plan_anchor_strategy(
        Reversibility::L0UndoStack,
        &availability(
            EvidenceAvailability::Unknown,
            EvidenceAvailability::Available,
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Unavailable,
            AnchorTargetKind::Document,
        ),
    );
    assert_eq!(
        missing.map_err(|error| error.error_code()),
        Err(ErrorCode::CapabilityMissing)
    );
    Ok(())
}

#[test]
fn test_plan_anchor_l1_uses_shadow_for_file_and_content_for_document()
-> Result<(), Box<dyn std::error::Error>> {
    let file = plan_anchor_strategy(
        Reversibility::L1Snapshot,
        &availability(
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Available,
            EvidenceAvailability::Available,
            EvidenceAvailability::Unavailable,
            AnchorTargetKind::File,
        ),
    )?;
    assert_eq!(file, AnchorStrategy::ShadowCopy);

    let document = plan_anchor_strategy(
        Reversibility::L1Snapshot,
        &availability(
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Available,
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Unavailable,
            AnchorTargetKind::Document,
        ),
    )?;
    assert_eq!(document, AnchorStrategy::ContentSnapshot);
    Ok(())
}

#[test]
fn test_plan_anchor_l2_requires_recipe_and_l3_is_evidence_only()
-> Result<(), Box<dyn std::error::Error>> {
    let missing = plan_anchor_strategy(
        Reversibility::L2Compensating,
        &availability(
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Unavailable,
            EvidenceAvailability::Unavailable,
            AnchorTargetKind::Unknown,
        ),
    );
    assert_eq!(
        missing.map_err(|error| error.error_code()),
        Err(ErrorCode::CapabilityMissing)
    );

    let evidence_only = plan_anchor_strategy(
        Reversibility::L3Irreversible,
        &availability(
            EvidenceAvailability::Unknown,
            EvidenceAvailability::Unknown,
            EvidenceAvailability::Unknown,
            EvidenceAvailability::Unknown,
            AnchorTargetKind::Unknown,
        ),
    )?;
    assert_eq!(evidence_only, AnchorStrategy::EvidenceOnly);
    Ok(())
}

#[test]
fn test_anchor_new_rejects_mismatched_payload() -> Result<(), Box<dyn std::error::Error>> {
    let result = Anchor::new(
        AnchorId::parse("a_1")?,
        StepId::parse("s_1")?,
        TargetId::parse("document_1")?,
        Reversibility::L1Snapshot,
        fingerprint('0')?,
        AnchorKind::UndoStack {
            budget: UndoBudget::new(UndoStepCount::new(1)?, UndoValidity::DocumentClose),
            fallback_snapshot: None,
        },
    );
    assert_eq!(
        result.map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    Ok(())
}

#[test]
fn test_l2_anchor_rejects_non_compensating_action() -> Result<(), Box<dyn std::error::Error>> {
    let recipe = assistant_undo::RollbackRecipe::new(
        AnchorId::parse("a_1")?,
        vec![assistant_undo::RollbackAction::UndoStack {
            times: UndoStepCount::new(1)?,
            until_fingerprint: fingerprint('0')?,
        }],
    )?;
    let result = Anchor::new(
        AnchorId::parse("a_1")?,
        StepId::parse("s_1")?,
        TargetId::parse("document_1")?,
        Reversibility::L2Compensating,
        fingerprint('0')?,
        AnchorKind::CompensatingAction { recipe },
    );
    assert_eq!(
        result.map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    Ok(())
}

#[test]
fn test_anchor_post_fingerprint_cannot_be_overwritten() -> Result<(), Box<dyn std::error::Error>> {
    let mut anchor = Anchor::new(
        AnchorId::parse("a_1")?,
        StepId::parse("s_1")?,
        TargetId::parse("file_1")?,
        Reversibility::L1Snapshot,
        fingerprint('0')?,
        AnchorKind::ShadowCopy {
            shadow_copy: ShadowCopy::new(ShadowCopyPath::parse("shadow/file_1.bak")?, digest('1')?),
        },
    )?;
    anchor.record_post_fingerprint(fingerprint('2')?)?;
    let second = anchor.record_post_fingerprint(fingerprint('3')?);
    assert_eq!(
        second.map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    Ok(())
}

#[test]
fn test_shadow_copy_path_rejects_traversal() {
    let result = ShadowCopyPath::parse("shadow/../file.bak");
    assert_eq!(
        result.map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
}
