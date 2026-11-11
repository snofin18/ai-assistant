//! Real NamedPipe contract tests for the shared desktop UI session.
//!
//! The server is scripted in-process. No real browser, profile, form, or human
//! approval action is involved.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::mpsc;
use std::sync::{Arc, mpsc::Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, NamedPipeTransport, Transport, UiIpcEvent, UiIpcRequest, UiIpcResponse, WireMessage,
    generate_session_id, server_handshake,
};
use serde_json::{Value, json};

use super::CorePipeSession;
use crate::commands::UiTransportFailure;
use crate::core_pipe::CorePipeConfig;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn unique_pipe_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("assistant-ui-session-{}-{nanos}", std::process::id())
}

fn test_config(pipe_name: String) -> CorePipeConfig {
    CorePipeConfig {
        pipe_name,
        token_environment_variable: "UNUSED_TEST_TOKEN".to_owned(),
        connect_timeout: Duration::from_secs(2),
        request_timeout: Duration::from_secs(2),
    }
}

fn heartbeat(session_id: &str, timestamp_unix_ms: u64) -> WireMessage {
    WireMessage::Heartbeat(Heartbeat {
        session_id: session_id.to_owned(),
        timestamp_unix_ms,
        alive: true,
    })
}

fn await_request(transport: &mut NamedPipeTransport) -> Result<UiIpcRequest, String> {
    loop {
        match transport
            .recv(Duration::from_secs(2))
            .map_err(|error| error.to_string())?
        {
            WireMessage::Heartbeat(_) => {}
            WireMessage::UiRequest(request) => return Ok(request),
            other => return Err(format!("unexpected {}", other.kind())),
        }
    }
}

fn serve_test_session(pipe_name: &str) -> Result<(), String> {
    let mut transport = NamedPipeTransport::server(pipe_name).map_err(|error| error.to_string())?;
    transport
        .accept(Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    let session_id = generate_session_id().map_err(|error| error.to_string())?;
    let handshake = server_handshake(
        &mut transport,
        TOKEN,
        Vec::new(),
        &session_id,
        Duration::from_secs(2),
    )
    .map_err(|error| error.to_string())?;
    let hello = handshake.server_hello;
    transport
        .send(&heartbeat(&hello.session_id, 1), Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    transport
        .send(
            &WireMessage::UiEvent(UiIpcEvent::new(json!({
                "kind": "step_approval_requested",
                "request_id": "runtime:task:submit"
            }))),
            Duration::from_secs(2),
        )
        .map_err(|error| error.to_string())?;

    let request = await_request(&mut transport)?;
    transport
        .send(&heartbeat(&hello.session_id, 2), Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    transport
        .send(
            &WireMessage::UiResponse(UiIpcResponse::outcome(
                request.correlation_id,
                json!({
                    "status": "approval_granted",
                    "request_id": "runtime:task:submit",
                    "scope": "once"
                }),
            )),
            Duration::from_secs(2),
        )
        .map_err(|error| error.to_string())?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

fn forward_event(event: Value, sender: &Sender<Value>) -> Result<(), UiTransportFailure> {
    sender
        .send(event)
        .map_err(|_| UiTransportFailure::unavailable("test event receiver is unavailable"))
}

#[test]
fn test_events_commands_and_heartbeats_share_one_pipe_session() {
    let pipe_name = unique_pipe_name();
    let server_pipe = pipe_name.clone();
    let server = thread::spawn(move || serve_test_session(&server_pipe));
    let (event_sender, event_receiver) = mpsc::channel();
    let session = Arc::new(CorePipeSession::with_test_token(
        test_config(pipe_name),
        TOKEN,
    ));
    let event_session = Arc::clone(&session);
    let subscriber =
        thread::spawn(move || event_session.subscribe(|event| forward_event(event, &event_sender)));

    let event = event_receiver
        .recv_timeout(Duration::from_secs(3))
        .expect("approval event");
    assert_eq!(event["kind"], "step_approval_requested");
    let outcome = session
        .send(json!({
            "version": "1.0",
            "command": {
                "kind": "approve_request",
                "request_id": "runtime:task:submit",
                "scope": "once"
            }
        }))
        .expect("approval response");
    assert_eq!(outcome["status"], "approval_granted");

    subscriber
        .join()
        .expect("subscriber thread")
        .expect("clean session close");
    server
        .join()
        .expect("server thread")
        .expect("server result");
}
