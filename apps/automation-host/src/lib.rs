//! Automation host configuration, authentication, and session loop.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, HeartbeatMonitor, IpcError, IpcResult, NamedPipeTransport, ServerHello, Transport,
    WireMessage, generate_session_id, peer_process_image_path, server_handshake,
};

/// Parsed host command-line configuration.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HostConfig {
    /// Named pipe component without the `\\.\pipe\` prefix.
    pub pipe_name: String,
    /// Environment variable containing the expected authentication token.
    pub token_environment_variable: String,
    /// Allowed canonical peer image paths. Empty means deny all.
    pub allowed_peer_images: Vec<PathBuf>,
    /// Deadline for creating and accepting the pipe.
    pub handshake_timeout: Duration,
    /// Maximum silence allowed before the watchdog fires.
    pub heartbeat_timeout: Duration,
    /// Optional machine-readable status destination.
    pub status_file: Option<PathBuf>,
}

impl HostConfig {
    /// Parse host arguments.
    ///
    /// # Errors
    /// Returns [`IpcError::Serialization`] for missing/unknown arguments or an
    /// invalid timeout value.
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> IpcResult<Self> {
        let mut arguments = arguments.into_iter();
        let _executable = arguments.next();
        let mut pipe_name = None;
        let mut token_environment_variable = None;
        let mut allowed_peer_images = Vec::new();
        let mut handshake_timeout = Duration::from_secs(5);
        let mut heartbeat_timeout = Duration::from_secs(5);
        let mut status_file = None;

        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| invalid_configuration("arguments must be valid Unicode"))?;
            match argument.as_str() {
                "--pipe-name" => {
                    pipe_name = Some(next_value(&mut arguments, "--pipe-name")?);
                }
                "--token-env" => {
                    token_environment_variable = Some(next_value(&mut arguments, "--token-env")?);
                }
                "--allow-peer" => {
                    allowed_peer_images
                        .push(PathBuf::from(next_value(&mut arguments, "--allow-peer")?));
                }
                "--status-file" => {
                    status_file = Some(PathBuf::from(next_value(&mut arguments, "--status-file")?));
                }
                "--handshake-timeout-ms" => {
                    handshake_timeout =
                        parse_timeout(&next_value(&mut arguments, "--handshake-timeout-ms")?)?;
                }
                "--heartbeat-timeout-ms" => {
                    heartbeat_timeout =
                        parse_timeout(&next_value(&mut arguments, "--heartbeat-timeout-ms")?)?;
                }
                _ => {
                    return Err(invalid_configuration(format!(
                        "unknown argument: {argument}"
                    )));
                }
            }
        }

        let pipe_name =
            pipe_name.ok_or_else(|| invalid_configuration("--pipe-name is required"))?;
        let token_environment_variable = token_environment_variable
            .ok_or_else(|| invalid_configuration("--token-env is required"))?;
        if allowed_peer_images.is_empty() {
            return Err(invalid_configuration(
                "at least one --allow-peer entry is required",
            ));
        }
        if allowed_peer_images
            .iter()
            .any(|path| path.as_os_str().is_empty())
        {
            return Err(invalid_configuration("--allow-peer cannot be empty"));
        }
        if heartbeat_timeout.is_zero() {
            return Err(invalid_configuration(
                "--heartbeat-timeout-ms must be greater than zero",
            ));
        }
        Ok(Self {
            pipe_name,
            token_environment_variable,
            allowed_peer_images,
            handshake_timeout,
            heartbeat_timeout,
            status_file,
        })
    }

    fn is_peer_allowed(&self, image_path: &str) -> bool {
        let candidate = normalize_image_path(image_path);
        self.allowed_peer_images
            .iter()
            .any(|allowed| normalize_image_path(&allowed.to_string_lossy()) == candidate)
    }
}

/// Run the host until the connected peer disconnects or the watchdog fires.
///
/// # Errors
/// Fails closed for missing token, rejected peer identity, handshake failure,
/// malformed heartbeat, or transport timeout/disconnect.
pub fn run_host(config: &HostConfig) -> IpcResult<()> {
    report_stage(config, "starting")?;
    let authentication_token = std::env::var(&config.token_environment_variable)
        .map_err(|_| IpcError::InvalidAuthenticationToken)?;

    let mut transport = NamedPipeTransport::server(&config.pipe_name)?;
    report_stage(config, "listening")?;
    transport.accept(config.handshake_timeout)?;
    report_stage(config, "accepted")?;
    let peer_process_id = transport.peer_process_id()?;
    let peer_image_path = peer_process_image_path(peer_process_id)?;
    if !config.is_peer_allowed(&peer_image_path) {
        return Err(IpcError::PeerIdentityRejected {
            image_path: peer_image_path,
        });
    }
    report_stage(config, "peer-allowed")?;

    let session_id = generate_session_id()?;
    let handshake = server_handshake(
        &mut transport,
        &authentication_token,
        Vec::new(),
        &session_id,
        config.handshake_timeout,
    )?;
    report_stage(config, "handshaken")?;
    run_session(&mut transport, &handshake.server_hello, config)
}

fn run_session(
    transport: &mut NamedPipeTransport,
    server_hello: &ServerHello,
    config: &HostConfig,
) -> IpcResult<()> {
    let heartbeat_timeout = config.heartbeat_timeout;
    let now = unix_time_ms()?;
    let mut monitor = HeartbeatMonitor::new(heartbeat_timeout, now);
    send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
    report_stage(config, "heartbeat-sent")?;
    let poll_interval = heartbeat_timeout / 3;

    loop {
        let now = unix_time_ms()?;
        monitor.check(now)?;
        let wait = monitor.remaining(now).min(poll_interval);
        match transport.recv(wait) {
            Ok(WireMessage::Heartbeat(heartbeat)) => {
                if heartbeat.session_id != server_hello.session_id || !heartbeat.alive {
                    return Err(IpcError::UnexpectedMessage {
                        expected: "live Heartbeat for the negotiated session",
                        actual: "Heartbeat",
                    });
                }
                monitor.observe(now);
                send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
            }
            Ok(_) => {
                monitor.observe(now);
            }
            Err(IpcError::Timeout { .. }) => {
                let now = unix_time_ms()?;
                monitor.check(now)?;
                send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn send_heartbeat(
    transport: &mut NamedPipeTransport,
    session_id: &str,
    timestamp_unix_ms: u64,
    timeout: Duration,
) -> IpcResult<()> {
    transport.send(
        &WireMessage::Heartbeat(Heartbeat {
            session_id: session_id.to_string(),
            timestamp_unix_ms,
            alive: true,
        }),
        timeout,
    )
}

/// Write a minimal machine-readable status line.
///
/// # Errors
/// Returns an I/O-shaped IPC transport error if the status file cannot be written.
pub fn write_status(path: &Path, outcome: Result<(), &IpcError>) -> IpcResult<()> {
    let text = match outcome {
        Ok(()) => "ok=true\n".to_string(),
        Err(error) => format!(
            "ok=false\nmessage={}\n",
            error.to_string().replace(['\r', '\n'], " ")
        ),
    };
    std::fs::write(path, text).map_err(|error| IpcError::Transport {
        message: format!("status file write failed: {error}"),
    })
}

fn report_stage(config: &HostConfig, stage: &str) -> IpcResult<()> {
    let Some(path) = config.status_file.as_ref() else {
        return Ok(());
    };
    std::fs::write(path, format!("ok=pending\nstage={stage}\n")).map_err(|error| {
        IpcError::Transport {
            message: format!("status file write failed: {error}"),
        }
    })
}

fn next_value(arguments: &mut impl Iterator<Item = OsString>, option: &str) -> IpcResult<String> {
    arguments
        .next()
        .ok_or_else(|| invalid_configuration(format!("{option} requires a value")))?
        .into_string()
        .map_err(|_| invalid_configuration(format!("{option} value must be valid Unicode")))
}

fn parse_timeout(value: &str) -> IpcResult<Duration> {
    let milliseconds = value
        .parse::<u64>()
        .map_err(|_| invalid_configuration("timeout must be an unsigned integer"))?;
    if milliseconds == 0 {
        return Err(invalid_configuration("timeout must be greater than zero"));
    }
    Ok(Duration::from_millis(milliseconds))
}

fn invalid_configuration(message: impl Into<String>) -> IpcError {
    IpcError::Serialization {
        message: message.into(),
    }
}

fn normalize_image_path(path: &str) -> String {
    path.trim().replace('/', "\\").to_ascii_lowercase()
}

fn unix_time_ms() -> IpcResult<u64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| IpcError::Transport {
            message: format!("system clock is before Unix epoch: {error}"),
        })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| IpcError::Transport {
        message: "system time does not fit in u64 milliseconds".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::HostConfig;
    use crate::IpcError;

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn test_parse_host_config() {
        let parsed = HostConfig::parse(arguments(&[
            "host",
            "--pipe-name",
            "test-pipe",
            "--token-env",
            "TOKEN",
            "--allow-peer",
            r"C:\app\core.exe",
            "--handshake-timeout-ms",
            "1000",
            "--heartbeat-timeout-ms",
            "2000",
        ]));
        assert!(parsed.is_ok());
        if let Ok(config) = parsed {
            assert_eq!(config.handshake_timeout, Duration::from_millis(1_000));
            assert_eq!(
                config.allowed_peer_images,
                vec![PathBuf::from(r"C:\app\core.exe")]
            );
        }
    }

    #[test]
    fn test_parse_rejects_missing_peer_allow_list() {
        let parsed = HostConfig::parse(arguments(&[
            "host",
            "--pipe-name",
            "test-pipe",
            "--token-env",
            "TOKEN",
        ]));
        assert!(matches!(parsed, Err(IpcError::Serialization { .. })));
    }

    #[test]
    fn test_peer_path_comparison_is_case_and_separator_insensitive() {
        let parsed = HostConfig::parse(arguments(&[
            "host",
            "--pipe-name",
            "test-pipe",
            "--token-env",
            "TOKEN",
            "--allow-peer",
            r"C:/Apps/Core.EXE",
        ]));
        assert!(parsed.is_ok());
        if let Ok(config) = parsed {
            assert!(config.is_peer_allowed(r"c:\apps\core.exe"));
            assert!(!config.is_peer_allowed(r"c:\apps\other.exe"));
        }
    }
}
