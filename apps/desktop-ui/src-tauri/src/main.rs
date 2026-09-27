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
