//! CDP loopback transport contract tests.
//!
//! These tests use a real local TCP listener and a minimal RFC 6455 server. No
//! browser, external network, credential, or production handler is involved.

use std::error::Error;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use assistant_platform_api::{
    CdpConnectOptions, CdpLoadCondition, CdpPoint, CdpProvider, ErrorCode, PlatformError,
};
use assistant_platform_windows::cdp::runtime::RuntimeCdpProvider;

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

struct Command {
    id: u64,
    method: String,
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    use std::task::{Context, Poll, Waker};
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

struct Fixture {
    port: u16,
    handle: thread::JoinHandle<TestResult>,
}

impl Fixture {
    fn start(server: impl FnOnce(TcpListener) -> TestResult + Send + 'static) -> io::Result<Self> {
        let (port_sender, port_receiver) = mpsc::channel();
        let handle = thread::spawn(move || {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            let port = listener.local_addr()?.port();
            port_sender
                .send(port)
                .map_err(|_| io::Error::other("failed to publish fixture port"))?;
            server(listener)
        });
        let port = port_receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| io::Error::other(format!("fixture did not start: {error}")))?;
        Ok(Self { port, handle })
    }

    fn finish(self) -> TestResult {
        self.handle
            .join()
            .map_err(|_| -> Box<dyn Error + Send + Sync> {
                Box::new(io::Error::other("fixture thread panicked"))
            })?
    }
}

#[test]
fn test_cdp_loopback_transport_navigates_and_reads_dom() -> TestResult {
    let fixture = Fixture::start(|listener| serve_positive_fixture(&listener))?;
    let options = options(fixture.port, "Fixture Page", 5_000)?;
    let provider = RuntimeCdpProvider::new();
    let session = block_on(provider.connect(&options)).map_err(|error| platform_error(&error))?;
    assert_eq!(session.target_identity().marker(), Some("task-109-fixture"));

    block_on(provider.navigate(
        &session.target(),
        "http://127.0.0.1:8765/page-2.html",
        &CdpLoadCondition::NetworkIdle { quiet_ms: 100 },
    ))
    .map_err(|error| platform_error(&error))?;
    let snapshot = block_on(provider.evaluate_dom(
        &session.target(),
        "document.querySelector('main').outerHTML",
    ))
    .map_err(|error| platform_error(&error))?;
    assert_eq!(snapshot.target_url(), "http://127.0.0.1:8765/page-2.html");
    assert_eq!(snapshot.target_title(), "Fixture Page");
    assert_eq!(snapshot.target_marker(), Some("task-109-fixture"));
    assert_eq!(snapshot.dom(), "<main>fixture rows</main>");

    assert_error_code(
        block_on(provider.click(&session.target(), &CdpPoint::new(1, 1))),
        ErrorCode::CapabilityMissing,
    )?;
    assert_error_code(
        block_on(provider.capture_screenshot(&session.target())),
        ErrorCode::CapabilityMissing,
    )?;
    assert_error_code(
        block_on(provider.set_download_directory(&session.target(), "downloads")),
        ErrorCode::CapabilityMissing,
    )?;
    assert_error_code(
        block_on(provider.close(&session.target())),
        ErrorCode::CapabilityMissing,
    )?;
    fixture.finish()
}

#[test]
fn test_cdp_transport_rejects_non_allowlisted_loopback_name() -> TestResult {
    let options = CdpConnectOptions::new(
        "127.0.0.2",
        9222,
        "http://127.0.0.1:8765/page-1.html",
        None,
        None,
        500,
    )?;
    let provider = RuntimeCdpProvider::new();
    assert_error_code(
        block_on(provider.connect(&options)),
        ErrorCode::ToolInvalidArgs,
    )
}

#[test]
fn test_cdp_transport_rejects_ambiguous_discovery() -> TestResult {
    let fixture = Fixture::start(|listener| {
        let mut stream = accept_until(&listener, Duration::from_secs(2))?;
        read_headers(&mut stream)?;
        let port = listener.local_addr()?.port();
        let body = format!(
            "[{{\"type\":\"page\",\"url\":\"http://127.0.0.1:8765/page-1.html\",\
             \"title\":\"Fixture Page\",\"webSocketDebuggerUrl\":\
             \"ws://127.0.0.1:{port}/devtools/page/a\"}},\
             {{\"type\":\"page\",\"url\":\"http://127.0.0.1:8765/page-1.html\",\
             \"title\":\"Fixture Page\",\"webSocketDebuggerUrl\":\
             \"ws://127.0.0.1:{port}/devtools/page/b\"}}]"
        );
        write_http_json(&mut stream, 200, &body)
    })?;
    let options = options(fixture.port, "Fixture Page", 500)?;
    let provider = RuntimeCdpProvider::new();
    assert_error_code(
        block_on(provider.connect(&options)),
        ErrorCode::TargetAmbiguous,
    )?;
    fixture.finish()
}

#[test]
fn test_cdp_transport_rejects_bad_websocket_accept() -> TestResult {
    let fixture = Fixture::start(|listener| {
        serve_discovery(&listener)?;
        let mut stream = accept_until(&listener, Duration::from_secs(2))?;
        let request = read_headers(&mut stream)?;
        let key = header_value(&request, "sec-websocket-key")?;
        let response = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\
                        Connection: Upgrade\r\nSec-WebSocket-Accept: invalid\r\n\r\n";
        let _ = key;
        stream.write_all(response.as_bytes())?;
        Ok(())
    })?;
    let options = options(fixture.port, "Fixture Page", 500)?;
    let provider = RuntimeCdpProvider::new();
    let session = block_on(provider.connect(&options)).map_err(|error| platform_error(&error))?;
    assert_error_code(
        block_on(provider.navigate(
            &session.target(),
            "http://127.0.0.1:8765/page-1.html",
            &CdpLoadCondition::Load,
        )),
        ErrorCode::Transient,
    )?;
    fixture.finish()
}

#[test]
fn test_cdp_transport_accepts_101_reason_phrase_from_edge() -> TestResult {
    let fixture = Fixture::start(|listener| {
        serve_discovery(&listener)?;
        let mut stream = accept_until(&listener, Duration::from_secs(2))?;
        accept_websocket_with_status_line(
            &mut stream,
            "HTTP/1.1 101 WebSocket Protocol Handshake",
        )?;
        serve_navigation(&mut stream)
    })?;
    let options = options(fixture.port, "Fixture Page", 5_000)?;
    let provider = RuntimeCdpProvider::new();
    let session = block_on(provider.connect(&options)).map_err(|error| platform_error(&error))?;
    block_on(provider.navigate(
        &session.target(),
        "http://127.0.0.1:8765/page-2.html",
        &CdpLoadCondition::NetworkIdle { quiet_ms: 100 },
    ))
    .map_err(|error| platform_error(&error))?;
    fixture.finish()
}

#[test]
fn test_cdp_transport_rejects_non_101_websocket_status() -> TestResult {
    let fixture = Fixture::start(|listener| {
        serve_discovery(&listener)?;
        let mut stream = accept_until(&listener, Duration::from_secs(2))?;
        accept_websocket_with_status_line(&mut stream, "HTTP/1.1 200 OK")
    })?;
    let options = options(fixture.port, "Fixture Page", 500)?;
    let provider = RuntimeCdpProvider::new();
    let session = block_on(provider.connect(&options)).map_err(|error| platform_error(&error))?;
    assert_error_code(
        block_on(provider.navigate(
            &session.target(),
            "http://127.0.0.1:8765/page-2.html",
            &CdpLoadCondition::Load,
        )),
        ErrorCode::Transient,
    )?;
    fixture.finish()
}

#[test]
fn test_cdp_transport_rejects_jsonrpc_error() -> TestResult {
    let fixture = Fixture::start(|listener| {
        serve_discovery(&listener)?;
        let mut stream = accept_until(&listener, Duration::from_secs(2))?;
        accept_websocket(&mut stream)?;
        let command = read_command(&mut stream)?;
        write_json(
            &mut stream,
            &format!(
                "{{\"id\":{},\"error\":{{\"code\":-32602,\
                 \"message\":\"fixture rejected evaluation\"}}}}",
                command.id
            ),
        )
    })?;
    let options = options(fixture.port, "Fixture Page", 500)?;
    let provider = RuntimeCdpProvider::new();
    let session = block_on(provider.connect(&options)).map_err(|error| platform_error(&error))?;
    assert_error_code(
        block_on(provider.evaluate_dom(&session.target(), "document.body")),
        ErrorCode::Transient,
    )?;
    fixture.finish()
}

#[test]
fn test_cdp_transport_rejects_incomplete_network_idle() -> TestResult {
    let fixture = Fixture::start(|listener| {
        serve_discovery(&listener)?;
        let mut stream = accept_until(&listener, Duration::from_secs(2))?;
        accept_websocket(&mut stream)?;
        let _ = read_command(&mut stream)?;
        write_json(&mut stream, "{\"id\":1,\"result\":{}}")?;
        let _ = read_command(&mut stream)?;
        write_json(&mut stream, "{\"id\":2,\"result\":{}}")?;
        let _ = read_command(&mut stream)?;
        write_json(
            &mut stream,
            "{\"id\":3,\"result\":{\"frameId\":\"fixture\",\"loaderId\":\"loader\"}}",
        )?;
        write_json(
            &mut stream,
            "{\"method\":\"Page.loadEventFired\",\"params\":{}}",
        )?;
        write_json(
            &mut stream,
            "{\"method\":\"Network.requestWillBeSent\",\
             \"params\":{\"requestId\":\"never-finishes\"}}",
        )?;
        thread::sleep(Duration::from_millis(500));
        Ok(())
    })?;
    let options = options(fixture.port, "Fixture Page", 250)?;
    let provider = RuntimeCdpProvider::new();
    let session = block_on(provider.connect(&options)).map_err(|error| platform_error(&error))?;
    assert_error_code(
        block_on(provider.navigate(
            &session.target(),
            "http://127.0.0.1:8765/page-1.html",
            &CdpLoadCondition::NetworkIdle { quiet_ms: 50 },
        )),
        ErrorCode::TargetUnresponsive,
    )?;
    fixture.finish()
}

fn options(
    port: u16,
    title: &str,
    timeout_ms: u64,
) -> Result<CdpConnectOptions, assistant_platform_api::PlatformError> {
    CdpConnectOptions::new(
        "127.0.0.1",
        port,
        "http://127.0.0.1:8765/page-1.html",
        Some(title.to_string()),
        Some("task-109-fixture".to_string()),
        timeout_ms,
    )
}

fn platform_error(error: &PlatformError) -> io::Error {
    io::Error::other(format!(
        "platform error {:?}: {}",
        error.code(),
        error.message()
    ))
}

fn assert_error_code<T>(result: Result<T, PlatformError>, expected: ErrorCode) -> TestResult {
    match result {
        Ok(_) => Err(Box::new(io::Error::other(format!(
            "expected {expected:?}, received success"
        )))),
        Err(error) if error.code() == expected => Ok(()),
        Err(error) => Err(Box::new(io::Error::other(format!(
            "expected {expected:?}, received {:?}: {}",
            error.code(),
            error.message()
        )))),
    }
}

fn serve_positive_fixture(listener: &TcpListener) -> TestResult {
    serve_discovery(listener)?;
    let mut navigate = accept_until(listener, Duration::from_secs(2))?;
    accept_websocket(&mut navigate)?;
    serve_navigation(&mut navigate)?;
    let mut evaluate = accept_until(listener, Duration::from_secs(2))?;
    accept_websocket(&mut evaluate)?;
    serve_evaluation(&mut evaluate)?;
    Ok(())
}

fn serve_discovery(listener: &TcpListener) -> TestResult {
    let mut stream = accept_until(listener, Duration::from_secs(2))?;
    let request = read_headers(&mut stream)?;
    if !request.starts_with("GET /json/list HTTP/1.1") {
        return Err(Box::new(io::Error::other("fixture expected /json/list")));
    }
    let port = listener.local_addr()?.port();
    let body = format!(
        "[{{\"type\":\"page\",\"url\":\"http://127.0.0.1:8765/page-1.html\",\
         \"title\":\"Fixture Page\",\"webSocketDebuggerUrl\":\
         \"ws://127.0.0.1:{port}/devtools/page/fixture\"}}]"
    );
    write_http_json(&mut stream, 200, &body)
}

fn serve_navigation(stream: &mut TcpStream) -> TestResult {
    let mut command_number = 0_u8;
    loop {
        let command = read_command(stream)?;
        let method = command.method.as_str();
        let id = command.id;
        command_number = command_number.saturating_add(1);
        match method {
            "Page.enable" | "Network.enable" => {
                write_json(stream, &format!("{{\"id\":{id},\"result\":{{}}}}"))?;
            }
            "Page.navigate" => {
                write_json(
                    stream,
                    &format!(
                        "{{\"id\":{id},\"result\":\
                         {{\"frameId\":\"fixture\",\"loaderId\":\"loader\"}}}}"
                    ),
                )?;
                write_json(stream, "{\"method\":\"Page.loadEventFired\",\"params\":{}}")?;
                write_json(
                    stream,
                    "{\"method\":\"Network.requestWillBeSent\",\
                     \"params\":{\"requestId\":\"request-1\"}}",
                )?;
                write_json(
                    stream,
                    "{\"method\":\"Network.loadingFinished\",\
                     \"params\":{\"requestId\":\"request-1\"}}",
                )?;
                thread::sleep(Duration::from_secs(1));
                return Ok(());
            }
            _ => {
                return Err(Box::new(io::Error::other(format!(
                    "fixture received unexpected method {method} after {command_number} commands"
                ))));
            }
        }
    }
}

fn serve_evaluation(stream: &mut TcpStream) -> TestResult {
    let command = read_command(stream)?;
    if command.method != "Runtime.evaluate" {
        return Err(Box::new(io::Error::other(
            "fixture expected Runtime.evaluate",
        )));
    }
    let id = command.id;
    let snapshot = "{\\\"url\\\":\\\"http://127.0.0.1:8765/page-2.html\\\",\
                    \\\"title\\\":\\\"Fixture Page\\\",\
                    \\\"dom\\\":\\\"<main>fixture rows</main>\\\"}";
    write_json(
        stream,
        &format!(
            "{{\"id\":{id},\"result\":{{\"result\":\
             {{\"type\":\"string\",\"value\":\"{snapshot}\"}}}}}}"
        ),
    )?;
    thread::sleep(Duration::from_secs(1));
    Ok(())
}

fn accept_until(listener: &TcpListener, timeout: Duration) -> io::Result<TcpStream> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(timeout))?;
                stream.set_write_timeout(Some(timeout))?;
                return Ok(stream);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "fixture accept timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
}

fn read_headers(stream: &mut TcpStream) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    while bytes.len() < 16 * 1024 {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return String::from_utf8(bytes)
                .map_err(|error| io::Error::other(format!("fixture header UTF-8: {error}")));
        }
    }
    Err(io::Error::other("fixture header exceeded 16 KiB"))
}

fn header_value(request: &str, wanted: &str) -> io::Result<String> {
    for line in request.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(wanted) {
            return Ok(value.trim().to_string());
        }
    }
    Err(io::Error::other(format!(
        "fixture header `{wanted}` is missing"
    )))
}

fn write_http_json(stream: &mut TcpStream, status: u16, body: &str) -> TestResult {
    let response = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn accept_websocket(stream: &mut TcpStream) -> TestResult {
    accept_websocket_with_status_line(stream, "HTTP/1.1 101 Switching Protocols")
}

fn accept_websocket_with_status_line(stream: &mut TcpStream, status_line: &str) -> TestResult {
    let request = read_headers(stream)?;
    let key = header_value(&request, "sec-websocket-key")?;
    let accept = websocket_accept(&key);
    let response = format!(
        "{status_line}\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn read_command(stream: &mut TcpStream) -> io::Result<Command> {
    let (opcode, payload) = read_frame(stream)?;
    if opcode != 0x1 {
        return Err(io::Error::other(format!(
            "fixture expected text frame, got opcode {opcode}"
        )));
    }
    let text = std::str::from_utf8(&payload)
        .map_err(|error| io::Error::other(format!("fixture client UTF-8: {error}")))?;
    Ok(Command {
        id: numeric_field(text, "id")?,
        method: string_field(text, "method")?,
    })
}

fn read_frame(stream: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header)?;
    let opcode = header[0] & 0x0f;
    let masked = header[1] & 0x80 != 0;
    if !masked {
        return Err(io::Error::other("fixture client frame was not masked"));
    }
    let mut length = u64::from(header[1] & 0x7f);
    if length == 126 {
        let mut bytes = [0_u8; 2];
        stream.read_exact(&mut bytes)?;
        length = u64::from(u16::from_be_bytes(bytes));
    } else if length == 127 {
        let mut bytes = [0_u8; 8];
        stream.read_exact(&mut bytes)?;
        length = u64::from_be_bytes(bytes);
    }
    let length = usize::try_from(length)
        .map_err(|_| io::Error::other("fixture frame length exceeds usize"))?;
    if length > 4 * 1024 * 1024 {
        return Err(io::Error::other("fixture frame exceeds 4 MiB"));
    }
    let mut mask = [0_u8; 4];
    stream.read_exact(&mut mask)?;
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload)?;
    for (index, byte) in payload.iter_mut().enumerate() {
        if let Some(mask_byte) = mask.get(index % 4) {
            *byte ^= mask_byte;
        }
    }
    Ok((opcode, payload))
}

fn numeric_field(text: &str, field: &str) -> io::Result<u64> {
    let marker = format!("\"{field}\":");
    let start = text
        .find(&marker)
        .map(|position| position + marker.len())
        .ok_or_else(|| io::Error::other(format!("fixture command lacks `{field}`")))?;
    let digits = text
        .get(start..)
        .ok_or_else(|| io::Error::other("fixture numeric field range is invalid"))?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits
        .parse::<u64>()
        .map_err(|_| io::Error::other(format!("fixture `{field}` is not numeric")))
}

fn string_field(text: &str, field: &str) -> io::Result<String> {
    let marker = format!("\"{field}\":\"");
    let start = text
        .find(&marker)
        .map(|position| position + marker.len())
        .ok_or_else(|| io::Error::other(format!("fixture command lacks string `{field}`")))?;
    let remainder = text
        .get(start..)
        .ok_or_else(|| io::Error::other("fixture string field range is invalid"))?;
    let mut output = String::new();
    let mut escaped = false;
    for character in remainder.chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Ok(output);
        } else {
            output.push(character);
        }
    }
    Err(io::Error::other(format!(
        "fixture string `{field}` is unterminated"
    )))
}

fn write_json(stream: &mut TcpStream, payload: &str) -> TestResult {
    let payload = payload.as_bytes();
    let mut frame = Vec::with_capacity(payload.len() + 10);
    frame.push(0x81);
    if payload.len() <= 125 {
        frame.push(u8::try_from(payload.len())?);
    } else if payload.len() <= 65_535 {
        frame.push(126);
        frame.extend_from_slice(&u16::try_from(payload.len())?.to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&u64::try_from(payload.len())?.to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    Ok(())
}

fn websocket_accept(key: &str) -> String {
    let mut input = key.as_bytes().to_vec();
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64_encode(&sha1(&input))
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
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
    let mut padded = input.to_vec();
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
