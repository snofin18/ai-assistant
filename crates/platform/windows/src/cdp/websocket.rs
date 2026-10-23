//! Minimal RFC 6455 client for loopback CDP sessions.
//!
//! The implementation is deliberately bounded: one text message at a time,
//! masked client frames, unmasked server frames, 4 MiB message ceiling and
//! deadline-based reads. It is not a general-purpose WebSocket client.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};
#[cfg(windows)]
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};

use super::http::{connect_loopback, parse_websocket_url};
use super::json::{self, JsonValue};

const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// A connected loopback CDP WebSocket.
#[derive(Debug)]
pub struct CdpWebSocket {
    stream: TcpStream,
    timeout: Duration,
}

impl CdpWebSocket {
    /// Connect and complete a validated RFC 6455 handshake.
    pub(crate) fn connect(
        websocket_url: &str,
        expected_port: u16,
        timeout: Duration,
    ) -> PlatformResult<Self> {
        let endpoint = parse_websocket_url(websocket_url, expected_port)?;
        let mut stream = connect_loopback(&endpoint.host, expected_port, timeout)?;
        let key = websocket_key()?;
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n",
            endpoint.path, endpoint.authority
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| transport_error(&error))?;
        read_handshake(&mut stream, &key)?;
        Ok(Self { stream, timeout })
    }

    /// Send one text message using RFC 6455 masking.
    pub(crate) fn send_json(&mut self, value: &JsonValue) -> PlatformResult<()> {
        let payload = json::encode(value).into_bytes();
        if payload.len() > MAX_MESSAGE_BYTES {
            return Err(protocol_error("CDP command exceeds 4 MiB"));
        }
        self.send_frame(0x1, &payload)
    }

    /// Receive one complete JSON text message, or `None` at the deadline.
    pub(crate) fn receive_json_until(
        &mut self,
        deadline: Instant,
    ) -> PlatformResult<Option<JsonValue>> {
        let Some(payload) = self.receive_message_until(deadline)? else {
            return Ok(None);
        };
        let text = std::str::from_utf8(&payload)
            .map_err(|_| protocol_error("CDP WebSocket text frame is not UTF-8"))?;
        let value = json::parse(text.as_bytes())?;
        Ok(Some(value))
    }

    /// Send a best-effort RFC 6455 close frame.
    pub(crate) fn close(mut self) -> PlatformResult<()> {
        self.send_frame(0x8, &[0x03, 0xe8])
    }

    fn receive_message_until(&mut self, deadline: Instant) -> PlatformResult<Option<Vec<u8>>> {
        let mut assembled = Vec::new();
        let mut current_opcode = None;
        loop {
            let Some(frame) = self.read_frame_until(deadline)? else {
                return Ok(None);
            };
            match frame.opcode {
                0x0 => {
                    if current_opcode.is_none() {
                        return Err(protocol_error(
                            "CDP WebSocket continuation arrived without a data frame",
                        ));
                    }
                    append_bounded(&mut assembled, &frame.payload)?;
                }
                0x1 => {
                    if current_opcode.is_some() {
                        return Err(protocol_error(
                            "CDP WebSocket started a new data frame before continuation completed",
                        ));
                    }
                    current_opcode = Some(frame.opcode);
                    append_bounded(&mut assembled, &frame.payload)?;
                }
                0x8 => {
                    return Err(PlatformError::new(
                        ErrorCode::Transient,
                        "CDP WebSocket closed before a response was received",
                    ));
                }
                0x9 => self.send_frame(0xA, &frame.payload)?,
                0xA => {}
                _ => {
                    return Err(protocol_error(format!(
                        "CDP WebSocket used unsupported opcode {}",
                        frame.opcode
                    )));
                }
            }
            if frame.fin
                && matches!(frame.opcode, 0x0..=0x1)
                && matches!(current_opcode, Some(0x1 | 0x2))
            {
                return Ok(Some(assembled));
            }
        }
    }

    fn read_frame_until(&mut self, deadline: Instant) -> PlatformResult<Option<WebSocketFrame>> {
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        let timeout = deadline.saturating_duration_since(now).min(self.timeout);
        self.stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| transport_error(&error))?;

        let mut header = [0_u8; 2];
        let Some(read) = read_exact_or_deadline(&mut self.stream, &mut header, deadline)? else {
            return Ok(None);
        };
        if !read {
            return Ok(None);
        }
        let first = *header
            .first()
            .ok_or_else(|| protocol_error("CDP WebSocket header is missing"))?;
        let second = *header
            .get(1)
            .ok_or_else(|| protocol_error("CDP WebSocket header is incomplete"))?;
        let fin = first & 0x80 != 0;
        if first & 0x70 != 0 {
            return Err(protocol_error(
                "CDP WebSocket frame uses unsupported reserved bits",
            ));
        }
        let opcode = first & 0x0f;
        if matches!(opcode, 0x8..=0xA) && !fin {
            return Err(protocol_error("CDP WebSocket control frame must set FIN"));
        }
        if second & 0x80 != 0 {
            return Err(protocol_error(
                "CDP WebSocket server frames must not be masked",
            ));
        }
        let length_code = second & 0x7f;
        let payload_length = match length_code {
            126 => {
                let mut length = [0_u8; 2];
                require_exact(&mut self.stream, &mut length, deadline)?;
                u64::from(u16::from_be_bytes(length))
            }
            127 => {
                let mut length = [0_u8; 8];
                require_exact(&mut self.stream, &mut length, deadline)?;
                let value = u64::from_be_bytes(length);
                if value & (1_u64 << 63) != 0 {
                    return Err(protocol_error(
                        "CDP WebSocket frame uses an invalid 64-bit length",
                    ));
                }
                value
            }
            value => u64::from(value),
        };
        let payload_length = usize::try_from(payload_length)
            .map_err(|_| protocol_error("CDP WebSocket frame length does not fit usize"))?;
        if payload_length > MAX_MESSAGE_BYTES {
            return Err(protocol_error("CDP WebSocket frame exceeds 4 MiB"));
        }
        if matches!(opcode, 0x8..=0xA) && payload_length > 125 {
            return Err(protocol_error(
                "CDP WebSocket control frame exceeds 125 bytes",
            ));
        }
        let mut payload = vec![0_u8; payload_length];
        require_exact(&mut self.stream, &mut payload, deadline)?;
        Ok(Some(WebSocketFrame {
            fin,
            opcode,
            payload,
        }))
    }

    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> PlatformResult<()> {
        if payload.len() > MAX_MESSAGE_BYTES {
            return Err(protocol_error("CDP WebSocket payload exceeds 4 MiB"));
        }
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x80 | opcode);
        match payload.len() {
            0..=125 => {
                let length = u8::try_from(payload.len())
                    .map_err(|_| protocol_error("CDP WebSocket length overflow"))?;
                frame.push(0x80 | length);
            }
            126..=65_535 => {
                frame.push(0x80 | 0x7e);
                let length = u16::try_from(payload.len())
                    .map_err(|_| protocol_error("CDP WebSocket length overflow"))?;
                frame.extend_from_slice(&length.to_be_bytes());
            }
            _ => {
                frame.push(0x80 | 127);
                let length = u64::try_from(payload.len())
                    .map_err(|_| protocol_error("CDP WebSocket length overflow"))?;
                frame.extend_from_slice(&length.to_be_bytes());
            }
        }
        let mask = random_mask()?;
        frame.extend_from_slice(&mask);
        for (index, byte) in payload.iter().enumerate() {
            let mask_byte = mask
                .get(index % 4)
                .ok_or_else(|| protocol_error("CDP WebSocket mask is incomplete"))?;
            frame.push(byte ^ mask_byte);
        }
        self.stream
            .write_all(&frame)
            .map_err(|error| transport_error(&error))
    }
}

#[derive(Debug)]
struct WebSocketFrame {
    fin: bool,
    opcode: u8,
    payload: Vec<u8>,
}

fn read_handshake(stream: &mut TcpStream, key: &str) -> PlatformResult<()> {
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while response.len() <= MAX_HANDSHAKE_BYTES {
        stream
            .read_exact(&mut byte)
            .map_err(|error| transport_error(&error))?;
        response.push(byte[0]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    if response.len() > MAX_HANDSHAKE_BYTES {
        return Err(protocol_error("CDP WebSocket handshake exceeds 16 KiB"));
    }
    let headers = std::str::from_utf8(&response)
        .map_err(|_| protocol_error("CDP WebSocket handshake is not UTF-8"))?;
    let mut lines = headers.split("\r\n");
    let status = lines
        .next()
        .ok_or_else(|| protocol_error("CDP WebSocket handshake is missing its status line"))?;
    if status != "HTTP/1.1 101 Switching Protocols" {
        return Err(protocol_error(format!(
            "CDP WebSocket upgrade returned `{status}`"
        )));
    }
    let mut upgrade = false;
    let mut connection = false;
    let mut accept = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if name.eq_ignore_ascii_case("upgrade") {
            upgrade = value.eq_ignore_ascii_case("websocket");
        } else if name.eq_ignore_ascii_case("connection") {
            connection = value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
        } else if name.eq_ignore_ascii_case("sec-websocket-accept") {
            accept = Some(value.to_string());
        }
    }
    if !upgrade || !connection {
        return Err(protocol_error(
            "CDP WebSocket handshake is missing Upgrade/Connection",
        ));
    }
    let expected = websocket_accept(key);
    if accept.as_deref() != Some(expected.as_str()) {
        return Err(protocol_error(
            "CDP WebSocket handshake has an invalid accept digest",
        ));
    }
    Ok(())
}

fn append_bounded(buffer: &mut Vec<u8>, fragment: &[u8]) -> PlatformResult<()> {
    let next_length = buffer
        .len()
        .checked_add(fragment.len())
        .ok_or_else(|| protocol_error("CDP WebSocket message length overflow"))?;
    if next_length > MAX_MESSAGE_BYTES {
        return Err(protocol_error("CDP WebSocket message exceeds 4 MiB"));
    }
    buffer.extend_from_slice(fragment);
    Ok(())
}

fn require_exact(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> PlatformResult<()> {
    match read_exact_or_deadline(stream, buffer, deadline)? {
        Some(true) => Ok(()),
        Some(false) | None => Err(protocol_error(
            "CDP WebSocket ended before its frame was complete",
        )),
    }
}

fn read_exact_or_deadline(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> PlatformResult<Option<bool>> {
    let mut offset = 0;
    while offset < buffer.len() {
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        stream
            .set_read_timeout(Some(deadline.saturating_duration_since(now)))
            .map_err(|error| transport_error(&error))?;
        let remaining = buffer
            .get_mut(offset..)
            .ok_or_else(|| protocol_error("CDP WebSocket read range is invalid"))?;
        match stream.read(remaining) {
            Ok(0) => return Ok(Some(false)),
            Ok(read) => offset += read,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) && offset == 0 =>
            {
                return Ok(None);
            }
            Err(error) => return Err(transport_error(&error)),
        }
    }
    Ok(Some(true))
}

fn websocket_key() -> PlatformResult<String> {
    let mut key = [0_u8; 16];
    fill_random(&mut key)?;
    Ok(base64_encode(&key))
}

fn random_mask() -> PlatformResult<[u8; 4]> {
    let mut mask = [0_u8; 4];
    fill_random(&mut mask)?;
    Ok(mask)
}

#[cfg(windows)]
fn fill_random(buffer: &mut [u8]) -> PlatformResult<()> {
    // SAFETY: the mutable slice remains valid for the duration of the call and
    // the system-preferred RNG flag requires no algorithm handle.
    let status = unsafe { BCryptGenRandom(None, buffer, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status.0 >= 0 {
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::Fatal,
            format!("BCryptGenRandom failed with NTSTATUS {:#x}", status.0),
        ))
    }
}

#[cfg(not(windows))]
fn fill_random(buffer: &mut [u8]) -> PlatformResult<()> {
    use std::fs::File;

    let mut source = File::open("/dev/urandom").map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!("failed to open the system random source: {error}"),
        )
    })?;
    source.read_exact(buffer).map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!("failed to read the system random source: {error}"),
        )
    })
}

fn websocket_accept(key: &str) -> String {
    let mut input = Vec::with_capacity(key.len() + WEBSOCKET_GUID.len());
    input.extend_from_slice(key.as_bytes());
    input.extend_from_slice(WEBSOCKET_GUID.as_bytes());
    base64_encode(&sha1(&input))
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk.first().copied().unwrap_or(0);
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let triple = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
        for (quartet_index, shift) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            let index = ((triple >> shift) & 0x3f) as usize;
            if quartet_index < chunk.len() + 1
                && let Some(value) = TABLE.get(index)
            {
                output.push(char::from(*value));
            } else if quartet_index > chunk.len() {
                output.push('=');
            }
        }
    }
    output
}

fn sha1(input: &[u8]) -> [u8; 20] {
    let bit_length = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 72);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89_u32,
        0x98ba_dcfe_u32,
        0x1032_5476_u32,
        0xc3d2_e1f0_u32,
    ];
    for block in padded.as_chunks::<64>().0 {
        sha1_compress_block(&mut state, block);
    }

    let mut digest = [0_u8; 20];
    for (index, word) in state.into_iter().enumerate() {
        let start = index * 4;
        if let Some(slot) = digest.get_mut(start..start + 4) {
            slot.copy_from_slice(&word.to_be_bytes());
        }
    }
    digest
}

fn sha1_compress_block(state: &mut [u32; 5], block: &[u8]) {
    let mut schedule = [0_u32; 80];
    for index in 0..16 {
        let start = index * 4;
        let Some(bytes) = block.get(start..start + 4) else {
            continue;
        };
        let Ok(word_bytes) = <[u8; 4]>::try_from(bytes) else {
            continue;
        };
        if let Some(slot) = schedule.get_mut(index) {
            *slot = u32::from_be_bytes(word_bytes);
        }
    }
    for index in 16_usize..80 {
        let word = schedule.get(index - 3).copied().unwrap_or(0)
            ^ schedule.get(index - 8).copied().unwrap_or(0)
            ^ schedule.get(index - 14).copied().unwrap_or(0)
            ^ schedule.get(index - 16).copied().unwrap_or(0);
        if let Some(slot) = schedule.get_mut(index) {
            *slot = word.rotate_left(1);
        }
    }

    let [
        mut working_a,
        mut working_b,
        mut working_c,
        mut working_d,
        mut working_e,
    ] = *state;
    for (index, word) in schedule.iter().enumerate() {
        let (function, constant) = match index {
            0..=19 => (
                (working_b & working_c) | ((!working_b) & working_d),
                0x5a82_7999,
            ),
            20..=39 => (working_b ^ working_c ^ working_d, 0x6ed9_eba1),
            40..=59 => (
                (working_b & working_c) | (working_b & working_d) | (working_c & working_d),
                0x8f1b_bcdc,
            ),
            _ => (working_b ^ working_c ^ working_d, 0xca62_c1d6),
        };
        let temporary = working_a
            .rotate_left(5)
            .wrapping_add(function)
            .wrapping_add(working_e)
            .wrapping_add(constant)
            .wrapping_add(*word);
        working_e = working_d;
        working_d = working_c;
        working_c = working_b.rotate_left(30);
        working_b = working_a;
        working_a = temporary;
    }
    let [hash_a, hash_b, hash_c, hash_d, hash_e] = *state;
    *state = [
        hash_a.wrapping_add(working_a),
        hash_b.wrapping_add(working_b),
        hash_c.wrapping_add(working_c),
        hash_d.wrapping_add(working_d),
        hash_e.wrapping_add(working_e),
    ];
}

fn transport_error(error: &std::io::Error) -> PlatformError {
    PlatformError::new(
        ErrorCode::Transient,
        format!("CDP WebSocket transport failed: {error}"),
    )
}

fn protocol_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Transient, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_accept_matches_rfc6455_example() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
