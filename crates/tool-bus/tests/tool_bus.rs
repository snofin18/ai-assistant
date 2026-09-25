//! MCP 全链路 + 统一信封 + JSON Schema 拒绝 + 截断契约（TASK-020 交付物 2~6）。
//!
//! 为什么是集成测试而不是 `#[cfg(test)]` 模块：这里要证明的结论都在**跨 MCP 边界**上
//! （`tools/list` / `tools/call` 真实往返、`_meta` 身份传递、服务端返回信封），单元测试
//! 覆盖不到这条路径。
//!
//! `unwrap` / `expect` / `panic` 的例外由本文件顶部一行显式放开（AGENTS.md §5.3）。

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use assistant_protocol::{ErrorCode, RiskLevel, SourceKind, TruncationReason};
use assistant_tool_bus::{
    CallContext, MountSelection, SourceDescriptor, ToolBus, ToolBusConfig, ToolDefinition,
    ToolHandler, ToolOutput, ToolRegistry,
};
use common::{arguments, assert_rejected, clock, echo_definition, echo_handler, payload, register};
use serde_json::{Map, Value, json};

/// DoD「内部工具经 MCP 表达」+「信封字段齐全」：`tools/list` 与 `tools/call` 都走真实 MCP 往返。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mcp_round_trip_lists_and_calls_tool() {
    let mut registry = ToolRegistry::new();
    register(
        &mut registry,
        echo_definition("把入参原样返回"),
        echo_handler(),
    );

    let (bus, report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-1"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    // ① tools/list：服务端实际暴露的名字必须与本侧挂载报告逐字一致（含两个元工具）。
    let exposed = bus
        .list_tool_names()
        .await
        .expect("tools/list must succeed");
    assert_eq!(exposed, report.mounted);
    assert_eq!(
        exposed,
        vec![
            "demo.echo.echo".to_owned(),
            "toolset.list".to_owned(),
            "toolset.search".to_owned(),
        ]
    );

    // ② tools/call：拿到成功信封，身份字段来自 `_meta`（不是从进程状态里猜的）。
    let envelope = bus
        .call_tool(
            "demo.echo.echo",
            arguments(json!({ "text": "hello" })),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("tools/call must return an envelope");

    assert!(envelope.ok);
    assert_eq!(envelope.version, "1.0");
    assert_eq!(envelope.tool, "demo.echo.echo");
    assert_eq!(envelope.task_id, "task-1");
    assert_eq!(envelope.step_id, "step-1");
    assert_eq!(payload(&envelope), &json!({ "echoed": "hello" }));
    assert!(!envelope.untrusted);
    assert!(envelope.source.is_none());
    assert!(envelope.truncated.is_none());
    assert!(envelope.evidence.is_none());
    assert!(envelope.error.is_none());
    let metrics = envelope
        .metrics
        .as_ref()
        .expect("a success envelope must carry metrics");
    assert_eq!(metrics.attempts, 1);

    bus.shutdown().await.expect("bus must shut down");
}

/// 架构 v2 §12.4：不可信内容必须带 `source`，且线上形状与 `envelope-1.0.json` 对齐。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_envelope_marks_untrusted_content_with_source() {
    let mut registry = ToolRegistry::new();
    let definition = ToolDefinition::new(
        "demo.app.read_text",
        "读取目标应用里的正文（测试用）",
        RiskLevel::Low,
        json!({ "type": "object", "additionalProperties": false }),
    )
    .expect("the reader definition must be valid");
    let handler: Arc<dyn ToolHandler> = Arc::new(
        |_call: &CallContext,
         _arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            Ok(ToolOutput::untrusted(
                SourceDescriptor::app_content("com.example.notepad").with_target("doc_1"),
                json!({ "text": "来自目标应用的正文" }),
            ))
        },
    );
    register(&mut registry, definition, handler);

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-untrusted"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let envelope = bus
        .call_tool(
            "demo.app.read_text",
            arguments(json!({})),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("tools/call must return an envelope");

    assert!(envelope.ok);
    assert!(envelope.untrusted);
    let source = envelope
        .source
        .as_ref()
        .expect("untrusted content must carry a source");
    assert_eq!(source.kind, SourceKind::AppContent);
    assert_eq!(source.app_id.as_deref(), Some("com.example.notepad"));
    assert_eq!(source.target.as_deref(), Some("doc_1"));

    // 线上形状：`kind` 是稳定 token（`app_content`），不是 Rust 变体名。
    let wire = serde_json::to_value(&envelope).expect("envelope must serialize");
    assert_eq!(wire.get("untrusted"), Some(&json!(true)));
    assert_eq!(
        wire.get("source").and_then(|value| value.get("kind")),
        Some(&json!("app_content"))
    );
    assert_eq!(wire.get("error"), Some(&Value::Null));
    assert_eq!(
        wire.get("data")
            .and_then(|value| value.get("text"))
            .and_then(Value::as_str),
        Some("来自目标应用的正文")
    );

    bus.shutdown().await.expect("bus must shut down");
}

/// 铁律 1 / 不变量 1：参数不合法直接拒，**永不进入 handler**。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_invalid_arguments_are_rejected_before_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let handler: Arc<dyn ToolHandler> = Arc::new(
        move |_call: &CallContext,
              _arguments: &Map<String, Value>|
              -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutput::json(json!({ "ran": true })))
        },
    );

    let mut registry = ToolRegistry::new();
    register(&mut registry, echo_definition("把入参原样返回"), handler);

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-invalid"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    // 负向用例 1：缺必填字段。
    let missing = bus
        .call_tool(
            "demo.echo.echo",
            arguments(json!({})),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("a rejected call is still an envelope");
    assert_rejected(&missing, "missing required property `text`");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // 负向用例 2：类型不对 + 多余字段（`additionalProperties: false`）。
    let wrong_type = bus
        .call_tool(
            "demo.echo.echo",
            arguments(json!({ "text": 42, "extra": true })),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("a rejected call is still an envelope");
    assert_rejected(&wrong_type, "expected type `string`");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // 正向：合法参数才真的进 handler。
    let accepted = bus
        .call_tool(
            "demo.echo.echo",
            arguments(json!({ "text": "hello" })),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("tools/call must return an envelope");
    assert!(accepted.ok);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    bus.shutdown().await.expect("bus must shut down");
}

/// 不变量 3：超预算的载荷被截断，且**显式**标注 `reason` 与 `original_bytes`（不是静默丢数据）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_oversized_payload_is_truncated_not_silently_dropped() {
    let handler: Arc<dyn ToolHandler> = Arc::new(
        |_call: &CallContext,
         _arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            Ok(ToolOutput::json(json!({
                "alpha": "A".repeat(4000),
                "beta": "B".repeat(10),
            }))
            .with_max_bytes(256))
        },
    );

    let mut registry = ToolRegistry::new();
    let definition = ToolDefinition::new(
        "demo.echo.big",
        "返回一个大载荷（测试用）",
        RiskLevel::Low,
        json!({ "type": "object", "additionalProperties": false }),
    )
    .expect("the big-payload definition must be valid");
    register(&mut registry, definition, handler);

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-truncate"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let envelope = bus
        .call_tool(
            "demo.echo.big",
            arguments(json!({})),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("tools/call must return an envelope");

    assert!(envelope.ok);
    let truncated = envelope
        .truncated
        .as_ref()
        .expect("an over-budget payload must be reported as truncated");
    assert!(truncated.occurred);
    assert_eq!(truncated.reason, TruncationReason::MaxBytes);

    let original = json!({
        "alpha": "A".repeat(4000),
        "beta": "B".repeat(10),
    });
    let original_bytes = serde_json::to_vec(&original)
        .expect("payload must serialize")
        .len();
    assert_eq!(truncated.original_bytes, Some(original_bytes as u64));

    // 截断后的载荷必须真的落到预算内 —— 否则这条路径就是「假装成功」。
    let delivered_bytes = serde_json::to_vec(payload(&envelope))
        .expect("payload must serialize")
        .len();
    assert!(
        delivered_bytes <= 256,
        "truncated payload is {delivered_bytes} bytes, over the 256-byte budget"
    );
    assert_ne!(payload(&envelope), &original);

    bus.shutdown().await.expect("bus must shut down");
}

/// 不变量 3 的另一半：**截不动就失败**（fail-closed），绝不放过超限载荷。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_untruncatable_payload_fails_closed() {
    let handler: Arc<dyn ToolHandler> = Arc::new(
        |_call: &CallContext,
         _arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            // 没有任何字符串叶子 → 截断器无从下手 → 必须失败（不能静默放行）。
            Ok(ToolOutput::json(json!({ "count": 1, "ratio": 2, "flag": true })).with_max_bytes(8))
        },
    );

    let mut registry = ToolRegistry::new();
    let definition = ToolDefinition::new(
        "demo.echo.numbers",
        "返回一个没有字符串叶子的载荷（测试用）",
        RiskLevel::Low,
        json!({ "type": "object", "additionalProperties": false }),
    )
    .expect("the numbers definition must be valid");
    register(&mut registry, definition, handler);

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-fail-closed"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let envelope = bus
        .call_tool(
            "demo.echo.numbers",
            arguments(json!({})),
            &CallContext::new("task-1", "step-1"),
        )
        .await
        .expect("a fail-closed result is still an envelope");

    assert!(!envelope.ok);
    assert!(envelope.data.is_none(), "a failed call must not ship data");
    let error = envelope
        .error
        .as_ref()
        .expect("a failed envelope must carry error");
    assert_eq!(error.code, ErrorCode::Fatal);
    assert!(
        error.message.contains("cannot be truncated"),
        "error message must explain why it failed: {:?}",
        error.message
    );

    bus.shutdown().await.expect("bus must shut down");
}
