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
}
