//! Physical rollback snapshots and the `crates/undo` executor for Notepad.
//!
//! Responsibilities:
//! - capture the editor's canonical text and the target file's raw bytes before a write;
//! - keep the captured state in a process-local registry keyed by task id;
//! - execute `crates/undo` rollback recipes against the injected notepad context;
//! - re-read the editor and the file after a rollback so success is verified, not assumed.
//!
//! Boundaries:
//! - no policy decision, approval decision, persistence, or audit write;
//! - no `crates/undo` contract change: this module only implements `RollbackExecutor`;
//! - no handle or element leaves the process.
//!
//! Invariants:
//! 1. a snapshot is captured before any write step and fails closed when the editor text,
//!    the target path, or the file bytes cannot be observed;
//! 2. restore writes the captured bytes and re-reads the same digest before reporting success;
//! 3. a rollback succeeds only when the editor text equals the anchor text.
//!
//! Related documents: ADR-0062, ADR-0023, ADR-0043,
//! `docs/audits/stage-1a-runtime-validation-*.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_platform_api::{Fingerprint, KeyModifier, UiAutomationProvider, WindowProvider};
use assistant_protocol::serde_json::{Value, json};
use assistant_storage::BlobId;
use assistant_undo::{
    ActionOutcome, Anchor, AnchorId, AnchorKind, ConflictResolution, ContentDigest,
    ExecutorFailure, IncidentId, Reversibility, RollbackAction, RollbackExecutor, RollbackOutcome,
    RollbackRequest, StepId, TargetId, UndoBudget, UndoStepCount, UndoValidity, execute_rollback,
};

use crate::notepad_handlers::{NotepadHandlerContext, normalize_line_endings};

/// Physical state captured before a write plus the `crates/undo` anchor describing it.
#[derive(Clone)]
pub struct CapturedRollbackAnchor {
    anchor: Anchor,
    canonical_text: String,
    target_path: Option<PathBuf>,
    file_bytes: Option<Vec<u8>>,
}

impl CapturedRollbackAnchor {
    /// Returns the captured canonical editor text.
    #[must_use]
    pub(crate) fn canonical_text(&self) -> &str {
        &self.canonical_text
    }

    /// Returns the captured target path, when the task declared one.
    #[must_use]
    pub(crate) fn target_path(&self) -> Option<&Path> {
        self.target_path.as_deref()
    }
}

/// Captures physical snapshots and runs a full rollback through `crates/undo`.
///
/// The registry is intentionally process-local: it is a single run's working state, not a
/// second conversation truth. A missing entry is an explicit failure rather than a no-op.
pub struct NotepadRollback<P> {
    context: Arc<NotepadHandlerContext<P>>,
}

impl<P> NotepadRollback<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    /// Creates the rollback helper around the assembled notepad context.
    #[must_use]
    pub(crate) const fn new(context: Arc<NotepadHandlerContext<P>>) -> Self {
        Self { context }
    }

    /// Captures the pre-write anchor described by ADR-0062 D1/D2.
    ///
    /// # Errors
    ///
    /// Returns a readable reason when the editor text, the target path, or the file bytes
    /// cannot be observed, or when the anchor cannot be constructed.
    pub(crate) fn capture(
        &self,
        task_id: &str,
        step_id: &str,
        sequence: u32,
        target_path: Option<&Path>,
    ) -> Result<CapturedRollbackAnchor, String> {
        let canonical_text = self.read_canonical_text()?;
        let file_bytes = match target_path {
            Some(path) => Some(std::fs::read(path).map_err(|error| {
                format!(
                    "rollback anchor: cannot read target file `{}`: {error}",
                    path.display()
                )
            })?),
            // A task without a file path (for example the fake-platform contract test) can still
            // anchor the editor text. The L1 file restore is skipped and reported explicitly by
            // `file_matches_snapshot` rather than invented.
            None => None,
        };
        let anchor = self.build_anchor(task_id, step_id, sequence, &canonical_text)?;
        Ok(CapturedRollbackAnchor {
            anchor,
            canonical_text,
            target_path: target_path.map(Path::to_path_buf),
            file_bytes,
        })
    }

    fn build_anchor(
        &self,
        task_id: &str,
        step_id: &str,
        sequence: u32,
        canonical_text: &str,
    ) -> Result<Anchor, String> {
        let anchor_id = AnchorId::parse(format!("{task_id}:{step_id}:{sequence}"))
            .map_err(|error| format!("rollback anchor: invalid anchor id: {error}"))?;
        let step = StepId::parse(step_id)
            .map_err(|error| format!("rollback anchor: invalid step id: {error}"))?;
        let target = TargetId::parse(self.context.app_id.clone())
            .map_err(|error| format!("rollback anchor: invalid target id: {error}"))?;
        // The L0 undo executor reports the editor-content fingerprint, so the anchor uses the
        // same content digest as its pre/post fingerprint. The whole-window fingerprint stays
        // with `prepare_anchors` for the runtime state check; ADR-0062 D9 keeps both shapes.
        let fingerprint = fingerprint_for(canonical_text)
            .map_err(|error| format!("rollback anchor: invalid content fingerprint: {error}"))?;
        let content_digest = ContentDigest::parse(
            BlobId::of_content(canonical_text.as_bytes())
                .as_str()
                .to_owned(),
        )
        .map_err(|error| format!("rollback anchor: invalid content digest: {error}"))?;
        let undo_budget = UndoBudget::new(
            UndoStepCount::new(1)
                .map_err(|error| format!("rollback anchor: invalid undo budget: {error}"))?,
            UndoValidity::SessionEnd,
        );
        let mut anchor = Anchor::new(
            anchor_id,
            step,
            target,
            Reversibility::L0UndoStack,
            fingerprint,
            AnchorKind::UndoStack {
                budget: undo_budget,
                fallback_snapshot: Some(content_digest),
            },
        )
        .map_err(|error| format!("rollback anchor: cannot build the anchor: {error}"))?;
        // At capture time the target is already back at the anchor, so the post-step fingerprint
        // equals the pre-step fingerprint; conflict detection sees "already at anchor" rather
        // than a false user change.
        anchor
            .record_post_fingerprint(anchor.pre_fingerprint().clone())
            .map_err(|error| {
                format!("rollback anchor: cannot record the post fingerprint: {error}")
            })?;
        Ok(anchor)
    }

    /// Executes a full rollback and reports whether the anchor state was re-observed.
    ///
    /// `restore_file = true` also writes the captured target bytes back, which is the L1
    /// recovery path after a save. `restore_file = false` restores the editor only.
    ///
    /// # Errors
    ///
    /// Returns a readable reason for missing evidence or for a rollback that reached an incident.
    pub(crate) fn execute(
        &self,
        task_id: &str,
        step_id: &str,
        captured: &CapturedRollbackAnchor,
        restore_file: bool,
    ) -> Result<Value, String> {
        let executor = NotepadRollbackExecutor {
            context: Arc::clone(&self.context),
            canonical_text: captured.canonical_text.clone(),
            target_path: captured.target_path.clone(),
            file_bytes: captured.file_bytes.clone(),
            restore_file,
        };
        let observed = self.observe_fingerprint();
        let incident_id = IncidentId::parse(format!("{task_id}:{step_id}:incident"))
            .map_err(|error| format!("rollback: invalid incident id: {error}"))?;
        // L1 recovery is an explicit whole-anchor restore requested by the caller, so ADR-0062
        // D7 allows it to overwrite a later change. The editor-only path stays fail-closed.
        let conflict_resolution = if restore_file {
            ConflictResolution::RestoreOverall
        } else {
            ConflictResolution::FailClosed
        };
        let request = RollbackRequest::new(
            &captured.anchor,
            observed.as_ref(),
            conflict_resolution,
            &incident_id,
        );
        let outcome = execute_rollback(&request, &executor)
            .map_err(|error| format!("rollback: cannot execute the recipe: {error}"))?;
        let description = describe_outcome(&outcome)?;
        // The primary L0 recipe only contains the undo action, so the L1 file bytes are written
        // here after the undo crate has confirmed the editor reaches the anchor text, then the
        // observation re-read below verifies both. This keeps the undo crate's recipe/outcome
        // semantics unchanged while honouring ADR-0062 D5/D8.
        if restore_file {
            Self::restore_captured_file(captured)?;
        }
        let file_matches_snapshot = self.verify_after_rollback(captured, restore_file)?;
        Ok(json!({
            "status": description.status,
            "used_fallback": description.used_fallback,
            "final_fingerprint": description.final_fingerprint,
            "editor_matches_anchor": true,
            "file_matches_snapshot": file_matches_snapshot,
            "evidence": description.evidence,
        }))
    }

    fn restore_captured_file(captured: &CapturedRollbackAnchor) -> Result<(), String> {
        let Some((path, bytes)) = captured
            .target_path
            .as_deref()
            .zip(captured.file_bytes.as_deref())
        else {
            return Err("rollback: no captured target file snapshot to restore".to_owned());
        };
        std::fs::write(path, bytes).map_err(|error| {
            format!(
                "rollback: cannot restore target file `{}`: {error}",
                path.display()
            )
        })?;
        let observed = std::fs::read(path).map_err(|error| {
            format!(
                "rollback: cannot re-read restored target `{}`: {error}",
                path.display()
            )
        })?;
        if observed != bytes {
            return Err(format!(
                "rollback: restored target file `{}` does not equal the captured snapshot",
                path.display()
            ));
        }
        Ok(())
    }

    /// Re-reads the editor and the captured target so a test can assert the observed state.
    ///
    /// # Errors
    ///
    /// Returns a readable reason when the editor or target cannot be observed.
    pub(crate) fn observe_state(&self, captured: &CapturedRollbackAnchor) -> Result<Value, String> {
        let editor_text = self.read_canonical_text()?;
        let file_state = match (
            captured.target_path.as_deref(),
            captured.file_bytes.as_deref(),
        ) {
            (Some(path), Some(expected)) => {
                let observed = std::fs::read(path).map_err(|error| {
                    format!("rollback: cannot read target `{}`: {error}", path.display())
                })?;
                json!({
                    "target_path": path.to_string_lossy(),
                    "file_matches_snapshot": observed == expected,
                })
            }
            _ => json!({ "target_path": Value::Null, "file_matches_snapshot": true }),
        };
        Ok(json!({
            "editor_text": editor_text,
            "editor_matches_anchor": editor_text == captured.canonical_text,
            "file": file_state,
        }))
    }

    fn verify_after_rollback(
        &self,
        captured: &CapturedRollbackAnchor,
        restore_file: bool,
    ) -> Result<bool, String> {
        let observed_editor = self.read_canonical_text()?;
        if observed_editor != captured.canonical_text {
            return Err(format!(
                "rollback: editor text does not equal the anchor after rollback (expected {} chars, observed {} chars)",
                captured.canonical_text.chars().count(),
                observed_editor.chars().count()
            ));
        }
        let Some((path, expected)) = captured
            .target_path
            .as_deref()
            .zip(captured.file_bytes.as_deref())
        else {
            return Ok(true);
        };
        let observed_bytes = std::fs::read(path).map_err(|error| {
            format!(
                "rollback: cannot read restored target `{}`: {error}",
                path.display()
            )
        })?;
        let matches = observed_bytes == expected;
        if restore_file && !matches {
            return Err(format!(
                "rollback: target file `{}` does not equal the captured snapshot",
                path.display()
            ));
        }
        Ok(matches)
    }

    fn read_canonical_text(&self) -> Result<String, String> {
        let tool = crate::runtime_tools::TOOL_PREPARE_ANCHORS;
        let (_, editor) = self
            .context
            .resolve_editor(tool)
            .map_err(|error| format!("rollback: cannot resolve the editor: {error}"))?;
        self.context
            .read_element_text(&editor, tool)
            .map(|text| normalize_line_endings(&text))
            .map_err(|error| format!("rollback: cannot read the editor: {error}"))
    }

    fn observe_fingerprint(&self) -> Option<Fingerprint> {
        let text = self.read_canonical_text().ok()?;
        fingerprint_for(&text).ok()
    }
}

struct OutcomeDescription {
    status: &'static str,
    used_fallback: bool,
    final_fingerprint: String,
    evidence: Value,
}

fn describe_outcome(outcome: &RollbackOutcome) -> Result<OutcomeDescription, String> {
    match outcome {
        RollbackOutcome::Restored {
            completed_actions,
            final_fingerprint,
            used_fallback,
            evidence_refs,
        } => Ok(OutcomeDescription {
            status: "restored",
            used_fallback: *used_fallback,
            final_fingerprint: final_fingerprint.as_str().to_owned(),
            evidence: json!({
                "completed_actions": completed_actions,
                "evidence_refs": evidence_refs,
            }),
        }),
        RollbackOutcome::AlreadyAtAnchor {
            observed_fingerprint,
        } => Ok(OutcomeDescription {
            status: "already_at_anchor",
            used_fallback: false,
            final_fingerprint: observed_fingerprint.as_str().to_owned(),
            evidence: json!({ "completed_actions": 0 }),
        }),
        RollbackOutcome::Incident { report } => Err(format!(
            "rollback: {} ({:?}); evidence={:?}",
            report.summary(),
            report.error_code(),
            report.evidence_refs()
        )),
    }
}

fn fingerprint_for(text: &str) -> Result<Fingerprint, assistant_platform_api::PlatformError> {
    Fingerprint::parse(format!(
        "sha256:{}",
        BlobId::of_content(text.as_bytes()).as_str()
    ))
}

struct NotepadRollbackExecutor<P> {
    context: Arc<NotepadHandlerContext<P>>,
    canonical_text: String,
    target_path: Option<PathBuf>,
    file_bytes: Option<Vec<u8>>,
    restore_file: bool,
}

impl<P> RollbackExecutor for NotepadRollbackExecutor<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn execute(&self, action: &RollbackAction) -> Result<ActionOutcome, ExecutorFailure> {
        match action {
            RollbackAction::UndoStack { times, .. } => self.undo_stack(*times),
            RollbackAction::RestoreContentSnapshot { .. }
            | RollbackAction::RestoreShadowCopy { .. } => self.restore_snapshot(),
            RollbackAction::CompensatingAction { .. } => Err(ExecutorFailure::new(
                assistant_protocol::ErrorCode::CapabilityMissing,
                "rollback action kind is not implemented by the Notepad executor",
            )),
        }
    }
}

impl<P> NotepadRollbackExecutor<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn undo_stack(&self, times: UndoStepCount) -> Result<ActionOutcome, ExecutorFailure> {
        let tool = crate::runtime_tools::TOOL_PREPARE_ANCHORS;
        let (_window, editor) = self.context.resolve_editor(tool).map_err(|error| {
            ExecutorFailure::new(
                assistant_protocol::ErrorCode::TargetNotFound,
                format!("rollback action: cannot resolve the editor: {error}"),
            )
        })?;
        for _ in 0..times.get() {
            self.context
                .send_key_to_element(&editor, "Z", vec![KeyModifier::Control], tool)
                .map_err(|error| {
                    ExecutorFailure::new(
                        assistant_protocol::ErrorCode::VerifyFailed,
                        format!("rollback undo failed: {error}"),
                    )
                })?;
        }
        let observed = self.read_editor()?;
        if observed != self.canonical_text {
            return Err(ExecutorFailure::new(
                assistant_protocol::ErrorCode::VerifyFailed,
                "rollback undo did not reach the anchor text",
            ));
        }
        fingerprint_for(&observed)
            .map(ActionOutcome::new)
            .map_err(|error| {
                ExecutorFailure::new(
                    assistant_protocol::ErrorCode::Fatal,
                    format!("rollback: cannot build the post-action fingerprint: {error}"),
                )
            })
    }

    fn restore_snapshot(&self) -> Result<ActionOutcome, ExecutorFailure> {
        let tool = crate::runtime_tools::TOOL_PREPARE_ANCHORS;
        let (_, editor) = self.context.resolve_editor(tool).map_err(|error| {
            ExecutorFailure::new(
                assistant_protocol::ErrorCode::TargetNotFound,
                format!("rollback: cannot resolve the editor: {error}"),
            )
        })?;
        self.context
            .set_element_value(&editor, &self.canonical_text, tool)
            .map_err(|error| {
                ExecutorFailure::new(
                    assistant_protocol::ErrorCode::VerifyFailed,
                    format!("rollback: cannot restore editor text: {error}"),
                )
            })?;
        if self.restore_file {
            self.restore_file_bytes()?;
        }
        let observed = self.read_editor()?;
        fingerprint_for(&observed)
            .map(ActionOutcome::new)
            .map_err(|error| {
                ExecutorFailure::new(
                    assistant_protocol::ErrorCode::Fatal,
                    format!("rollback: cannot build the post-action fingerprint: {error}"),
                )
            })
    }

    fn restore_file_bytes(&self) -> Result<(), ExecutorFailure> {
        let Some((path, bytes)) = self.target_path.as_deref().zip(self.file_bytes.as_deref())
        else {
            return Ok(());
        };
        std::fs::write(path, bytes).map_err(|error| {
            ExecutorFailure::new(
                assistant_protocol::ErrorCode::VerifyFailed,
                format!(
                    "rollback: cannot restore target file `{}`: {error}",
                    path.display()
                ),
            )
        })?;
        let observed = std::fs::read(path).map_err(|error| {
            ExecutorFailure::new(
                assistant_protocol::ErrorCode::VerifyFailed,
                format!(
                    "rollback: cannot re-read target file `{}`: {error}",
                    path.display()
                ),
            )
        })?;
        if observed != bytes {
            return Err(ExecutorFailure::new(
                assistant_protocol::ErrorCode::VerifyFailed,
                "rollback: restored target file digest does not match the snapshot",
            ));
        }
        Ok(())
    }

    fn read_editor(&self) -> Result<String, ExecutorFailure> {
        let tool = crate::runtime_tools::TOOL_PREPARE_ANCHORS;
        let (_, editor) = self.context.resolve_editor(tool).map_err(|error| {
            ExecutorFailure::new(
                assistant_protocol::ErrorCode::TargetNotFound,
                format!("rollback: cannot resolve the editor: {error}"),
            )
        })?;
        self.context
            .read_element_text(&editor, tool)
            .map(|text| normalize_line_endings(&text))
            .map_err(|error| {
                ExecutorFailure::new(
                    assistant_protocol::ErrorCode::VerifyFailed,
                    format!("rollback: cannot read the editor: {error}"),
                )
            })
    }
}
