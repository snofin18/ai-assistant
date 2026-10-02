//! Host-side operations used by reserved runtime steps (ADR-0061 D7).
//!
//! Responsibilities:
//! - expose the two host operations the 1a task packages need;
//! - keep platform access behind the already assembled Notepad handler context.
//!
//! Boundaries:
//! - these operations are not model-visible tools;
//! - they return serializable data only, never platform handles.

use serde_json::Value;

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
}
