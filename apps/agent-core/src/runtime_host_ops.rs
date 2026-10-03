//! Host-side operations used by reserved runtime steps (ADR-0061 D7).
//!
//! Responsibilities:
//! - expose the read-only and rollback host operations the 1a task packages need;
//! - keep platform access behind the already assembled Notepad handler context.
//!
//! Boundaries:
//! - these operations are not model-visible tools;
//! - they return serializable data only, never platform handles.

use assistant_protocol::ErrorCode;
use serde_json::Value;

/// Structured failure returned by a reserved host operation.
///
/// The existing rollback-oriented methods keep their string errors because their callers
/// already map them to fixed codes. File-channel reads need to preserve the distinction
/// between malformed arguments and a missing target, so this error carries the code at the
/// boundary where the filesystem outcome is known.
#[derive(Debug, Clone)]
pub struct ReservedHostOperationError {
    code: ErrorCode,
    message: String,
}

impl ReservedHostOperationError {
    /// Creates an explicit host-operation failure.
    #[must_use]
    pub const fn new(code: ErrorCode, message: String) -> Self {
        Self { code, message }
    }

    /// Returns the stable error code the caller must surface.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the readable failure reason.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Binary-layer host operations referenced by reserved runtime tools.
pub trait ReservedHostOperations: Send + Sync {
    /// Inspects a target path without modifying it.
    ///
    /// # Errors
    ///
    /// Returns a readable reason when the path is invalid or cannot be
    /// inspected.
    fn inspect_target_path(&self, target_path: &str) -> Result<Value, String>;

    /// Sets the current editor value through the injected platform.
    ///
    /// # Errors
    ///
    /// Returns a readable reason when the value cannot be written or read back.
    fn set_editor_value(&self, text: &str) -> Result<Value, String>;

    /// Reads a bounded UTF-8 prefix from the target file.
    ///
    /// `target_path` may be omitted by the plan step when the assembly already captured
    /// the task's absolute target path. The implementation must still validate that the
    /// resolved path is absolute and free of parent-directory traversal.
    ///
    /// # Errors
    ///
    /// Returns [`ReservedHostOperationError`] when the path or budget is invalid, the file
    /// cannot be opened, or the selected bytes are not valid UTF-8 at a legal boundary.
    fn read_utf8_prefix(
        &self,
        target_path: Option<&str>,
        max_text_bytes: u64,
    ) -> Result<Value, ReservedHostOperationError>;

    /// Observes the target's initial fingerprint through the injected platform (ADR-0065).
    ///
    /// A task package declares this as its first executable step and without a condition, so a
    /// later step whose `when` is false can be skipped against a fingerprint that a committed
    /// step really published. The returned payload must carry the observed value under both
    /// `fingerprint` and `previous_fingerprint`: the observation changes nothing, so the two are
    /// the same reading and the step's `state_unchanged` postcondition stays honest.
    ///
    /// The value must come from the injected `UiAutomationProvider` - never from a constant, a
    /// content digest, or a build-configuration branch (ADR-0065 D2/D3/D4).
    ///
    /// # Errors
    ///
    /// Returns a readable reason when the target window cannot be resolved or the platform
    /// cannot report a fingerprint; the caller turns that into an explicit `ErrorCode` instead
    /// of publishing a placeholder.
    fn capture_initial_fingerprint(&self) -> Result<Value, String>;

    /// Captures the physical pre-write snapshot for one task step (ADR-0062 D1/D2).
    ///
    /// # Errors
    ///
    /// Returns a readable reason when the editor text, the target file bytes, or the
    /// fingerprint cannot be observed, or when the captured anchor is invalid.
    fn capture_rollback_anchor(
        &self,
        task_id: &str,
        step_id: &str,
        sequence: u32,
        target_path: Option<&str>,
    ) -> Result<Value, String>;

    /// Executes the captured rollback and re-observes the editor and target file.
    ///
    /// # Errors
    ///
    /// Returns a readable reason when no anchor is captured for the task, when `crates/undo`
    /// produces an incident, or when the post-rollback observation does not match the anchor.
    fn execute_rollback(
        &self,
        task_id: &str,
        step_id: &str,
        restore_file: bool,
    ) -> Result<Value, String>;

    /// Re-reads the captured anchor state so the caller can assert what was actually observed.
    ///
    /// # Errors
    ///
    /// Returns a readable reason when no anchor is captured for the task or the target cannot
    /// be observed.
    fn observe_rollback_state(&self, task_id: &str) -> Result<Value, String>;

    /// Releases any per-task state the host kept for the task (ADR-0062 / leak guards).
    ///
    /// # Errors
    ///
    /// Returns a readable reason when a registry cannot be locked.
    fn release_task(&self, task_id: &str) -> Result<(), String>;
}
