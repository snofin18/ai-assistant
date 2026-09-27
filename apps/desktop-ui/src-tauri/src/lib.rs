//! Minimal Tauri 2 shell.
//!
//! The shell has no system plugin and no business logic. Typed IPC commands are
//! added by the feature cards that consume this skeleton.

/// Runs the desktop shell.
///
/// # Errors
///
/// Returns the Tauri startup error when the webview cannot be created or the
/// event loop exits abnormally.
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default().run(tauri::generate_context!())
}
