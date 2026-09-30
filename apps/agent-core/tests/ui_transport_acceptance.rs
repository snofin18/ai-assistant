//! Windows end-to-end acceptance for the UI↔Core transport (ADR-0057).
//!
//! This is the real-pipe proof for TASK-213: a Core-side session loop and a UI
//! client talk over an actual `NamedPipe` — no scripted transport, no mocks. It
//! asserts three properties the card's `DoD` names explicitly:
//!
//! 1. a UI request reaches the handler and comes back with the same
//!    `correlation_id`;
//! 2. a pending UI event is pushed **before** the response on the same pipe;
//! 3. when the client goes away, the session ends with an explicit disconnect
//!    instead of hanging.

#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use assistant_agent_core::{
    UiCommand, UiCommandError, UiCommandHandler, UiCommandOutcome, UiEvent, UiEventSource,
    serve_session_with_events,
};
use assistant_ipc::{
    Heartbeat, HeartbeatMonitor, IpcError, NamedPipeTransport, ServerHello, Transport,
    UiIpcRequest, UiIpcResult, WireMessage, client_handshake, generate_session_id,
    server_handshake,
};
use assistant_protocol::serde_json::json;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(2);

/// Counts how many commands reached the handler.
#[derive(Default)]
struct CountingHandler {
    handled: Arc<AtomicUsize>,
}

impl UiCommandHandler for CountingHandler {
    fn handle(&mut self, command: UiCommand) -> Result<UiCommandOutcome, UiCommandError> {
        self.handled.fetch_add(1, Ordering::SeqCst);
        match command {
            UiCommand::SubmitIntent { intent_id, .. } => {
                Ok(UiCommandOutcome::IntentAccepted { intent_id })
            }
            other => Ok(UiCommandOutcome::IntentAccepted {
                intent_id: format!("{other:?}"),
            }),
        }
    }
}

/// Emits exactly one event, then goes quiet.
struct OneEventSource {
    pending: Vec<UiEvent>,
}

impl UiEventSource for OneEventSource {
    fn drain(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.pending)
    }
}

fn unique_pipe_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("assistant-ui-acceptance-{}-{nanos}", std::process::id())
}

fn submit_intent() -> UiIpcRequest {
    UiIpcRequest::new(
        "c_acceptance",
        json!({
            "version": "1.0",
            "command": { "kind": "submit_intent", "intent_id": "i_1", "goal": "replace and save" }
        }),
    )
}

/// 读到响应为止，返回沿途的消息 kind 序列（同时断言响应内容与事件顺序）。
fn read_until_response(
    transport: &mut NamedPipeTransport,
) -> Result<Vec<&'static str>, Box<dyn std::error::Error>> {
    let mut kinds = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let message = transport.recv(Duration::from_millis(500))?;
        kinds.push(message.kind());
        if let WireMessage::UiResponse(response) = &message {
            assert_eq!(response.correlation_id, "c_acceptance");
            assert!(matches!(response.result, UiIpcResult::Outcome { .. }));
            let event_index = kinds.iter().position(|kind| *kind == "UiEvent");
            let response_index = kinds.iter().position(|kind| *kind == "UiResponse");
            assert!(
                matches!((event_index, response_index), (Some(event), Some(response)) if event < response),
                "事件必须先于响应：{kinds:?}"
            );
            return Ok(kinds);
        }
    }
    Err(format!("在真实管道上没等到响应：{kinds:?}").into())
}

#[test]
fn test_ui_request_response_and_event_over_a_real_pipe() -> Result<(), Box<dyn std::error::Error>> {
    let pipe_name = unique_pipe_name();
    let handled_count = Arc::new(AtomicUsize::new(0));
    let handled_on_server = Arc::clone(&handled_count);
    let server_pipe = pipe_name.clone();

    let server = thread::spawn(move || -> Result<(), IpcError> {
        let mut transport = NamedPipeTransport::server(&server_pipe)?;
        transport.accept(HANDSHAKE_TIMEOUT)?;
        let session_id = generate_session_id()?;
        let handshake = server_handshake(
            &mut transport,
            TOKEN,
            Vec::new(),
            &session_id,
            HANDSHAKE_TIMEOUT,
        )?;
        let mut handler = CountingHandler {
            handled: handled_on_server,
        };
        let mut events = OneEventSource {
            pending: vec![UiEvent::TaskStateChanged {
                task_id: "t_1".to_owned(),
                status: "running".to_owned(),
                hold_reason: None,
            }],
        };
        serve_session_with_events(
            &mut transport,
            &handshake.server_hello,
            HEARTBEAT_TIMEOUT,
            &mut handler,
            &mut events,
        )
    });

    let mut transport = NamedPipeTransport::client(&pipe_name)?;
    transport.connect(HANDSHAKE_TIMEOUT)?;
    let server_hello: ServerHello =
        client_handshake(&mut transport, TOKEN, Vec::new(), HANDSHAKE_TIMEOUT)?;
    assert_eq!(server_hello.version, assistant_ipc::IPC_PROTOCOL_VERSION);

    transport.send(&WireMessage::UiRequest(submit_intent()), HANDSHAKE_TIMEOUT)?;

    // 收集顺序：事件必须先于响应到达（Core 侧在阻塞读之前推事件）
    let _kinds = read_until_response(&mut transport)?;
    assert_eq!(
        handled_count.load(Ordering::SeqCst),
        1,
        "请求必须恰好触达处理器一次"
    );

    // 客户端消失后，会话必须以显式断连结束，而不是挂住
    drop(transport);
    let disconnected = Instant::now() + Duration::from_secs(2);
    while !server.is_finished() && Instant::now() < disconnected {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        server.is_finished(),
        "客户端断开后 Core 侧会话没有在 2s 内收尾"
    );
    let ended = server.join().expect("server thread");
    assert!(
        matches!(ended, Err(IpcError::Disconnected { .. })),
        "期望显式断连，实际 {ended:?}"
    );
    Ok(())
}

/// 保留一个显式的心跳断言，避免"会话只测了请求路径"的假绿。
#[test]
fn test_heartbeat_monitor_used_by_the_session_is_not_a_noop() {
    let monitor = HeartbeatMonitor::new(Duration::from_millis(100), 1_000);
    assert!(monitor.check(1_050).is_ok());
    assert!(matches!(
        monitor.check(1_200),
        Err(IpcError::Timeout {
            operation: "heartbeat",
            ..
        })
    ));
    let heartbeat = Heartbeat {
        session_id: generate_session_id().expect("session id").to_string(),
        timestamp_unix_ms: 1,
        alive: true,
    };
    assert!(heartbeat.alive);
}
