//! Core-side UI command listener (ADR-0057 D1 / D4 / D5 / D7).
//!
//! Responsibilities:
//! - accept one authenticated UI connection and run one session loop;
//! - dispatch each validated `UiIpcRequest` through a [`UiCommandHandler`];
//! - answer with a `UiIpcResponse` carrying the same correlation id, or a
//!   structured rejection that names a stable `ErrorCode`.
//!
//! Boundaries:
//! - does not parse UI semantics itself (that is [`crate::ui_ipc`]);
//! - does not execute tools, resolve targets, or decide policy;
//! - does not accept tool-shaped `Request` / `Response` on this channel.
//!
//! Invariants:
//! 1. every `UiRequest` gets exactly one `UiResponse` with the same id;
//! 2. a rejection never happens after a side effect (`dispatch_ui_command`
//!    parses first);
//! 3. tool-shaped traffic and unknown messages fail the session instead of
//!    being ignored;
//! 4. heartbeat silence fails the session explicitly.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, HeartbeatMonitor, IpcError, IpcResult, NamedPipeTransport, ServerHello, Transport,
    UiIpcEvent, UiIpcRequest, UiIpcResponse, WireMessage, generate_session_id,
    peer_process_image_path, server_handshake,
};

use crate::ui_ipc::{UiCommandHandler, UiEvent, dispatch_ui_command};

/// Source of UI events pushed on a live session (ADR-0057 D5).
///
/// The session loop drains this **before** it blocks on the next read, so a
/// slow client cannot stall event delivery behind a pending request.
pub trait UiEventSource {
    /// Returns the events produced since the previous call.
    ///
    /// An empty vector means "nothing new" — never an error placeholder.
    fn drain(&mut self) -> Vec<UiEvent>;
}

/// A source that never produces events (used when events are not wired yet).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoEvents;

impl UiEventSource for NoEvents {
    fn drain(&mut self) -> Vec<UiEvent> {
        Vec::new()
    }
}

/// Pushes every pending event as a one-way `UiIpcEvent`.
///
/// # Errors
///
/// Returns a serialization error when an event cannot be encoded, or a
/// transport error when the peer is gone.
pub fn push_events<T, S>(transport: &mut T, source: &mut S, timeout: Duration) -> IpcResult<usize>
where
    T: Transport,
    S: UiEventSource,
{
    let mut pushed = 0;
    for event in source.drain() {
        let value = serde_json::to_value(&event).map_err(|error| IpcError::Serialization {
            message: error.to_string(),
        })?;
        transport.send(&WireMessage::UiEvent(UiIpcEvent::new(value)), timeout)?;
        pushed += 1;
    }
    Ok(pushed)
}

/// Configuration for the Core-side UI listener (ADR-0057 D4).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct UiServerConfig {
    /// Named pipe component without the `\\.\pipe\` prefix.
    pub pipe_name: String,
    /// Environment variable holding the one-shot authentication token.
    pub token_environment_variable: String,
    /// Allowed canonical peer image paths. Empty means deny all.
    pub allowed_peer_images: Vec<PathBuf>,
    /// Deadline for creating and accepting the pipe.
    pub handshake_timeout: Duration,
    /// Maximum silence allowed before the watchdog fires.
    pub heartbeat_timeout: Duration,
}

impl UiServerConfig {
    /// Creates a config that denies every peer until one is added.
    #[must_use]
    pub fn new(
        pipe_name: impl Into<String>,
        token_environment_variable: impl Into<String>,
        heartbeat_timeout: Duration,
    ) -> Self {
        Self {
            pipe_name: pipe_name.into(),
            token_environment_variable: token_environment_variable.into(),
            allowed_peer_images: Vec::new(),
            handshake_timeout: Duration::from_secs(5),
            heartbeat_timeout,
        }
    }

    /// Adds one allowed peer image.
    #[must_use]
    pub fn with_allowed_peer(mut self, peer_image: impl Into<PathBuf>) -> Self {
        self.allowed_peer_images.push(peer_image.into());
        self
    }

    /// Returns whether a peer image path is on the allow-list.
    ///
    /// Comparison is case- and separator-insensitive and never hashes: the peer
    /// identity must stay debuggable when a connection is rejected (ADR-0057 D4
    /// 沿用 automation-host 的既有模式).
    #[must_use]
    pub fn is_peer_allowed(&self, peer_image: &str) -> bool {
        let normalized = normalize_image_path(peer_image);
        self.allowed_peer_images
            .iter()
            .any(|allowed| normalize_image_path(&allowed.to_string_lossy()) == normalized)
    }
}

/// Serves exactly one UI session on the configured named pipe.
///
/// # Errors
///
/// Fails closed for a missing token, rejected peer identity, handshake failure,
/// tool-shaped traffic on the UI channel, or transport timeout/disconnect.
pub fn serve<H: UiCommandHandler>(config: &UiServerConfig, handler: &mut H) -> IpcResult<()> {
    let authentication_token = std::env::var(&config.token_environment_variable)
        .map_err(|_| IpcError::InvalidAuthenticationToken)?;

    let mut transport = NamedPipeTransport::server(&config.pipe_name)?;
    transport.accept(config.handshake_timeout)?;
    let peer_process_id = transport.peer_process_id()?;
    let peer_image_path = peer_process_image_path(peer_process_id)?;
    if !config.is_peer_allowed(&peer_image_path) {
        return Err(IpcError::PeerIdentityRejected {
            image_path: peer_image_path,
        });
    }

    let session_id = generate_session_id()?;
    let handshake = server_handshake(
        &mut transport,
        &authentication_token,
        Vec::new(),
        &session_id,
        config.handshake_timeout,
    )?;
    serve_session(
        &mut transport,
        &handshake.server_hello,
        config.heartbeat_timeout,
        handler,
    )
}

/// Runs one already-handshaken session over any [`Transport`].
///
/// This is the white-box seam: tests drive it with a scripted transport, so the
/// dispatch loop is covered without a real pipe.
///
/// # Errors
///
/// Returns an IPC error for malformed heartbeats, tool-shaped messages on the UI
/// channel, watchdog expiry, or transport failure.
pub fn serve_session<T, H>(
    transport: &mut T,
    server_hello: &ServerHello,
    heartbeat_timeout: Duration,
    handler: &mut H,
) -> IpcResult<()>
where
    T: Transport,
    H: UiCommandHandler,
{
    let mut events = NoEvents;
    serve_session_with_events(
        transport,
        server_hello,
        heartbeat_timeout,
        handler,
        &mut events,
    )
}

/// Runs one session that also pushes pending UI events (ADR-0057 D5).
///
/// # Errors
///
/// Same failure set as [`serve_session`], plus serialization errors from
/// [`push_events`].
pub fn serve_session_with_events<T, H, S>(
    transport: &mut T,
    server_hello: &ServerHello,
    heartbeat_timeout: Duration,
    handler: &mut H,
    events: &mut S,
) -> IpcResult<()>
where
    T: Transport,
    H: UiCommandHandler,
    S: UiEventSource,
{
    let now = unix_time_ms()?;
    let mut monitor = HeartbeatMonitor::new(heartbeat_timeout, now);
    send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
    let poll_interval = heartbeat_timeout / 3;

    loop {
        // 事件先于阻塞读取推送：慢客户端不能把事件堵在请求后面
        push_events(transport, events, heartbeat_timeout)?;
        let now = unix_time_ms()?;
        monitor.check(now)?;
        let wait = monitor.remaining(now).min(poll_interval);
        match transport.recv(wait) {
            Ok(WireMessage::Heartbeat(heartbeat)) => {
                if heartbeat.session_id != server_hello.session_id || !heartbeat.alive {
                    return Err(IpcError::UnexpectedMessage {
                        expected: "live Heartbeat for the negotiated UI session",
                        actual: "Heartbeat",
                    });
                }
                monitor.observe(now);
                send_heartbeat(transport, &server_hello.session_id, now, heartbeat_timeout)?;
            }
            Ok(WireMessage::UiRequest(request)) => {
                monitor.observe(now);
                let response = process_ui_request(handler, &request);
                transport.send(&WireMessage::UiResponse(response), heartbeat_timeout)?;
            }
            Ok(other) => {
                // 工具形状的 Request/Response 走错通道必须显式失败，不能忽略
                return Err(IpcError::UnexpectedMessage {
                    expected: "Heartbeat or UiRequest",
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

/// Applies one UI request and builds its matching response.
///
/// Parsing happens before the handler runs, so a malformed payload cannot cause
/// a side effect; every rejection names a stable `ErrorCode`.
#[must_use]
pub fn process_ui_request<H: UiCommandHandler>(
    handler: &mut H,
    request: &UiIpcRequest,
) -> UiIpcResponse {
    let correlation_id = request.correlation_id.clone();
    match dispatch_ui_command(handler, &request.command) {
        Ok(outcome) => match serde_json::to_value(&outcome) {
            Ok(value) => UiIpcResponse::outcome(correlation_id, value),
            Err(error) => UiIpcResponse::rejected(
                correlation_id,
                "Fatal",
                format!("outcome serialization failed: {error}"),
            ),
        },
        Err(error) => UiIpcResponse::rejected(
            correlation_id,
            format!("{:?}", error.error_code()),
            error.to_string(),
        ),
    }
}

fn send_heartbeat<T: Transport>(
    transport: &mut T,
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
        message: "system time does not fit in u64 milliseconds".to_owned(),
    })
}

/// Returns whether the given path is non-empty (config-validity helper).
#[must_use]
pub fn looks_like_image_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
}

#[cfg(test)]
#[path = "ui_server_tests.rs"]
mod tests;
