//! Loopback-only CDP target discovery.
//!
//! This module intentionally implements only the small HTTP surface needed for
//! `GET /json/list`. It never follows redirects, never uses ambient proxy
//! settings, and rejects every non-loopback destination before connecting.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

use super::json::{self, JsonValue};

const MAX_HTTP_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_TARGETS: usize = 64;

/// One page target returned by `/json/list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPage {
    pub url: String,
    pub title: String,
    pub websocket_url: String,
}

/// Validated loopback WebSocket endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketEndpoint {
    pub host: String,
    pub authority: String,
    pub path: String,
}

/// Resolve and connect to a loopback CDP HTTP endpoint.
pub fn connect_loopback(host: &str, port: u16, timeout: Duration) -> PlatformResult<TcpStream> {
    validate_timeout(timeout)?;
    let addresses = resolve_loopback(host, port)?;
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "CDP transport timeout exceeds the monotonic clock range",
        )
    })?;
    let mut last_error = None;
    let mut stream = None;
    for address in addresses {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&address, remaining) {
            Ok(connected) => {
                stream = Some(connected);
                break;
            }
            Err(error) => last_error = Some(format!("{address}: {error}")),
        }
    }
    let stream = stream.ok_or_else(|| {
        PlatformError::new(
            ErrorCode::Transient,
            format!(
                "failed to connect to loopback CDP endpoint: {}",
                last_error.unwrap_or_else(|| "timeout elapsed".to_string())
            ),
        )
    })?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| io_error(&error))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| io_error(&error))?;
    Ok(stream)
}

/// Discover page targets through `GET /json/list`.
pub fn discover_pages(host: &str, port: u16, timeout: Duration) -> PlatformResult<Vec<TargetPage>> {
    let mut stream = connect_loopback(host, port, timeout)?;
    let authority = authority_for(host, port)?;
    let request = format!(
        "GET /json/list HTTP/1.1\r\nHost: {authority}\r\nAccept: application/json\r\n\
         Connection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| io_error(&error))?;

    let mut response = Vec::new();
    stream
        .take((MAX_HTTP_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut response)
        .map_err(|error| io_error(&error))?;
    if response.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(protocol_error("CDP /json/list response exceeds 1 MiB"));
    }
    let body = parse_http_response(&response)?;
    parse_targets(body)
}

/// Parse and validate a page target's WebSocket URL.
pub fn parse_websocket_url(
    websocket_url: &str,
    expected_port: u16,
) -> PlatformResult<WebSocketEndpoint> {
    let remainder = websocket_url
        .strip_prefix("ws://")
        .ok_or_else(|| protocol_error("CDP WebSocket URL must use ws://"))?;
    let (authority, path) = remainder
        .split_once('/')
        .ok_or_else(|| protocol_error("CDP WebSocket URL is missing its page path"))?;
    if authority.contains('@') {
        return Err(protocol_error(
            "CDP WebSocket URL must not contain user information",
        ));
    }
    let (host, port) = parse_authority(authority)?;
    if port != expected_port {
        return Err(protocol_error(
            "CDP WebSocket URL port differs from the discovery port",
        ));
    }
    if !is_loopback_host(host) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            format!("CDP WebSocket host `{host}` is not loopback"),
        ));
    }
    let path = format!("/{path}");
    if !path.starts_with("/devtools/page/") {
        return Err(protocol_error(
            "CDP WebSocket URL is not a page-target endpoint",
        ));
    }
    Ok(WebSocketEndpoint {
        host: host.to_string(),
        authority: authority.to_string(),
        path,
    })
}

fn parse_http_response(response: &[u8]) -> PlatformResult<&[u8]> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| protocol_error("CDP HTTP response is missing its header terminator"))?;
    let headers = response
        .get(..header_end)
        .ok_or_else(|| protocol_error("CDP HTTP response header range is invalid"))?;
    let headers = std::str::from_utf8(headers)
        .map_err(|_| protocol_error("CDP HTTP response headers are not UTF-8"))?;
    let mut lines = headers.split("\r\n");
    let status = lines
        .next()
        .ok_or_else(|| protocol_error("CDP HTTP response is missing its status line"))?;
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(protocol_error(format!(
            "CDP HTTP discovery returned `{status}`"
        )));
    }

    let mut content_length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("transfer-encoding")
            && !value.trim().eq_ignore_ascii_case("identity")
        {
            return Err(protocol_error(
                "CDP HTTP discovery does not accept transfer encoding",
            ));
        }
        if name.trim().eq_ignore_ascii_case("content-length") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .map_err(|_| protocol_error("CDP HTTP Content-Length is invalid"))?;
            content_length = Some(parsed);
        }
    }

    let body = response
        .get(header_end + 4..)
        .ok_or_else(|| protocol_error("CDP HTTP response body range is invalid"))?;
    if let Some(expected) = content_length {
        let declared = body
            .get(..expected)
            .ok_or_else(|| protocol_error("CDP HTTP response body is shorter than declared"))?;
        return Ok(declared);
    }
    Ok(body)
}

fn parse_targets(body: &[u8]) -> PlatformResult<Vec<TargetPage>> {
    let value: JsonValue = json::parse(body).map_err(|error| {
        protocol_error(format!("CDP /json/list returned invalid JSON: {error}"))
    })?;
    let entries = value
        .as_array()
        .ok_or_else(|| protocol_error("CDP /json/list root must be an array"))?;
    if entries.len() > MAX_TARGETS {
        return Err(protocol_error("CDP /json/list returned too many targets"));
    }

    let mut pages = Vec::new();
    for entry in entries {
        if entry.get("type").and_then(JsonValue::as_str) != Some("page") {
            continue;
        }
        let url = required_string(entry, "url")?;
        let title = required_string(entry, "title")?;
        let websocket_url = required_string(entry, "webSocketDebuggerUrl")?;
        pages.push(TargetPage {
            url,
            title,
            websocket_url,
        });
    }
    Ok(pages)
}

fn required_string(value: &JsonValue, field: &str) -> PlatformResult<String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| protocol_error(format!("CDP page target is missing `{field}`")))
}

fn parse_authority(authority: &str) -> PlatformResult<(&str, u16)> {
    if let Some(remainder) = authority.strip_prefix('[') {
        let (host, port) = remainder
            .split_once("]:")
            .ok_or_else(|| protocol_error("invalid bracketed CDP WebSocket authority"))?;
        let port = port
            .parse::<u16>()
            .map_err(|_| protocol_error("invalid CDP WebSocket port"))?;
        return Ok((host, port));
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| protocol_error("CDP WebSocket authority is missing its port"))?;
    let port = port
        .parse::<u16>()
        .map_err(|_| protocol_error("invalid CDP WebSocket port"))?;
    Ok((host, port))
}

fn resolve_loopback(host: &str, port: u16) -> PlatformResult<Vec<SocketAddr>> {
    if !is_loopback_host(host) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            format!("CDP host `{host}` is not loopback"),
        ));
    }
    let authority = authority_for(host, port)?;
    let addresses = authority
        .to_socket_addrs()
        .map_err(|error| protocol_error(format!("failed to resolve loopback CDP host: {error}")))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(protocol_error(
            "loopback CDP host resolved to no loopback address",
        ));
    }
    if let Some(non_loopback) = addresses.iter().find(|address| !address.ip().is_loopback()) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            format!("loopback CDP host unexpectedly resolved to {non_loopback}"),
        ));
    }
    Ok(addresses)
}

fn authority_for(host: &str, port: u16) -> PlatformResult<String> {
    if host == "::1" {
        return Ok(format!("[::1]:{port}"));
    }
    if matches!(host, "127.0.0.1" | "localhost") {
        return Ok(format!("{host}:{port}"));
    }
    Err(PlatformError::new(
        ErrorCode::ToolInvalidArgs,
        format!("CDP host `{host}` is not loopback"),
    ))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn validate_timeout(timeout: Duration) -> PlatformResult<()> {
    if timeout.is_zero() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "CDP transport timeout must be greater than zero",
        ));
    }
    Ok(())
}

fn io_error(error: &std::io::Error) -> PlatformError {
    PlatformError::new(
        ErrorCode::Transient,
        format!("CDP HTTP transport failed: {error}"),
    )
}

fn protocol_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Transient, message)
}
