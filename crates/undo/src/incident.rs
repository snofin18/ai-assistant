//! Structured incident reports for rollback failures and their delivery port.
//!
//! Responsibility: preserve expected versus observed state, failed action context, evidence, and
//! human recovery guidance. The report is the value returned to the Host; delivery is a separate
//! injected reporter.
//!
//! Boundary: no audit storage, notification transport, retry, or clock access.

use assistant_platform_api::{ErrorCode, Fingerprint};

use crate::error::{UndoError, UndoResult};
use crate::id::{AnchorId, IncidentId, StepId, TargetId};

/// Why an incident was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentKind {
    /// User changes were detected while fail-closed rollback was requested.
    ConflictSuspected,
    /// Required fingerprint evidence was missing.
    ConflictEvidenceMissing,
    /// A rollback action failed.
    ExecutorFailed,
    /// Actions completed, but the final fingerprint did not reach the anchor.
    FinalFingerprintMismatch,
}

/// Severity of a rollback incident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentSeverity {
    /// The failure can be explained and no immediate data loss is known.
    High,
    /// State is unknown or potentially damaged; automation must stop and a human must review.
    Critical,
}

/// A structured rollback incident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentReport {
    pub(crate) incident_id: IncidentId,
    pub(crate) kind: IncidentKind,
    pub(crate) severity: IncidentSeverity,
    pub(crate) error_code: ErrorCode,
    pub(crate) anchor_id: AnchorId,
    pub(crate) step_id: StepId,
    pub(crate) target_id: TargetId,
    pub(crate) expected_fingerprint: Fingerprint,
    pub(crate) observed_fingerprint: Option<Fingerprint>,
    pub(crate) failed_action_index: Option<usize>,
    pub(crate) completed_actions: usize,
    pub(crate) summary: String,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) recovery_guidance: Vec<String>,
}

impl IncidentReport {
    /// Returns the incident identifier.
    #[must_use]
    pub const fn incident_id(&self) -> &IncidentId {
        &self.incident_id
    }

    /// Returns the incident kind.
    #[must_use]
    pub const fn kind(&self) -> IncidentKind {
        self.kind
    }

    /// Returns the incident severity.
    #[must_use]
    pub const fn severity(&self) -> IncidentSeverity {
        self.severity
    }

    /// Returns the protocol error category to record with this incident.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        self.error_code
    }

    /// Returns the affected anchor.
    #[must_use]
    pub const fn anchor_id(&self) -> &AnchorId {
        &self.anchor_id
    }

    /// Returns the affected step.
    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    /// Returns the affected target.
    #[must_use]
    pub const fn target_id(&self) -> &TargetId {
        &self.target_id
    }

    /// Returns the fingerprint rollback attempted to reach.
    #[must_use]
    pub const fn expected_fingerprint(&self) -> &Fingerprint {
        &self.expected_fingerprint
    }

    /// Returns the last observed fingerprint, when one exists.
    #[must_use]
    pub const fn observed_fingerprint(&self) -> Option<&Fingerprint> {
        self.observed_fingerprint.as_ref()
    }

    /// Returns the failed action index, when an action failed.
    #[must_use]
    pub const fn failed_action_index(&self) -> Option<usize> {
        self.failed_action_index
    }

    /// Returns the number of actions that completed before the incident.
    #[must_use]
    pub const fn completed_actions(&self) -> usize {
        self.completed_actions
    }

    /// Returns the human-readable incident summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Returns evidence references to preserve with the incident.
    #[must_use]
    pub fn evidence_refs(&self) -> &[String] {
        &self.evidence_refs
    }

    /// Returns human recovery guidance.
    #[must_use]
    pub fn recovery_guidance(&self) -> &[String] {
        &self.recovery_guidance
    }
}

/// Failure returned by a reporter after it attempts to deliver an incident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFailure {
    /// Error category returned by the reporter.
    pub error_code: ErrorCode,
    /// Human-readable failure reason.
    pub reason: String,
}

impl ReportFailure {
    /// Creates a structured reporter failure.
    #[must_use]
    pub fn new(error_code: ErrorCode, reason: impl Into<String>) -> Self {
        Self {
            error_code,
            reason: reason.into(),
        }
    }
}

/// Injected port that delivers incidents to the Host, audit channel, or UI.
pub trait IncidentReporter {
    /// Delivers one incident.
    ///
    /// # Errors
    ///
    /// Returns [`ReportFailure`] when delivery fails. Silent success is not an accepted outcome.
    fn report(&self, report: &IncidentReport) -> Result<(), ReportFailure>;
}

/// Publishes an incident through an injected reporter.
///
/// # Errors
///
/// Returns [`UndoError::IncidentDeliveryFailed`] when the reporter rejects or cannot deliver the
/// report. The caller must not treat the incident as handled in that case.
pub fn publish_incident(
    reporter: &dyn IncidentReporter,
    report: &IncidentReport,
) -> UndoResult<()> {
    reporter
        .report(report)
        .map_err(|failure| UndoError::IncidentDeliveryFailed {
            incident_id: report.incident_id.as_str().to_owned(),
            error_code: failure.error_code,
            reason: failure.reason,
        })
}
