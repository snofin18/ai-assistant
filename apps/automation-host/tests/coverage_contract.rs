//! Public coverage contracts for the automation host process.
//!
//! The tests exercise configuration parsing and the status-file boundary without
//! starting a `NamedPipe` server or connecting to a real peer.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_automation_host::{HostConfig, run_host, write_status};
use assistant_ipc::IpcError;

fn arguments(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn unique_status_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "assistant-automation-host-{label}-{}-{nonce}.txt",
        std::process::id()
    ))
}

#[test]
fn test_parse_status_file_preserves_optional_configuration() {
    let status_file = unique_status_path("parse");
    let parsed = HostConfig::parse(arguments(&[
        "assistant-automation-host",
        "--pipe-name",
        "coverage-pipe",
        "--token-env",
        "COVERAGE_TOKEN",
        "--allow-peer",
        r"C:\Program Files\Assistant\agent-core.exe",
        "--handshake-timeout-ms",
        "750",
        "--heartbeat-timeout-ms",
        "1500",
        "--status-file",
        status_file.to_string_lossy().as_ref(),
    ]));

    assert!(parsed.is_ok());
    if let Ok(config) = parsed {
        assert_eq!(config.pipe_name, "coverage-pipe");
        assert_eq!(config.token_environment_variable, "COVERAGE_TOKEN");
        assert_eq!(config.handshake_timeout, Duration::from_millis(750));
        assert_eq!(config.heartbeat_timeout, Duration::from_millis(1_500));
        assert_eq!(config.status_file, Some(status_file));
    }
}

#[test]
fn test_write_status_success_and_error_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let status_file = unique_status_path("write");
    let success = write_status(&status_file, Ok(()));
    assert!(success.is_ok());
    let success_text = std::fs::read_to_string(&status_file)?;
    assert_eq!(success_text, "ok=true\n");

    let error = IpcError::Transport {
        message: "line one\nline two".to_owned(),
    };
    let failed = write_status(&status_file, Err(&error));
    assert!(failed.is_ok());
    let failed_text = std::fs::read_to_string(&status_file)?;
    assert_eq!(
        failed_text,
        "ok=false\nmessage=IPC transport failure: line one line two\n"
    );

    let write_failure = write_status(&std::env::temp_dir(), Ok(()));
    assert!(matches!(write_failure, Err(IpcError::Transport { .. })));

    std::fs::remove_file(status_file)?;
    Ok(())
}

#[test]
fn test_run_host_missing_token_fails_before_namedpipe() -> Result<(), Box<dyn std::error::Error>> {
    let token_variable = "ASSISTANT_AUTOMATION_HOST_COVERAGE_TOKEN_MUST_NOT_EXIST";
    assert!(std::env::var_os(token_variable).is_none());

    let parsed = HostConfig::parse(arguments(&[
        "assistant-automation-host",
        "--pipe-name",
        "coverage-pipe",
        "--token-env",
        token_variable,
        "--allow-peer",
        r"C:\Program Files\Assistant\agent-core.exe",
    ]));
    let config = parsed?;

    let ended = run_host(&config);
    assert!(matches!(ended, Err(IpcError::InvalidAuthenticationToken)));
    Ok(())
}
