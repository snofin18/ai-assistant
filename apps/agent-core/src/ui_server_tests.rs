//! `ui_server.rs` 的单元测试：Core 侧 UI 监听端的白盒用例。
//!
//! **为什么单独一个文件**：`ui_server.rs` 连同测试会超过 gov §5.4 的 600 行警告阈值。
//! 用 `#[path]` 把 `mod tests` 外置后两侧都回到阈值内，而测试**仍然是本模块的私有单元测试** ——
//! `use super::` 照旧能访问私有项，这与 `tests/` 目录下的集成测试有本质区别。
//! 声明处在 `ui_server.rs` 末尾：`#[cfg(test)] #[path = "ui_server_tests.rs"] mod tests;`。

use std::collections::VecDeque;
use std::time::Duration;

use assistant_ipc::{
    IpcError, RequestMessage, ServerHello, Transport, UiIpcRequest, UiIpcResult, WireMessage,
};
use assistant_protocol::ToolEnvelope;

use super::{
    UiEventSource, UiServerConfig, looks_like_image_path, process_ui_request, push_events,
    serve_session, serve_session_with_events,
};
use crate::ui_ipc::{
    UiAuthorizationScope, UiCommand, UiCommandError, UiCommandHandler, UiCommandOutcome,
};

#[derive(Default)]
struct ScriptedTransport {
    incoming: VecDeque<WireMessage>,
    sent: Vec<WireMessage>,
}

impl Transport for ScriptedTransport {
    fn connect(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
        Ok(())
    }
    fn accept(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
        Ok(())
    }
    fn send(&mut self, message: &WireMessage, _timeout: Duration) -> assistant_ipc::IpcResult<()> {
        self.sent.push(message.clone());
        Ok(())
    }
    fn recv(&mut self, _timeout: Duration) -> assistant_ipc::IpcResult<WireMessage> {
        self.incoming
            .pop_front()
            .ok_or_else(|| IpcError::Disconnected {
                message: "scripted transport exhausted".to_owned(),
            })
    }
    fn close(&mut self) -> assistant_ipc::IpcResult<()> {
        Ok(())
    }
    fn peer_process_id(&self) -> assistant_ipc::IpcResult<u32> {
        Ok(1)
    }
}

#[derive(Default)]
struct RecordingHandler {
    seen: Vec<UiCommand>,
    reject_with: Option<&'static str>,
}

impl UiCommandHandler for RecordingHandler {
    fn handle(&mut self, command: UiCommand) -> Result<UiCommandOutcome, UiCommandError> {
        self.seen.push(command);
        if let Some(field) = self.reject_with {
            return Err(UiCommandError::EmptyField { field });
        }
        Ok(UiCommandOutcome::IntentAccepted {
            intent_id: "i_1".to_owned(),
        })
    }
}

fn hello() -> ServerHello {
    ServerHello {
        version: assistant_ipc::IPC_PROTOCOL_VERSION,
        capabilities: Vec::new(),
        session_id: "018f6d4e-52a1-7b03-8f22-1234567890ab".to_owned(),
    }
}

fn submit_intent_request() -> UiIpcRequest {
    UiIpcRequest::new(
        "c_1",
        serde_json::json!({
            "version": "1.0",
            "command": { "kind": "submit_intent", "intent_id": "i_1", "goal": "replace and save" }
        }),
    )
}

fn dispatch(request: &UiIpcRequest) -> (RecordingHandler, UiIpcResult) {
    let mut handler = RecordingHandler::default();
    let response = process_ui_request(&mut handler, request);
    assert_eq!(
        response.correlation_id, request.correlation_id,
        "响应必须回同一 correlation_id"
    );
    (handler, response.result)
}

#[test]
fn test_valid_request_is_dispatched_and_answered() {
    let (handler, result) = dispatch(&submit_intent_request());
    assert_eq!(handler.seen.len(), 1);
    assert!(matches!(
        result,
        UiIpcResult::Outcome { ref outcome }
            if outcome.get("status").and_then(|value| value.as_str()) == Some("intent_accepted")
    ));
}

#[test]
fn test_unknown_field_is_rejected_before_the_handler_runs() {
    let request = UiIpcRequest::new(
        "c_2",
        serde_json::json!({
            "version": "1.0",
            "command": { "kind": "pause_task", "task_id": "t_1", "approved_by": "attacker" }
        }),
    );
    let (handler, result) = dispatch(&request);
    assert!(handler.seen.is_empty(), "非法载荷不得触达处理器");
    assert!(matches!(
        result,
        UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
    ));
}

#[test]
fn test_version_drift_is_rejected_before_the_handler_runs() {
    let request = UiIpcRequest::new(
        "c_3",
        serde_json::json!({
            "version": "9.9",
            "command": { "kind": "pause_task", "task_id": "t_1" }
        }),
    );
    let (handler, result) = dispatch(&request);
    assert!(handler.seen.is_empty());
    assert!(matches!(
        result,
        UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
    ));
}

#[test]
fn test_handler_rejection_is_mapped_to_an_error_code() {
    let mut handler = RecordingHandler {
        seen: Vec::new(),
        reject_with: Some("task_id"),
    };
    let response = process_ui_request(&mut handler, &submit_intent_request());
    assert!(matches!(
        response.result,
        UiIpcResult::Rejected { ref code, .. } if code == "ToolInvalidArgs"
    ));
}

#[test]
fn test_approval_scope_matches_the_shared_contract() {
    let request = UiIpcRequest::new(
        "c_4",
        serde_json::json!({
            "version": "1.0",
            "command": { "kind": "approve_request", "request_id": "a_1", "scope": "once" }
        }),
    );
    let (handler, result) = dispatch(&request);
    assert!(matches!(result, UiIpcResult::Outcome { .. }));
    assert_eq!(
        handler.seen.first(),
        Some(&UiCommand::ApproveRequest {
            request_id: "a_1".to_owned(),
            scope: UiAuthorizationScope::Once,
        })
    );
}

#[test]
fn test_session_dispatches_request_and_sends_matching_response() {
    let mut transport = ScriptedTransport::default();
    transport
        .incoming
        .push_back(WireMessage::UiRequest(submit_intent_request()));
    let mut handler = RecordingHandler::default();
    let ended = serve_session(
        &mut transport,
        &hello(),
        Duration::from_millis(2_000),
        &mut handler,
    );
    assert!(matches!(ended, Err(IpcError::Disconnected { .. })));
    assert!(matches!(
        transport.sent.first(),
        Some(WireMessage::Heartbeat(_))
    ));
    let Some(response) = transport.sent.iter().find_map(|message| match message {
        WireMessage::UiResponse(response) => Some(response),
        _ => None,
    }) else {
        return;
    };
    assert_eq!(response.correlation_id, "c_1");
    assert_eq!(handler.seen.len(), 1);
}

#[test]
fn test_tool_shaped_request_on_ui_channel_fails_the_session() {
    let mut transport = ScriptedTransport::default();
    transport
        .incoming
        .push_back(WireMessage::Request(RequestMessage::new(
            "c_1",
            "document",
            ToolEnvelope::error(
                "notepad.text.read".to_owned(),
                "t_1".to_owned(),
                "s_1".to_owned(),
                assistant_protocol::ErrorCode::Fatal,
                "wrong channel",
            ),
        )));
    let mut handler = RecordingHandler::default();
    assert!(matches!(
        serve_session(
            &mut transport,
            &hello(),
            Duration::from_millis(2_000),
            &mut handler
        ),
        Err(IpcError::UnexpectedMessage { .. })
    ));
    assert!(handler.seen.is_empty());
}

#[test]
fn test_peer_allow_list_is_case_and_separator_insensitive() {
    let config = UiServerConfig::new(
        "assistant-ui-test",
        "ASSISTANT_UI_TOKEN",
        Duration::from_millis(2_000),
    )
    .with_allowed_peer(r"C:\Apps\Assistant.exe");
    assert!(config.is_peer_allowed("c:/apps/assistant.exe"));
    assert!(!config.is_peer_allowed(r"C:\Apps\Other.exe"));
}

#[test]
fn test_empty_peer_allow_list_denies_everyone() {
    let config = UiServerConfig::new(
        "assistant-ui-test",
        "ASSISTANT_UI_TOKEN",
        Duration::from_millis(2_000),
    );
    assert!(!config.is_peer_allowed(r"C:\Apps\Assistant.exe"));
    assert!(looks_like_image_path(std::path::Path::new("a.exe")));
    assert!(!looks_like_image_path(std::path::Path::new("")));
}

/// 一次性的假事件源：第一次 drain 返回 1 条，之后返回空。
struct OneEvent {
    pending: Vec<crate::ui_ipc::UiEvent>,
}

impl UiEventSource for OneEvent {
    fn drain(&mut self) -> Vec<crate::ui_ipc::UiEvent> {
        std::mem::take(&mut self.pending)
    }
}

fn one_event() -> OneEvent {
    OneEvent {
        pending: vec![crate::ui_ipc::UiEvent::TaskStateChanged {
            task_id: "t_1".to_owned(),
            status: "running".to_owned(),
            hold_reason: None,
        }],
    }
}

#[test]
fn test_push_events_sends_one_way_ui_events() {
    let mut transport = ScriptedTransport::default();
    let mut source = one_event();
    let pushed = push_events(&mut transport, &mut source, Duration::from_millis(2_000));
    assert_eq!(pushed, Ok(1));
    assert_eq!(transport.sent.len(), 1);
    let Some(WireMessage::UiEvent(event)) = transport.sent.first() else {
        return;
    };
    assert_eq!(
        event.event.get("kind").and_then(|value| value.as_str()),
        Some("task_state_changed")
    );
    // 第二次 drain 为空：事件只推一次，不重复
    assert_eq!(
        push_events(&mut transport, &mut source, Duration::from_millis(2_000)),
        Ok(0)
    );
}

#[test]
fn test_session_pushes_events_before_serving_the_request() {
    let mut transport = ScriptedTransport::default();
    transport
        .incoming
        .push_back(WireMessage::UiRequest(submit_intent_request()));
    let mut handler = RecordingHandler::default();
    let mut source = one_event();
    let ended = serve_session_with_events(
        &mut transport,
        &hello(),
        Duration::from_millis(2_000),
        &mut handler,
        &mut source,
    );
    assert!(matches!(ended, Err(IpcError::Disconnected { .. })));
    let kinds: Vec<&str> = transport.sent.iter().map(WireMessage::kind).collect();
    assert_eq!(
        kinds,
        vec!["Heartbeat", "UiEvent", "UiResponse"],
        "事件必须先于响应发出，且顺序确定"
    );
}
