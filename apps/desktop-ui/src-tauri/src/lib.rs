//! Tauri 2 shell for the desktop UI.
//!
//! The shell still has no system plugin and no business logic. It exposes the
//! typed UI command boundary from [`commands`]: payloads are validated locally
//! and forwarded to the Core process through an injected transport.

mod commands;
mod core_pipe;
mod core_pipe_session;
mod event_bridge;

pub use commands::{CommandState, CoreCommandTransport, UiCommandRejection, send_ui_command};
pub use core_pipe::{CorePipeConfig, CorePipeTransport};
pub use event_bridge::{EventStreamState, read_ui_event_stream, start_ui_event_stream};

/// Runs the desktop shell with no Core transport installed.
///
/// # Errors
///
/// Returns the Tauri startup error when the webview cannot be created or the
/// event loop exits abnormally.
pub fn run() -> Result<(), tauri::Error> {
    run_with_state(CommandState::unavailable())
}

/// Runs the desktop shell over an injected Core transport.
///
/// # Errors
///
/// Returns the Tauri startup error when the webview cannot be created or the
/// event loop exits abnormally.
pub fn run_with_state(state: CommandState) -> Result<(), tauri::Error> {
    run_with_event_state(state, EventStreamState::unavailable())
}

fn run_with_event_state(
    state: CommandState,
    event_state: EventStreamState,
) -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .manage(state)
        .manage(event_state)
        .invoke_handler(tauri::generate_handler![
            commands::send_ui_command,
            event_bridge::start_ui_event_stream,
            event_bridge::read_ui_event_stream
        ])
        .run(tauri::generate_context!())
}

/// Runs the desktop shell over the real Core UI pipe (ADR-0057 D7).
///
/// # Errors
///
/// Returns the Tauri startup error when the webview cannot be created or the
/// event loop exits abnormally.
pub fn run_with_core_pipe(config: CorePipeConfig) -> Result<(), tauri::Error> {
    let transport = std::sync::Arc::new(CorePipeTransport::new(config));
    run_with_event_state(
        CommandState::with_transport(transport.clone()),
        EventStreamState::new(transport),
    )
}
