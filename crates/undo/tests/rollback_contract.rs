//! Contract tests for rollback recipes, conflict handling, final verification, and incidents.

use std::cell::RefCell;
use std::collections::VecDeque;

use assistant_platform_api::{ErrorCode, Fingerprint};
use assistant_undo::{
    ActionOutcome, Anchor, AnchorId, AnchorKind, ConflictResolution, ContentDigest,
    ExecutorFailure, IncidentId, IncidentKind, IncidentReport, IncidentReporter, Reversibility,
    RollbackAction, RollbackExecutor, RollbackOutcome, RollbackRecipe, RollbackRequest, ShadowCopy,
    ShadowCopyPath, StepId, TargetId, ToolInvocation, ToolName, UndoBudget, UndoStepCount,
    UndoValidity, build_rollback_recipe, execute_rollback, publish_incident,
};

fn fingerprint(hex: char) -> Result<Fingerprint, Box<dyn std::error::Error>> {
    let digest = hex.to_string().repeat(64);
    Ok(Fingerprint::parse(format!("sha256:{digest}"))?)
}

fn digest(hex: char) -> Result<ContentDigest, Box<dyn std::error::Error>> {
    Ok(ContentDigest::parse(hex.to_string().repeat(64))?)
}

fn anchor_l0(post: Fingerprint) -> Result<Anchor, Box<dyn std::error::Error>> {
    anchor_l0_with_fallback(post, None)
}

fn anchor_l0_with_fallback(
    post: Fingerprint,
    fallback_snapshot: Option<ContentDigest>,
) -> Result<Anchor, Box<dyn std::error::Error>> {
    let mut anchor = Anchor::new(
        AnchorId::parse("a_l0")?,
        StepId::parse("s_l0")?,
        TargetId::parse("document_1")?,
        Reversibility::L0UndoStack,
        fingerprint('0')?,
        AnchorKind::UndoStack {
            budget: UndoBudget::new(UndoStepCount::new(3)?, UndoValidity::DocumentClose),
            fallback_snapshot,
        },
    )?;
    anchor.record_post_fingerprint(post)?;
    Ok(anchor)
}

fn anchor_l1(post: Fingerprint) -> Result<Anchor, Box<dyn std::error::Error>> {
    let mut anchor = Anchor::new(
        AnchorId::parse("a_l1")?,
        StepId::parse("s_l1")?,
        TargetId::parse("file_1")?,
        Reversibility::L1Snapshot,
        fingerprint('0')?,
        AnchorKind::ShadowCopy {
            shadow_copy: ShadowCopy::new(ShadowCopyPath::parse("shadow/file_1.bak")?, digest('1')?),
        },
    )?;
    anchor.record_post_fingerprint(post)?;
    Ok(anchor)
}

fn anchor_l2(post: Fingerprint) -> Result<Anchor, Box<dyn std::error::Error>> {
    let invocation = ToolInvocation::new(
        ToolName::parse("notepad.document.restore")?,
        TargetId::parse("document_1")?,
        r#"{"from":"shadow"}"#,
    )?;
    let recipe = RollbackRecipe::new(
        AnchorId::parse("a_l2")?,
        vec![
            RollbackAction::CompensatingAction {
                invocation: invocation.clone(),
            },
            RollbackAction::CompensatingAction { invocation },
        ],
    )?;
    let mut anchor = Anchor::new(
        AnchorId::parse("a_l2")?,
        StepId::parse("s_l2")?,
        TargetId::parse("document_1")?,
        Reversibility::L2Compensating,
        fingerprint('0')?,
        AnchorKind::CompensatingAction { recipe },
    )?;
    anchor.record_post_fingerprint(post)?;
    Ok(anchor)
}

#[derive(Default)]
struct ScriptedExecutor {
    actions: RefCell<Vec<RollbackAction>>,
    outcomes: RefCell<VecDeque<Result<ActionOutcome, ExecutorFailure>>>,
}

impl ScriptedExecutor {
    fn new(outcomes: Vec<Result<ActionOutcome, ExecutorFailure>>) -> Self {
        Self {
            actions: RefCell::new(Vec::new()),
            outcomes: RefCell::new(VecDeque::from(outcomes)),
        }
    }

    fn recorded_actions(&self) -> Vec<RollbackAction> {
        self.actions.borrow().clone()
    }
}

impl RollbackExecutor for ScriptedExecutor {
    fn execute(&self, action: &RollbackAction) -> Result<ActionOutcome, ExecutorFailure> {
        self.actions.borrow_mut().push(action.clone());
        self.outcomes.borrow_mut().pop_front().unwrap_or_else(|| {
            Err(ExecutorFailure::new(
                ErrorCode::Transient,
                "scripted executor has no outcome",
            ))
        })
    }
}

#[derive(Default)]
struct RecordingReporter {
    reports: RefCell<Vec<IncidentReport>>,
    failure: Option<ExecutorFailure>,
}

impl IncidentReporter for RecordingReporter {
    fn report(&self, report: &IncidentReport) -> Result<(), assistant_undo::ReportFailure> {
        self.reports.borrow_mut().push(report.clone());
        if let Some(failure) = &self.failure {
            return Err(assistant_undo::ReportFailure::new(
                failure.error_code,
                failure.reason.clone(),
            ));
        }
        Ok(())
    }
}

#[test]
fn test_build_recipe_l0_uses_recorded_step_count() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let recipe = build_rollback_recipe(&anchor)?;
    assert_eq!(recipe.actions().len(), 1);
    assert!(matches!(
        recipe.actions().first(),
        Some(RollbackAction::UndoStack { times, .. }) if times.get() == 3
    ));
    Ok(())
}

#[test]
fn test_build_recipe_l3_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = Anchor::new(
        AnchorId::parse("a_l3")?,
        StepId::parse("s_l3")?,
        TargetId::parse("mail_1")?,
        Reversibility::L3Irreversible,
        fingerprint('0')?,
        AnchorKind::EvidenceOnly,
    )?;
    assert_eq!(
        build_rollback_recipe(&anchor).map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    Ok(())
}

#[test]
fn test_execute_l0_recipe_returns_restored_and_records_action()
-> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::new(vec![Ok(ActionOutcome::with_evidence(
        fingerprint('0')?,
        "audit:undo",
    ))]);
    let incident_id = IncidentId::parse("i_l0")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.post_fingerprint().ok_or("post missing")?),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(outcome.is_restored_or_already_present());
    assert_eq!(executor.recorded_actions().len(), 1);
    Ok(())
}

#[test]
fn test_l0_fallback_snapshot_is_used_after_undo_failure() -> Result<(), Box<dyn std::error::Error>>
{
    let anchor = anchor_l0_with_fallback(fingerprint('2')?, Some(digest('1')?))?;
    let executor = ScriptedExecutor::new(vec![
        Err(ExecutorFailure::new(
            ErrorCode::Transient,
            "undo stack was cleared",
        )),
        Ok(ActionOutcome::new(fingerprint('0')?)),
    ]);
    let incident_id = IncidentId::parse("i_l0_fallback")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.post_fingerprint().ok_or("post missing")?),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(matches!(
        outcome,
        RollbackOutcome::Restored {
            completed_actions: 1,
            used_fallback: true,
            ..
        }
    ));
    assert!(matches!(
        executor.recorded_actions().get(1),
        Some(RollbackAction::RestoreContentSnapshot { .. })
    ));
    Ok(())
}

#[test]
fn test_execute_l1_shadow_recipe_restores_shadow_copy() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l1(fingerprint('2')?)?;
    let executor = ScriptedExecutor::new(vec![Ok(ActionOutcome::new(fingerprint('0')?))]);
    let incident_id = IncidentId::parse("i_l1")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.post_fingerprint().ok_or("post missing")?),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(matches!(
        outcome,
        RollbackOutcome::Restored {
            completed_actions: 1,
            ..
        }
    ));
    assert!(matches!(
        executor.recorded_actions().first(),
        Some(RollbackAction::RestoreShadowCopy { .. })
    ));
    Ok(())
}

#[test]
fn test_execute_l2_recipe_preserves_action_order() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l2(fingerprint('2')?)?;
    let executor = ScriptedExecutor::new(vec![
        Ok(ActionOutcome::new(fingerprint('3')?)),
        Ok(ActionOutcome::new(fingerprint('0')?)),
    ]);
    let incident_id = IncidentId::parse("i_l2")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.post_fingerprint().ok_or("post missing")?),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(matches!(
        outcome,
        RollbackOutcome::Restored {
            completed_actions: 2,
            ..
        }
    ));
    assert_eq!(executor.recorded_actions().len(), 2);
    Ok(())
}

#[test]
fn test_user_change_blocks_execution_by_default() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::default();
    let incident_id = IncidentId::parse("i_conflict")?;
    let changed = fingerprint('9')?;
    let request = RollbackRequest::new(
        &anchor,
        Some(&changed),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(matches!(
        outcome.incident().map(IncidentReport::kind),
        Some(IncidentKind::ConflictSuspected)
    ));
    assert!(executor.recorded_actions().is_empty());
    Ok(())
}

#[test]
fn test_restore_overall_allows_user_change_after_explicit_choice()
-> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::new(vec![Ok(ActionOutcome::new(fingerprint('0')?))]);
    let incident_id = IncidentId::parse("i_force")?;
    let changed = fingerprint('9')?;
    let request = RollbackRequest::new(
        &anchor,
        Some(&changed),
        ConflictResolution::RestoreOverall,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(outcome.is_restored_or_already_present());
    Ok(())
}

#[test]
fn test_missing_evidence_blocks_even_restore_overall() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::default();
    let incident_id = IncidentId::parse("i_missing")?;
    let request = RollbackRequest::new(
        &anchor,
        None,
        ConflictResolution::RestoreOverall,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert_eq!(
        outcome.incident().map(IncidentReport::kind),
        Some(IncidentKind::ConflictEvidenceMissing)
    );
    assert!(executor.recorded_actions().is_empty());
    Ok(())
}

#[test]
fn test_final_fingerprint_mismatch_becomes_incident() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::new(vec![Ok(ActionOutcome::new(fingerprint('7')?))]);
    let incident_id = IncidentId::parse("i_mismatch")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.post_fingerprint().ok_or("post missing")?),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert_eq!(
        outcome.incident().map(IncidentReport::kind),
        Some(IncidentKind::FinalFingerprintMismatch)
    );
    assert_eq!(
        outcome.incident().map(IncidentReport::completed_actions),
        Some(1)
    );
    Ok(())
}

#[test]
fn test_executor_failure_stops_and_preserves_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l2(fingerprint('2')?)?;
    let executor = ScriptedExecutor::new(vec![Err(ExecutorFailure::with_evidence(
        ErrorCode::TargetUnresponsive,
        "target did not answer",
        "evidence:timeout",
    ))]);
    let incident_id = IncidentId::parse("i_executor")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.post_fingerprint().ok_or("post missing")?),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    let report = outcome.incident().ok_or("expected incident")?;
    assert_eq!(report.kind(), IncidentKind::ExecutorFailed);
    assert_eq!(report.error_code(), ErrorCode::TargetUnresponsive);
    assert_eq!(report.failed_action_index(), Some(0));
    assert_eq!(report.completed_actions(), 0);
    assert!(
        report
            .evidence_refs()
            .iter()
            .any(|item| item == "evidence:timeout")
    );
    Ok(())
}

#[test]
fn test_already_at_anchor_is_idempotent_without_actions() -> Result<(), Box<dyn std::error::Error>>
{
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::default();
    let incident_id = IncidentId::parse("i_idempotent")?;
    let request = RollbackRequest::new(
        &anchor,
        Some(anchor.pre_fingerprint()),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    assert!(matches!(outcome, RollbackOutcome::AlreadyAtAnchor { .. }));
    assert!(executor.recorded_actions().is_empty());
    Ok(())
}

#[test]
fn test_publish_incident_reports_success_and_failure() -> Result<(), Box<dyn std::error::Error>> {
    let anchor = anchor_l0(fingerprint('2')?)?;
    let executor = ScriptedExecutor::default();
    let incident_id = IncidentId::parse("i_report")?;
    let changed = fingerprint('9')?;
    let request = RollbackRequest::new(
        &anchor,
        Some(&changed),
        ConflictResolution::FailClosed,
        &incident_id,
    );
    let outcome = execute_rollback(&request, &executor)?;
    let report = outcome.incident().ok_or("expected incident")?;

    let reporter = RecordingReporter::default();
    publish_incident(&reporter, report)?;
    assert_eq!(reporter.reports.borrow().len(), 1);

    let failing_reporter = RecordingReporter {
        reports: RefCell::new(Vec::new()),
        failure: Some(ExecutorFailure::new(ErrorCode::Fatal, "audit unavailable")),
    };
    assert_eq!(
        publish_incident(&failing_reporter, report).map_err(|error| error.error_code()),
        Err(ErrorCode::Fatal)
    );
    Ok(())
}
