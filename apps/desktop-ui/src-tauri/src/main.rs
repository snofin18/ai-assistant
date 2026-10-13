//! Desktop UI binary entry point.
//!
//! Responsibility: translate `assistant_desktop_ui::run` into a process exit code.
//! Boundary: no business logic, IPC policy, or platform calls live here.
//! Invariant: startup failures are reported to stderr and never converted into a success code.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    match assistant_desktop_ui::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("assistant-desktop-ui failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
