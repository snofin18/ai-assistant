//! Desktop UI binary entry point.
//!
//! Responsibility: load authenticated Core transport configuration and translate
//! the desktop UI shell result into a process exit code.
//! Boundary: no business logic, IPC policy, or platform calls live here.
//! Invariant: startup failures are reported to stderr and never converted into a success code.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use assistant_desktop_ui::{CorePipeConfig, run_with_core_pipe};

fn main() -> std::process::ExitCode {
    let config = match CorePipeConfig::from_environment() {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "assistant-desktop-ui configuration failed: {}: {}",
                error.code, error.message
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    match run_with_core_pipe(config) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("assistant-desktop-ui failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
