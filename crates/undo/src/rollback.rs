//! Rollback execution: conflict gate, action loop, final fingerprint verification, and incident
//! construction.
//!
//! Responsibility: enforce architecture v2 section 9's "verify after rollback" rule and stop at
//! the first failure.
//!
//! Boundary: execution is delegated to [`RollbackExecutor`]. This module owns orchestration and
//! safety decisions, not platform calls or storage writes.

use assistant_platform_api::{ErrorCode, Fingerprint};

use crate::anchor::{Anchor, AnchorKind};
use crate::conflict::{ConflictResolution, RollbackConflict, blocks_rollback, detect_conflict};
use crate::error::UndoResult;
use crate::id::IncidentId;
use crate::incident::{IncidentKind, IncidentReport, IncidentSeverity};
use crate::recipe::{RollbackAction, build_fallback_recipe, build_rollback_recipe};

/// Result of one successful rollback action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    /// Fingerprint observed immediately after the action.
    pub fingerprint: Fingerprint,
    /// Optional evidence reference preserved for the incident or audit trail.
    pub evidence_ref: Option<String>,
}

impl ActionOutcome {
    /// Creates an action outcome without an evidence reference.
    #[must_use]
    pub const fn new(fingerprint: Fingerprint) -> Self {
        Self {
            fingerprint,
            evidence_ref: None,
        }
    }

    /// Creates an action outcome with an evidence reference.
    #[must_use]
    pub fn with_evidence(fingerprint: Fingerprint, evidence_ref: impl Into<String>) -> Self {
        Self {
            fingerprint,
            evidence_ref: Some(evidence_ref.into()),
        }
    }
}

/// Failure returned by a rollback action executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorFailure {
    /// Existing protocol error category for the failed action.
    pub error_code: ErrorCode,
    /// Human-readable failure reason.
    pub reason: String,
    /// Optional evidence reference to preserve.
    pub evidence_ref: Option<String>,
}

impl ExecutorFailure {
    /// Creates an executor failure with no evidence reference.
    #[must_use]
    pub fn new(error_code: ErrorCode, reason: impl Into<String>) -> Self {
        Self {
            error_code,
            reason: reason.into(),
            evidence_ref: None,
        }
    }

    /// Creates an executor failure with an evidence reference.
    #[must_use]
    pub fn with_evidence(
        error_code: ErrorCode,
        reason: impl Into<String>,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self {
            error_code,
            reason: reason.into(),
            evidence_ref: Some(evidence_ref.into()),
        }
    }
}

/// Port that executes one rollback action.
pub trait RollbackExecutor {
    /// Executes one action and reports the fingerprint observed after it.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutorFailure`] with an existing protocol error category and evidence when the
    /// action cannot be completed. The engine stops immediately after this error.
    fn execute(&self, action: &RollbackAction) -> Result<ActionOutcome, ExecutorFailure>;
}

/// Inputs needed to execute one rollback.
#[derive(Debug, Clone, Copy)]
pub struct RollbackRequest<'a> {
    /// Anchor that defines the target state and rollback strategy.
    pub anchor: &'a Anchor,
    /// Current fingerprint observed by the caller, when available.
    pub observed_fingerprint: Option<&'a Fingerprint>,
    /// Conflict handling choice; defaults to fail-closed.
    pub conflict_resolution: ConflictResolution,
    /// Identifier assigned to an incident produced by this request.
    pub incident_id: &'a IncidentId,
}

impl<'a> RollbackRequest<'a> {
    /// Creates a rollback request.
    #[must_use]
    pub const fn new(
        anchor: &'a Anchor,
        observed_fingerprint: Option<&'a Fingerprint>,
        conflict_resolution: ConflictResolution,
        incident_id: &'a IncidentId,
    ) -> Self {
        Self {
            anchor,
            observed_fingerprint,
            conflict_resolution,
            incident_id,
        }
    }
}

/// Terminal result of one rollback request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackOutcome {
    /// The target was already at the anchor state, so no action was executed.
    AlreadyAtAnchor {
        /// Fingerprint observed at the anchor state.
        observed_fingerprint: Fingerprint,
    },
    /// Every action completed and the final fingerprint matched the anchor.
    Restored {
        /// Number of executed actions.
        completed_actions: usize,
        /// Final observed fingerprint.
        final_fingerprint: Fingerprint,
        /// Whether the optional L1 fallback was used after the primary recipe did not reach the
        /// anchor state.
        used_fallback: bool,
        /// Evidence references collected from successful actions.
        evidence_refs: Vec<String>,
    },
    /// Rollback was blocked or failed; subsequent automated actions must stop.
    Incident {
        /// Structured incident to surface and report.
        report: IncidentReport,
    },
}

impl RollbackOutcome {
    /// Returns whether the target is at the anchor state.
    #[must_use]
    pub const fn is_restored_or_already_present(&self) -> bool {
        matches!(self, Self::AlreadyAtAnchor { .. } | Self::Restored { .. })
    }

    /// Returns the incident report when rollback failed.
    #[must_use]
    pub const fn incident(&self) -> Option<&IncidentReport> {
        match self {
            Self::Incident { report } => Some(report),
            _ => None,
        }
    }
}

/// Executes an anchor's rollback recipe through the injected executor.
///
/// The engine first blocks conflicts, then executes actions in order. A successful action must
/// return a fingerprint. After all actions, the final fingerprint must equal the anchor's
/// pre-step fingerprint; otherwise the result is an incident.
///
/// # Errors
///
/// Returns [`UndoError::IrreversibleStep`] for an evidence-only anchor. Construction failures for
/// a recipe are also returned as [`UndoError`]; action failures become [`RollbackOutcome::Incident`]
/// so the caller cannot mistake them for a build error.
pub fn execute_rollback(
    request: &RollbackRequest<'_>,
    executor: &dyn RollbackExecutor,
) -> UndoResult<RollbackOutcome> {
    let anchor = request.anchor;
    let recipe = build_rollback_recipe(anchor)?;
    let fallback_recipe = build_fallback_recipe(anchor)?;
    let conflict = detect_conflict(anchor, request.observed_fingerprint);

    match &conflict {
        RollbackConflict::AlreadyAtAnchor {
            observed_fingerprint,
        } => {
            return Ok(RollbackOutcome::AlreadyAtAnchor {
                observed_fingerprint: observed_fingerprint.clone(),
            });
        }
        RollbackConflict::NoConflict => {}
        RollbackConflict::UserChanged { .. } | RollbackConflict::EvidenceMissing { .. } => {
            if blocks_rollback(&conflict, request.conflict_resolution) {
                return Ok(RollbackOutcome::Incident {
                    report: build_conflict_incident(request, &conflict, &[]),
                });
            }
        }
    }

    match run_actions(
        recipe.actions(),
        executor,
        request.observed_fingerprint.cloned(),
    ) {
        Ok(success) => Ok(finish_primary_success(
            request,
            executor,
            fallback_recipe.as_ref(),
            success,
        )),
        Err(failure) => {
            if let Some(fallback_recipe) = fallback_recipe.as_ref() {
                return Ok(finish_with_fallback(
                    request,
                    executor,
                    &failure,
                    fallback_recipe,
                    true,
                ));
            }
            Ok(RollbackOutcome::Incident {
                report: build_executor_incident(
                    request,
                    failure.action_index,
                    failure.completed_actions,
                    &failure.failure,
                    failure.last_fingerprint,
                    &failure.evidence_refs,
                ),
            })
        }
    }
}

struct ActionRunSuccess {
    completed_actions: usize,
    final_fingerprint: Option<Fingerprint>,
    evidence_refs: Vec<String>,
}

struct ActionRunFailure {
    action_index: usize,
    completed_actions: usize,
    failure: ExecutorFailure,
    last_fingerprint: Option<Fingerprint>,
    evidence_refs: Vec<String>,
}

fn run_actions(
    actions: &[RollbackAction],
    executor: &dyn RollbackExecutor,
    mut last_fingerprint: Option<Fingerprint>,
) -> Result<ActionRunSuccess, ActionRunFailure> {
    let mut completed_actions = 0;
    let mut evidence_refs = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        match executor.execute(action) {
            Ok(outcome) => {
                completed_actions += 1;
                last_fingerprint = Some(outcome.fingerprint);
                if let Some(evidence_ref) = outcome.evidence_ref {
                    evidence_refs.push(evidence_ref);
                }
            }
            Err(failure) => {
                if let Some(evidence_ref) = failure.evidence_ref.as_ref() {
                    evidence_refs.push(evidence_ref.clone());
                }
                return Err(ActionRunFailure {
                    action_index: index,
                    completed_actions,
                    failure,
                    last_fingerprint,
                    evidence_refs,
                });
            }
        }
    }
    Ok(ActionRunSuccess {
        completed_actions,
        final_fingerprint: last_fingerprint,
        evidence_refs,
    })
}

fn finish_primary_success(
    request: &RollbackRequest<'_>,
    executor: &dyn RollbackExecutor,
    fallback_recipe: Option<&crate::recipe::RollbackRecipe>,
    primary: ActionRunSuccess,
) -> RollbackOutcome {
    if primary.final_fingerprint.as_ref() == Some(request.anchor.pre_fingerprint()) {
        let Some(final_fingerprint) = primary.final_fingerprint else {
            return RollbackOutcome::Incident {
                report: build_missing_final_fingerprint_incident(
                    request,
                    primary.completed_actions,
                    &primary.evidence_refs,
                ),
            };
        };
        return RollbackOutcome::Restored {
            completed_actions: primary.completed_actions,
            final_fingerprint,
            used_fallback: false,
            evidence_refs: primary.evidence_refs,
        };
    }
    if let Some(fallback_recipe) = fallback_recipe {
        let failure = ActionRunFailure {
            action_index: primary.completed_actions,
            completed_actions: primary.completed_actions,
            failure: ExecutorFailure::new(
                ErrorCode::VerifyFailed,
                "primary rollback recipe did not reach the anchor fingerprint",
            ),
            last_fingerprint: primary.final_fingerprint,
            evidence_refs: primary.evidence_refs,
        };
        return finish_with_fallback(request, executor, &failure, fallback_recipe, true);
    }
    let Some(final_fingerprint) = primary.final_fingerprint else {
        return RollbackOutcome::Incident {
            report: build_missing_final_fingerprint_incident(
                request,
                primary.completed_actions,
                &primary.evidence_refs,
            ),
        };
    };
    RollbackOutcome::Incident {
        report: build_final_mismatch_incident(
            request,
            primary.completed_actions,
            final_fingerprint,
            &primary.evidence_refs,
        ),
    }
}

fn finish_with_fallback(
    request: &RollbackRequest<'_>,
    executor: &dyn RollbackExecutor,
    primary: &ActionRunFailure,
    fallback_recipe: &crate::recipe::RollbackRecipe,
    used_fallback: bool,
) -> RollbackOutcome {
    let mut evidence_refs = primary.evidence_refs.clone();
    match run_actions(
        fallback_recipe.actions(),
        executor,
        primary.last_fingerprint.clone(),
    ) {
        Ok(success) => {
            evidence_refs.extend(success.evidence_refs);
            if success.final_fingerprint.as_ref() == Some(request.anchor.pre_fingerprint()) {
                let Some(final_fingerprint) = success.final_fingerprint else {
                    return RollbackOutcome::Incident {
                        report: build_missing_final_fingerprint_incident(
                            request,
                            primary.completed_actions + success.completed_actions,
                            &evidence_refs,
                        ),
                    };
                };
                RollbackOutcome::Restored {
                    completed_actions: primary.completed_actions + success.completed_actions,
                    final_fingerprint,
                    used_fallback,
                    evidence_refs,
                }
            } else {
                let Some(final_fingerprint) = success.final_fingerprint else {
                    return RollbackOutcome::Incident {
                        report: build_missing_final_fingerprint_incident(
                            request,
                            primary.completed_actions + success.completed_actions,
                            &evidence_refs,
                        ),
                    };
                };
                RollbackOutcome::Incident {
                    report: build_final_mismatch_incident(
                        request,
                        primary.completed_actions + success.completed_actions,
                        final_fingerprint,
                        &evidence_refs,
                    ),
                }
            }
        }
        Err(fallback_failure) => {
            evidence_refs.extend(fallback_failure.evidence_refs);
            RollbackOutcome::Incident {
                report: build_executor_incident(
                    request,
                    primary.completed_actions + fallback_failure.action_index,
                    primary.completed_actions + fallback_failure.completed_actions,
                    &fallback_failure.failure,
                    fallback_failure.last_fingerprint,
                    &evidence_refs,
                ),
            }
        }
    }
}

fn build_conflict_incident(
    request: &RollbackRequest<'_>,
    conflict: &RollbackConflict,
    evidence_refs: &[String],
) -> IncidentReport {
    let (kind, summary, observed_fingerprint, guidance) = match conflict {
        RollbackConflict::UserChanged { observed_fingerprint, .. } => (
            IncidentKind::ConflictSuspected,
            "current state changed after the agent step; rollback stopped by default",
            Some(observed_fingerprint.clone()),
            vec![
                "Stop automated actions and preserve the current state.".to_owned(),
                "Show the expected post-step state and the observed current state to the user."
                    .to_owned(),
                "Require an explicit choice between precise reverse diff, whole-anchor restore, or cancel."
                    .to_owned(),
            ],
        ),
        RollbackConflict::EvidenceMissing { reason } => (
            IncidentKind::ConflictEvidenceMissing,
            "rollback evidence is incomplete; rollback stopped",
            None,
            vec![
                "Stop automated actions and collect a fresh fingerprint.".to_owned(),
                format!("Missing evidence: {reason}"),
                "Do not execute a rollback until a human verifies the current state.".to_owned(),
            ],
        ),
        RollbackConflict::AlreadyAtAnchor { .. } | RollbackConflict::NoConflict => (
            IncidentKind::ConflictEvidenceMissing,
            "internal conflict state was not executable",
            None,
            vec!["Stop automated actions and inspect the rollback request.".to_owned()],
        ),
    };
    IncidentReport {
        incident_id: request.incident_id.clone(),
        kind,
        severity: IncidentSeverity::Critical,
        error_code: ErrorCode::VerifyFailed,
        anchor_id: request.anchor.anchor_id().clone(),
        step_id: request.anchor.step_id().clone(),
        target_id: request.anchor.target_id().clone(),
        expected_fingerprint: request.anchor.pre_fingerprint().clone(),
        observed_fingerprint,
        failed_action_index: None,
        completed_actions: 0,
        summary: summary.to_owned(),
        evidence_refs: with_shadow_copy_evidence(request.anchor, evidence_refs),
        recovery_guidance: with_shadow_copy_guidance(request.anchor, guidance),
    }
}

fn build_executor_incident(
    request: &RollbackRequest<'_>,
    action_index: usize,
    completed_actions: usize,
    failure: &ExecutorFailure,
    last_fingerprint: Option<Fingerprint>,
    evidence_refs: &[String],
) -> IncidentReport {
    let mut guidance = vec![
        "Stop subsequent automated actions and preserve the failed action evidence.".to_owned(),
        format!("Failed action index: {action_index}; completed actions: {completed_actions}"),
        "Inspect the target and retry only after a human confirms it is safe.".to_owned(),
    ];
    guidance = with_shadow_copy_guidance(request.anchor, guidance);
    IncidentReport {
        incident_id: request.incident_id.clone(),
        kind: IncidentKind::ExecutorFailed,
        severity: IncidentSeverity::Critical,
        error_code: failure.error_code,
        anchor_id: request.anchor.anchor_id().clone(),
        step_id: request.anchor.step_id().clone(),
        target_id: request.anchor.target_id().clone(),
        expected_fingerprint: request.anchor.pre_fingerprint().clone(),
        observed_fingerprint: last_fingerprint,
        failed_action_index: Some(action_index),
        completed_actions,
        summary: format!("rollback action failed: {}", failure.reason),
        evidence_refs: with_shadow_copy_evidence(request.anchor, evidence_refs),
        recovery_guidance: guidance,
    }
}

fn build_missing_final_fingerprint_incident(
    request: &RollbackRequest<'_>,
    completed_actions: usize,
    evidence_refs: &[String],
) -> IncidentReport {
    let guidance = with_shadow_copy_guidance(
        request.anchor,
        vec![
            "Stop automated actions and collect a fresh fingerprint.".to_owned(),
            "Do not report rollback success without final fingerprint verification.".to_owned(),
        ],
    );
    IncidentReport {
        incident_id: request.incident_id.clone(),
        kind: IncidentKind::ConflictEvidenceMissing,
        severity: IncidentSeverity::Critical,
        error_code: ErrorCode::VerifyFailed,
        anchor_id: request.anchor.anchor_id().clone(),
        step_id: request.anchor.step_id().clone(),
        target_id: request.anchor.target_id().clone(),
        expected_fingerprint: request.anchor.pre_fingerprint().clone(),
        observed_fingerprint: None,
        failed_action_index: None,
        completed_actions,
        summary: "rollback finished without a final fingerprint".to_owned(),
        evidence_refs: with_shadow_copy_evidence(request.anchor, evidence_refs),
        recovery_guidance: guidance,
    }
}

fn build_final_mismatch_incident(
    request: &RollbackRequest<'_>,
    completed_actions: usize,
    final_fingerprint: Fingerprint,
    evidence_refs: &[String],
) -> IncidentReport {
    let guidance = with_shadow_copy_guidance(
        request.anchor,
        vec![
            "Stop automated actions and preserve the final observed state.".to_owned(),
            "Compare the expected anchor state with the observed state before retrying.".to_owned(),
            "Use the original shadow copy or snapshot as the manual recovery source.".to_owned(),
        ],
    );
    IncidentReport {
        incident_id: request.incident_id.clone(),
        kind: IncidentKind::FinalFingerprintMismatch,
        severity: IncidentSeverity::Critical,
        error_code: ErrorCode::VerifyFailed,
        anchor_id: request.anchor.anchor_id().clone(),
        step_id: request.anchor.step_id().clone(),
        target_id: request.anchor.target_id().clone(),
        expected_fingerprint: request.anchor.pre_fingerprint().clone(),
        observed_fingerprint: Some(final_fingerprint),
        failed_action_index: None,
        completed_actions,
        summary: "rollback completed but did not reach the anchor fingerprint".to_owned(),
        evidence_refs: with_shadow_copy_evidence(request.anchor, evidence_refs),
        recovery_guidance: guidance,
    }
}

fn with_shadow_copy_evidence(anchor: &Anchor, evidence_refs: &[String]) -> Vec<String> {
    let mut result = evidence_refs.to_vec();
    if let AnchorKind::ShadowCopy { shadow_copy } = anchor.kind() {
        result.push(format!("shadow_copy:{}", shadow_copy.path.as_str()));
    }
    result
}

fn with_shadow_copy_guidance(anchor: &Anchor, mut guidance: Vec<String>) -> Vec<String> {
    if let AnchorKind::ShadowCopy { shadow_copy } = anchor.kind() {
        guidance.push(format!(
            "Manual recovery source: {}",
            shadow_copy.path.as_str()
        ));
    }
    guidance
}
