//! Thin binary entry point. Diagnostics are written to the requested status file.

use std::path::PathBuf;
use std::process::ExitCode;

use assistant_automation_host::{HostConfig, run_host, write_status};

fn main() -> ExitCode {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let status_file = argument_value(&arguments, "--status-file");
    let result = HostConfig::parse(arguments).and_then(|config| run_host(&config));
    let status_result = match (&result, status_file) {
        (Ok(()), Some(path)) => write_status(&path, Ok(())),
        (Err(error), Some(path)) => write_status(&path, Err(error)),
        (_, None) => Ok(()),
    };
    if result.is_ok() && status_result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn argument_value(arguments: &[std::ffi::OsString], option: &str) -> Option<PathBuf> {
    let option = std::ffi::OsStr::new(option);
    arguments
        .windows(2)
        .find(|pair| pair.first().is_some_and(|value| value == option))
        .and_then(|pair| pair.get(1))
        .map(PathBuf::from)
}
