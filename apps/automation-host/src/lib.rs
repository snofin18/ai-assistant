//! Automation host configuration, authentication, and session loop.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, HeartbeatMonitor, IpcError, IpcResult, NamedPipeTransport, RequestMessage,
    ResponseMessage, ServerHello, Transport, WireMessage, generate_session_id,
    peer_process_image_path, server_handshake,
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
    run_host_with_dispatcher(config, &mut UnavailableDispatcher)
}

/// Runs the Host with an explicit request dispatcher.
///
/// The dispatcher executes one validated tool request and must return a
/// response with the same correlation id. A missing or mismatched response
/// fails the session instead of leaving the client waiting silently.
///
/// # Errors
///
/// Propagates authentication, transport, heartbeat, dispatcher, or response
/// correlation failures.
pub fn run_host_with_dispatcher<D>(config: &HostConfig, dispatcher: &mut D) -> IpcResult<()>
where
    D: RequestDispatcher,
{
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
    run_session(&mut transport, &handshake.server_hello, config, dispatcher)
}

/// Boundary implemented by the binary that owns actual tool handlers.
pub trait RequestDispatcher {
    /// Dispatches one authenticated request and returns its matching response.
    ///
    /// # Errors
    ///
    /// Returns an IPC-shaped error when the request cannot be dispatched.
    fn dispatch(&mut self, request: &RequestMessage) -> IpcResult<ResponseMessage>;
}

/// Executes one request through the Host dispatcher.
///
/// # Errors
///
/// Returns an IPC error when the dispatcher fails or returns a response whose
/// correlation id does not match the request.
pub fn process_request<D>(
    dispatcher: &mut D,
    request: &RequestMessage,
) -> IpcResult<ResponseMessage>
where
    D: RequestDispatcher,
{
    let response = dispatcher.dispatch(request)?;
    if response.correlation_id != request.correlation_id {
        return Err(IpcError::UnexpectedMessage {
            expected: "ResponseMessage with matching correlation_id",
            actual: "mismatched response correlation_id",
        });
    }
    Ok(response)
}

struct UnavailableDispatcher;

impl RequestDispatcher for UnavailableDispatcher {
    fn dispatch(&mut self, _request: &RequestMessage) -> IpcResult<ResponseMessage> {
        Err(IpcError::UnexpectedMessage {
            expected: "a configured Host request dispatcher",
            actual: "Request",
        })
    }
}

fn run_session<T, D>(
    transport: &mut T,
    server_hello: &ServerHello,
    config: &HostConfig,
    dispatcher: &mut D,
) -> IpcResult<()>
where
    T: Transport,
    D: RequestDispatcher,
{
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
            Ok(WireMessage::Request(request)) => {
                monitor.observe(now);
                let response = process_request(dispatcher, &request)?;
                transport.send(&WireMessage::Response(response), heartbeat_timeout)?;
            }
            Ok(other) => {
                return Err(IpcError::UnexpectedMessage {
                    expected: "Heartbeat or Request",
                    actual: other.kind(),
                });
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

fn send_heartbeat<T>(
    transport: &mut T,
    session_id: &str,
    timestamp_unix_ms: u64,
    timeout: Duration,
) -> IpcResult<()>
where
    T: Transport,
{
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
    use std::collections::VecDeque;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::Duration;

    use assistant_ipc::{
        Heartbeat, IpcError, RequestMessage, ResponseMessage, ServerHello, Transport, WireMessage,
    };
    use assistant_protocol::{ErrorCode, ToolEnvelope, serde_json::json};

    use super::{HostConfig, RequestDispatcher, process_request, run_session};

    #[derive(Default)]
    struct ScriptedTransport {
        incoming: VecDeque<WireMessage>,
        sent: Vec<WireMessage>,
        timeouts_before_disconnect: usize,
    }

    impl Transport for ScriptedTransport {
        fn connect(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
            Ok(())
        }

        fn accept(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
            Ok(())
        }

        fn send(
            &mut self,
            message: &WireMessage,
            _timeout: Duration,
        ) -> assistant_ipc::IpcResult<()> {
            self.sent.push(message.clone());
            Ok(())
        }

        fn recv(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<WireMessage> {
            if self.timeouts_before_disconnect > 0 {
                self.timeouts_before_disconnect -= 1;
                return Err(IpcError::Timeout {
                    operation: "scripted receive",
                    timeout_ms: 0,
                });
            }
            self.incoming
                .pop_front()
                .ok_or_else(|| IpcError::Disconnected {
                    message: "scripted transport is exhausted".to_owned(),
                })
        }

        fn close(&mut self) -> assistant_ipc::IpcResult<()> {
            Ok(())
        }

        fn peer_process_id(&self) -> assistant_ipc::IpcResult<u32> {
            Ok(1)
        }
    }

    fn session_config() -> HostConfig {
        HostConfig {
            pipe_name: "test-pipe".to_owned(),
            token_environment_variable: "UNUSED_IN_SESSION_TEST".to_owned(),
            allowed_peer_images: Vec::new(),
            handshake_timeout: Duration::from_secs(1),
            heartbeat_timeout: Duration::from_millis(2_000),
            status_file: None,
        }
    }

    struct EchoDispatcher;

    impl RequestDispatcher for EchoDispatcher {
        fn dispatch(
            &mut self,
            request: &RequestMessage,
        ) -> assistant_ipc::IpcResult<ResponseMessage> {
            Ok(ResponseMessage::new(
                request.correlation_id.clone(),
                request.tool_invoke.clone(),
            ))
        }
    }

    struct MismatchedDispatcher;

    impl RequestDispatcher for MismatchedDispatcher {
        fn dispatch(
            &mut self,
            request: &RequestMessage,
        ) -> assistant_ipc::IpcResult<ResponseMessage> {
            Ok(ResponseMessage::new(
                format!("{}-wrong", request.correlation_id),
                request.tool_invoke.clone(),
            ))
        }
    }

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

    #[test]
    fn test_request_dispatch_returns_matching_response() -> Result<(), Box<dyn std::error::Error>> {
        let request = RequestMessage::new(
            "c_1",
            "document",
            ToolEnvelope::ok(
                "notepad.text.read".to_owned(),
                "t_1".to_owned(),
                "s_1".to_owned(),
                json!({"text": "hello"}),
            ),
        );
        let response = process_request(&mut EchoDispatcher, &request)?;
        assert_eq!(response.correlation_id, "c_1");
        assert_eq!(response.result, request.tool_invoke);
        Ok(())
    }

    #[test]
    fn test_mismatched_response_correlation_is_rejected() {
        let request = RequestMessage::new(
            "c_1",
            "document",
            ToolEnvelope::error(
                "notepad.text.read".to_owned(),
                "t_1".to_owned(),
                "s_1".to_owned(),
                ErrorCode::Fatal,
                "not dispatched",
            ),
        );
        let result = process_request(&mut MismatchedDispatcher, &request);
        assert!(matches!(result, Err(IpcError::UnexpectedMessage { .. })));
    }

    #[test]
    fn test_session_dispatches_request_and_sends_matching_response()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = RequestMessage::new(
            "c_1",
            "document",
            ToolEnvelope::ok(
                "notepad.text.read".to_owned(),
                "t_1".to_owned(),
                "s_1".to_owned(),
                json!({"text": "hello"}),
            ),
        );
        let mut transport = ScriptedTransport::default();
        transport.incoming.push_back(WireMessage::Request(request));
        let server_hello = ServerHello {
            version: assistant_ipc::IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
        };
        let config = session_config();

        // The scripted transport is exhausted after one request, so the session
        // ends with an explicit disconnect once the request is handled.
        let ended = run_session(&mut transport, &server_hello, &config, &mut EchoDispatcher);
        assert!(matches!(ended, Err(IpcError::Disconnected { .. })));

        assert!(matches!(
            transport.sent.first(),
            Some(WireMessage::Heartbeat(_))
        ));
        let response = transport
            .sent
            .iter()
            .find_map(|message| match message {
                WireMessage::Response(response) => Some(response),
                _ => None,
            })
            .ok_or("the session did not send a response")?;
        assert_eq!(response.correlation_id, "c_1");
        Ok(())
    }

    #[test]
    fn test_session_accepts_negotiated_heartbeat_then_disconnects() {
        let server_hello = ServerHello {
            version: assistant_ipc::IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
        };
        let mut transport = ScriptedTransport::default();
        transport
            .incoming
            .push_back(WireMessage::Heartbeat(Heartbeat {
                session_id: server_hello.session_id.clone(),
                timestamp_unix_ms: 1,
                alive: true,
            }));

        let ended = run_session(
            &mut transport,
            &server_hello,
            &session_config(),
            &mut EchoDispatcher,
        );

        assert!(matches!(ended, Err(IpcError::Disconnected { .. })));
        assert!(transport.sent.iter().any(|message| matches!(
            message,
            WireMessage::Heartbeat(Heartbeat { alive: true, .. })
        )));
    }

    #[test]
    fn test_session_rejects_heartbeat_for_another_session() {
        let server_hello = ServerHello {
            version: assistant_ipc::IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
        };
        let mut transport = ScriptedTransport::default();
        transport
            .incoming
            .push_back(WireMessage::Heartbeat(Heartbeat {
                session_id: "018f6d4e-52a1-7b03-8f22-000000000000".to_owned(),
                timestamp_unix_ms: 1,
                alive: true,
            }));

        let ended = run_session(
            &mut transport,
            &server_hello,
            &session_config(),
            &mut EchoDispatcher,
        );

        assert!(matches!(ended, Err(IpcError::UnexpectedMessage { .. })));
    }

    #[test]
    fn test_session_rejects_non_request_messages() {
        let server_hello = ServerHello {
            version: assistant_ipc::IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
        };
        let mut transport = ScriptedTransport::default();
        transport
            .incoming
            .push_back(WireMessage::ServerHello(server_hello.clone()));

        let ended = run_session(
            &mut transport,
            &server_hello,
            &session_config(),
            &mut EchoDispatcher,
        );

        assert!(matches!(ended, Err(IpcError::UnexpectedMessage { .. })));
    }

    #[test]
    fn test_session_sends_heartbeat_after_receive_timeout() {
        let server_hello = ServerHello {
            version: assistant_ipc::IPC_PROTOCOL_VERSION,
            capabilities: Vec::new(),
            session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
        };
        let mut transport = ScriptedTransport {
            timeouts_before_disconnect: 1,
            ..ScriptedTransport::default()
        };

        let ended = run_session(
            &mut transport,
            &server_hello,
            &session_config(),
            &mut EchoDispatcher,
        );

        assert!(matches!(ended, Err(IpcError::Disconnected { .. })));
        assert!(
            transport
                .sent
                .iter()
                .filter(|message| matches!(message, WireMessage::Heartbeat(_)))
                .count()
                >= 2
        );
    }

    #[test]
    fn test_parse_rejects_unknown_argument() {
        let parsed = HostConfig::parse(arguments(&["host", "--unknown"]));
        assert!(matches!(parsed, Err(IpcError::Serialization { .. })));
    }

    #[test]
    fn test_parse_rejects_empty_peer_entry() {
        let parsed = HostConfig::parse(arguments(&[
            "host",
            "--pipe-name",
            "test-pipe",
            "--token-env",
            "TOKEN",
            "--allow-peer",
            "",
        ]));
        assert!(matches!(parsed, Err(IpcError::Serialization { .. })));
    }

    #[test]
    fn test_parse_rejects_zero_heartbeat_timeout() {
        let parsed = HostConfig::parse(arguments(&[
            "host",
            "--pipe-name",
            "test-pipe",
            "--token-env",
            "TOKEN",
            "--allow-peer",
            r"C:\app\core.exe",
            "--heartbeat-timeout-ms",
            "0",
        ]));
        assert!(matches!(parsed, Err(IpcError::Serialization { .. })));
    }

    #[test]
    fn test_parse_rejects_non_numeric_handshake_timeout() {
        let parsed = HostConfig::parse(arguments(&[
            "host",
            "--pipe-name",
            "test-pipe",
            "--token-env",
            "TOKEN",
            "--allow-peer",
            r"C:\app\core.exe",
            "--handshake-timeout-ms",
            "not-a-number",
        ]));
        assert!(matches!(parsed, Err(IpcError::Serialization { .. })));
    }
}
