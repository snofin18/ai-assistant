//! Coverage-focused contract tests for rollback errors, recipes, conflicts, and incidents.

use std::cell::RefCell;
use std::collections::VecDeque;

use assistant_platform_api::{ErrorCode, Fingerprint};
use assistant_undo::{
    ActionOutcome, Anchor, AnchorId, AnchorKind, AnchorStrategy, ConflictResolution, ContentDigest,
    ExecutorFailure, IncidentId, IncidentKind, IncidentReporter, IncidentSeverity, Reversibility,
    RollbackAction, RollbackConflict, RollbackExecutor, RollbackOutcome, RollbackRecipe,
    RollbackRequest, ShadowCopy, ShadowCopyPath, StepId, TargetId, ToolInvocation, ToolName,
    UndoBudget, UndoError, UndoStepCount, UndoValidity, blocks_rollback, build_fallback_recipe,
    build_rollback_recipe, detect_conflict, execute_rollback, publish_incident,
    validate_rollback_recipe,
};

fn fp(hex: char) -> Result<Fingerprint, Box<dyn std::error::Error>> {
    Ok(Fingerprint::parse(format!(
        "sha256:{}",
        hex.to_string().repeat(64)
    ))?)
}

fn digest(hex: char) -> Result<ContentDigest, Box<dyn std::error::Error>> {
    Ok(ContentDigest::parse(hex.to_string().repeat(64))?)
}

fn anchor_id(value: &str) -> Result<AnchorId, Box<dyn std::error::Error>> {
    Ok(AnchorId::parse(value)?)
}

fn anchor_l0(
    post: Fingerprint,
    fallback: Option<ContentDigest>,
) -> Result<Anchor, Box<dyn std::error::Error>> {
    let mut anchor = Anchor::new(
        anchor_id("a_1")?,
        StepId::parse("s_1")?,
        TargetId::parse("document_1")?,
        Reversibility::L0UndoStack,
        fp('0')?,
        AnchorKind::UndoStack {
            budget: UndoBudget::new(UndoStepCount::new(3)?, UndoValidity::DocumentClose),
            fallback_snapshot: fallback,
        },
    )?;
    anchor.record_post_fingerprint(post)?;
    Ok(anchor)
}

fn anchor_shadow(post: Fingerprint) -> Result<Anchor, Box<dyn std::error::Error>> {
    let mut anchor = Anchor::new(
        anchor_id("a_shadow")?,
        StepId::parse("s_shadow")?,
        TargetId::parse("file_1")?,
        Reversibility::L1Snapshot,
        fp('0')?,
        AnchorKind::ShadowCopy {
            shadow_copy: ShadowCopy::new(ShadowCopyPath::parse("shadow/file_1.bak")?, digest('1')?),
        },
    )?;
    anchor.record_post_fingerprint(post)?;
    Ok(anchor)
}

#[derive(Default)]
struct Executor {
    outcomes: RefCell<VecDeque<Result<ActionOutcome, ExecutorFailure>>>,
}

impl Executor {
    fn new(outcomes: Vec<Result<ActionOutcome, ExecutorFailure>>) -> Self {
        Self {
            outcomes: RefCell::new(VecDeque::from(outcomes)),
        }
    }
}

impl RollbackExecutor for Executor {
    fn execute(&self, _action: &RollbackAction) -> Result<ActionOutcome, ExecutorFailure> {
        self.outcomes.borrow_mut().pop_front().unwrap_or_else(|| {
            Err(ExecutorFailure::new(
                ErrorCode::Transient,
                "scripted executor exhausted",
            ))
        })
    }
}

struct FailingReporter;

impl IncidentReporter for FailingReporter {
    fn report(
        &self,
        _report: &assistant_undo::IncidentReport,
    ) -> Result<(), assistant_undo::ReportFailure> {
        Err(assistant_undo::ReportFailure::new(
            ErrorCode::Fatal,
            "audit unavailable",
        ))
    }
}

macro_rules! expect_error {
    ($error:expr, $code:expr, $text:expr) => {{
        let error = $error;
        assert_eq!(error.error_code(), $code);
        assert_eq!(error.to_string(), $text);
    }};
}

#[test]
fn test_error_mapping_and_display_for_validation_variants() -> Result<(), Box<dyn std::error::Error>>
{
    let anchor = anchor_id("a_1")?;
    expect_error!(
        UndoError::InvalidIdentifier {
            kind: "anchor_id",
            reason: "must not be empty".to_owned(),
        },
        ErrorCode::ToolInvalidArgs,
        "invalid anchor_id: must not be empty"
    );
    expect_error!(
        UndoError::InvalidContentDigest {
            reason: "bad digest".to_owned(),
        },
        ErrorCode::ToolInvalidArgs,
        "invalid content digest: bad digest"
    );
    expect_error!(
        UndoError::InvalidShadowCopyPath {
            reason: "bad path".to_owned(),
        },
        ErrorCode::ToolInvalidArgs,
        "invalid shadow-copy path: bad path"
    );
    expect_error!(
        UndoError::InvalidUndoStepCount { value: 0 },
        ErrorCode::ToolInvalidArgs,
        "undo step count 0 is outside 1..=50"
    );
    expect_error!(
        UndoError::InvalidToolName {
            value: "Bad".to_owned(),
        },
        ErrorCode::ToolInvalidArgs,
        "tool name `Bad` is not <app>.<domain>.<action>"
    );
    expect_error!(
        UndoError::InvalidCompensatingArguments {
            reason: "empty".to_owned(),
        },
        ErrorCode::ToolInvalidArgs,
        "invalid compensating-action arguments: empty"
    );
    expect_error!(
        UndoError::EmptyRollbackRecipe { anchor_id: anchor },
        ErrorCode::ToolInvalidArgs,
        "rollback recipe for anchor `a_1` is empty"
    );
    Ok(())
}

#[test]
fn test_error_mapping_and_display_for_lifecycle_variants() -> Result<(), Box<dyn std::error::Error>>
{
    let anchor = anchor_id("a_1")?;
    expect_error!(
        UndoError::RollbackRecipeTooLong {
            count: 65,
            maximum: 64,
        },
        ErrorCode::ToolInvalidArgs,
        "rollback recipe has 65 actions; maximum is 64"
    );
    expect_error!(
        UndoError::RecipeAnchorMismatch {
            anchor_id: anchor,
            recipe_anchor_id: anchor_id("a_2")?,
        },
        ErrorCode::ToolInvalidArgs,
        "recipe anchor `a_2` does not match requested anchor `a_1`"
    );
    expect_error!(
        UndoError::AnchorKindMismatch {
            reversibility: Reversibility::L1Snapshot,
            strategy: AnchorStrategy::UndoStack,
        },
        ErrorCode::ToolInvalidArgs,
        "anchor strategy `undo_stack` is not valid for reversibility `L1_snapshot`"
    );
    expect_error!(
        UndoError::CapabilityMissing {
            capability: "storage.shadow_copy",
            context: "file target".to_owned(),
        },
        ErrorCode::CapabilityMissing,
        "missing rollback prerequisite `storage.shadow_copy`: file target"
    );
    expect_error!(
        UndoError::IrreversibleStep {
            step_id: "s_1".to_owned(),
        },
        ErrorCode::ToolInvalidArgs,
        "step `s_1` is irreversible and has no rollback recipe"
    );
    expect_error!(
        UndoError::PostFingerprintAlreadyRecorded {
            anchor_id: anchor_id("a_1")?,
        },
        ErrorCode::ToolInvalidArgs,
        "post-step fingerprint is already recorded for anchor `a_1`"
    );
    expect_error!(
        UndoError::IncidentDeliveryFailed {
            incident_id: "i_1".to_owned(),
            error_code: ErrorCode::Fatal,
            reason: "reporter down".to_owned(),
        },
        ErrorCode::Fatal,
        "failed to deliver incident `i_1` as Fatal: reporter down"
    );
    Ok(())
}

#[test]
fn test_ids_recipe_bounds_and_tool_validation() -> Result<(), Box<dyn std::error::Error>> {
    for rejected in [
        AnchorId::parse("").is_err(),
        StepId::parse("\0").is_err(),
        TargetId::parse("").is_err(),
        IncidentId::parse("i\0").is_err(),
    ] {
        assert!(rejected);
    }
    let action = RollbackAction::UndoStack {
        times: UndoStepCount::new(1)?,
        until_fingerprint: fp('0')?,
    };
    assert!(RollbackRecipe::new(anchor_id("a_1")?, Vec::new()).is_err());
    assert!(RollbackRecipe::new(anchor_id("a_1")?, vec![action; 65]).is_err());
    assert_eq!(
        ToolName::parse("paint.canvas.draw")?.as_str(),
        "paint.canvas.draw"
    );
    for invalid in [
        "",
        "app.domain",
        "app.domain.action.extra",
        "App.domain.action",
    ] {
        assert!(ToolName::parse(invalid).is_err());
    }
    assert!(
        ToolInvocation::new(
            ToolName::parse("paint.canvas.draw")?,
            TargetId::parse("document_1")?,
            "",
        )
        .is_err()
    );
    assert!(
        ToolInvocation::new(
            ToolName::parse("paint.canvas.draw")?,
            TargetId::parse("document_1")?,
            "{\0}",
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn test_recipe_builders_cover_undo_and_content() -> Result<(), Box<dyn std::error::Error>> {
    let undo = anchor_l0(fp('2')?, Some(digest('1')?))?;
    let undo_recipe = build_rollback_recipe(&undo)?;
    assert!(matches!(
        undo_recipe.actions(),
        [RollbackAction::UndoStack { times, .. }] if times.get() == 3
    ));
    validate_rollback_recipe(&undo, &undo_recipe)?;
    assert!(build_fallback_recipe(&undo)?.is_some());
    let without_fallback = anchor_l0(fp('2')?, None)?;
    assert!(build_fallback_recipe(&without_fallback)?.is_none());

    let content = Anchor::new(
        anchor_id("a_content")?,
        StepId::parse("s_content")?,
        TargetId::parse("document_2")?,
        Reversibility::L1Snapshot,
        fp('0')?,
        AnchorKind::ContentSnapshot {
            content_digest: digest('1')?,
        },
    )?;
    let content_recipe = build_rollback_recipe(&content)?;
    assert!(matches!(
        content_recipe.actions(),
        [RollbackAction::RestoreContentSnapshot { .. }]
    ));
    validate_rollback_recipe(&content, &content_recipe)?;
    Ok(())
}

#[test]
fn test_recipe_builders_cover_shadow_compensating_and_evidence_only()
-> Result<(), Box<dyn std::error::Error>> {
    let shadow = anchor_shadow(fp('2')?)?;
    let shadow_recipe = build_rollback_recipe(&shadow)?;
    assert!(matches!(
        shadow_recipe.actions(),
        [RollbackAction::RestoreShadowCopy { .. }]
    ));
    assert_eq!(
        shadow.shadow_copy().map(|copy| copy.path.as_str()),
        Some("shadow/file_1.bak")
    );
    validate_rollback_recipe(&shadow, &shadow_recipe)?;

    let invocation = ToolInvocation::new(
        ToolName::parse("notepad.document.restore")?,
        TargetId::parse("document_3")?,
        r#"{"from":"shadow"}"#,
    )?;
    let compensating_recipe = RollbackRecipe::new(
        anchor_id("a_compensating")?,
        vec![RollbackAction::CompensatingAction { invocation }],
    )?;
    let compensating = Anchor::new(
        anchor_id("a_compensating")?,
        StepId::parse("s_compensating")?,
        TargetId::parse("document_3")?,
        Reversibility::L2Compensating,
        fp('0')?,
        AnchorKind::CompensatingAction {
            recipe: compensating_recipe.clone(),
        },
    )?;
    assert_eq!(build_rollback_recipe(&compensating)?, compensating_recipe);
    validate_rollback_recipe(&compensating, &compensating_recipe)?;

    let evidence_only = Anchor::new(
        anchor_id("a_l3")?,
        StepId::parse("s_l3")?,
        TargetId::parse("mail_1")?,
        Reversibility::L3Irreversible,
        fp('0')?,
        AnchorKind::EvidenceOnly,
    )?;
    assert!(build_rollback_recipe(&evidence_only).is_err());
    assert_eq!(evidence_only.strategy(), AnchorStrategy::EvidenceOnly);
    assert!(evidence_only.shadow_copy().is_none());
    assert!(evidence_only.fallback_snapshot().is_none());
    Ok(())
}

#[test]
fn test_recipe_validation_rejects_anchor_and_shape_mismatch()
-> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fp('2')?, None)?;
    let wrong_anchor = RollbackRecipe::new(
        anchor_id("a_other")?,
        vec![RollbackAction::UndoStack {
            times: UndoStepCount::new(3)?,
            until_fingerprint: fp('0')?,
        }],
    )?;
    assert!(validate_rollback_recipe(&anchor, &wrong_anchor).is_err());
    let wrong_shape = RollbackRecipe::new(
        anchor_id("a_1")?,
        vec![RollbackAction::UndoStack {
            times: UndoStepCount::new(2)?,
            until_fingerprint: fp('0')?,
        }],
    )?;
    assert!(validate_rollback_recipe(&anchor, &wrong_shape).is_err());
    Ok(())
}

#[test]
fn test_conflict_detection_and_blocking_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let no_post = Anchor::new(
        anchor_id("a_plain")?,
        StepId::parse("s_plain")?,
        TargetId::parse("document_4")?,
        Reversibility::L1Snapshot,
        fp('0')?,
        AnchorKind::ContentSnapshot {
            content_digest: digest('1')?,
        },
    )?;
    assert!(matches!(
        detect_conflict(&no_post, Some(no_post.pre_fingerprint())),
        RollbackConflict::EvidenceMissing { .. }
    ));
    let anchor = anchor_l0(fp('2')?, None)?;
    assert!(matches!(
        detect_conflict(&anchor, None),
        RollbackConflict::EvidenceMissing { .. }
    ));
    assert!(matches!(
        detect_conflict(&anchor, Some(&fp('2')?)),
        RollbackConflict::NoConflict
    ));
    assert!(matches!(
        detect_conflict(&anchor, Some(anchor.pre_fingerprint())),
        RollbackConflict::AlreadyAtAnchor { .. }
    ));
    let changed = detect_conflict(&anchor, Some(&fp('9')?));
    assert!(matches!(changed, RollbackConflict::UserChanged { .. }));
    assert!(blocks_rollback(&changed, ConflictResolution::FailClosed));
    assert!(!blocks_rollback(
        &changed,
        ConflictResolution::RestoreOverall
    ));
    assert!(!blocks_rollback(
        &RollbackConflict::NoConflict,
        ConflictResolution::FailClosed
    ));
    assert!(blocks_rollback(
        &RollbackConflict::EvidenceMissing {
            reason: "missing".to_owned(),
        },
        ConflictResolution::RestoreOverall,
    ));
    Ok(())
}

#[test]
fn test_l0_fallback_reports_mismatch_and_executor_failure() -> Result<(), Box<dyn std::error::Error>>
{
    let anchor = anchor_l0(fp('2')?, Some(digest('1')?))?;
    let observed = fp('2')?;
    let incident_id = IncidentId::parse("i_fallback")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(&observed),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let mismatch = Executor::new(vec![
        Ok(ActionOutcome::new(fp('3')?)),
        Ok(ActionOutcome::new(fp('4')?)),
    ]);
    let outcome = execute_rollback(&request, &mismatch)?;
    assert_eq!(
        outcome.incident().map(assistant_undo::IncidentReport::kind),
        Some(IncidentKind::FinalFingerprintMismatch)
    );
    assert_eq!(
        outcome
            .incident()
            .map(assistant_undo::IncidentReport::completed_actions),
        Some(2)
    );

    let failure = Executor::new(vec![
        Ok(ActionOutcome::new(fp('3')?)),
        Err(ExecutorFailure::with_evidence(
            ErrorCode::TargetUnresponsive,
            "fallback failed",
            "evidence:fallback",
        )),
    ]);
    let outcome = execute_rollback(&request, &failure)?;
    let report = outcome.incident().ok_or("expected incident")?;
    assert_eq!(report.kind(), IncidentKind::ExecutorFailed);
    assert_eq!(report.failed_action_index(), Some(1));
    assert_eq!(report.completed_actions(), 1);
    Ok(())
}

#[test]
fn test_incident_accessors_and_reporter_failure_are_observable()
-> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_shadow(fp('2')?)?;
    let incident_id = IncidentId::parse("i_shadow")?;
    let request = RollbackRequest::new(&anchor, None, ConflictResolution::FailClosed, &incident_id);
    let outcome = execute_rollback(&request, &Executor::default())?;
    let report = outcome.incident().ok_or("expected incident")?;
    assert_eq!(report.incident_id().as_str(), "i_shadow");
    assert_eq!(report.kind(), IncidentKind::ConflictEvidenceMissing);
    assert_eq!(report.severity(), IncidentSeverity::Critical);
    assert_eq!(report.error_code(), ErrorCode::VerifyFailed);
    assert_eq!(report.anchor_id().as_str(), "a_shadow");
    assert_eq!(report.step_id().as_str(), "s_shadow");
    assert_eq!(report.target_id().as_str(), "file_1");
    assert_eq!(report.expected_fingerprint(), anchor.pre_fingerprint());
    assert!(report.observed_fingerprint().is_none());
    assert!(report.failed_action_index().is_none());
    assert_eq!(report.completed_actions(), 0);
    assert!(report.summary().contains("incomplete"));
    assert!(
        report
            .evidence_refs()
            .iter()
            .any(|item| item == "shadow_copy:shadow/file_1.bak")
    );
    assert!(
        report
            .recovery_guidance()
            .iter()
            .any(|item| item.contains("shadow/file_1.bak"))
    );
    assert_eq!(
        publish_incident(&FailingReporter, report).map_err(|error| error.error_code()),
        Err(ErrorCode::Fatal)
    );
    Ok(())
}

#[test]
fn test_action_constructors_and_outcome_helpers() -> Result<(), Box<dyn std::error::Error>> {
    let plain = ActionOutcome::new(fp('1')?);
    assert_eq!(plain.evidence_ref, None);
    let evidenced = ActionOutcome::with_evidence(fp('2')?, "audit:rollback");
    assert_eq!(evidenced.evidence_ref.as_deref(), Some("audit:rollback"));
    let plain_failure = ExecutorFailure::new(ErrorCode::Transient, "retry");
    assert_eq!(plain_failure.evidence_ref, None);
    let evidenced_failure =
        ExecutorFailure::with_evidence(ErrorCode::Fatal, "stop", "evidence:fatal");
    assert_eq!(
        evidenced_failure.evidence_ref.as_deref(),
        Some("evidence:fatal")
    );
    assert_eq!(Reversibility::L0UndoStack.severity(), 0);
    assert_eq!(Reversibility::L3Irreversible.severity(), 3);
    assert!(Reversibility::L3Irreversible.is_irreversible());
    assert_eq!(Reversibility::worst(Vec::<Reversibility>::new()), None);
    assert!(
        RollbackOutcome::AlreadyAtAnchor {
            observed_fingerprint: fp('0')?,
        }
        .incident()
        .is_none()
    );
    Ok(())
}
