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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::argument_value;

    #[test]
    fn test_argument_value_returns_the_token_after_the_exact_option() {
        let arguments = [
            OsString::from("host"),
            OsString::from("--status-file"),
            OsString::from(r"C:\temp\status.txt"),
        ];

        assert_eq!(
            argument_value(&arguments, "--status-file"),
            Some(PathBuf::from(r"C:\temp\status.txt"))
        );
    }

    #[test]
    fn test_argument_value_requires_an_exact_adjacent_option() {
        let arguments = [
            OsString::from("host"),
            OsString::from("--status-file-prefix"),
            OsString::from("ignored"),
            OsString::from("--status-file"),
        ];

        assert_eq!(argument_value(&arguments, "--status-file"), None);
        assert_eq!(
            argument_value(&arguments, "--status-file-prefix"),
            Some(PathBuf::from("ignored"))
        );
    }
}
