//! 两个测试二进制共用的构件：时钟、入参、注册、信封读取与几个测试用工具。
//!
//! 为什么单独一个模块：`tests/` 下每个 `.rs` 都是**独立 crate**，共享代码只能这样放。
//! 每个二进制只用到其中一部分，因此这里显式放开 `dead_code`（测试构件不是公共 API）。
//!
//! `unwrap` / `expect` / `panic` 的例外与其它测试文件同源（AGENTS.md §5.3）。

#![allow(dead_code, clippy::expect_used, clippy::unwrap_used, clippy::panic)]
// 模块级的共享代码不该触发 pedantic 的「公共 API 才需要文档」之外的规则。
#![allow(clippy::missing_panics_doc)]

use std::sync::Arc;

use assistant_protocol::{ErrorCode, RiskLevel, ToolEnvelope};
use assistant_tool_bus::{
    CallContext, MountSelection, SystemClock, ToolBus, ToolBusConfig, ToolDefinition, ToolHandler,
    ToolOutput, ToolRegistry, ToolsetFingerprint,
};
use serde_json::{Map, Value, json};

/// 测试用时钟：直接吃系统时钟 —— 指纹里没有时间成分，时间只进超限审计事件的 `ts`。
pub fn clock() -> Arc<SystemClock> {
    Arc::new(SystemClock)
}

/// 把 JSON 字面量转成 `tools/call` 的入参 `Map`（测试里写 `arguments(json!({ .. }))`）。
pub fn arguments(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("tool arguments must be a JSON object, got {other}"),
    }
}

/// 注册一个工具，注册失败即测试失败（正常路径下不该失败）。
pub fn register(
    registry: &mut ToolRegistry,
    definition: ToolDefinition,
    handler: Arc<dyn ToolHandler>,
) {
    registry
        .register(definition, handler)
        .expect("registering a fresh tool must succeed");
}

/// 信封里的载荷（`data`）。成功信封一定带它；不带就是缺陷，测试直接失败。
pub const fn payload(envelope: &ToolEnvelope) -> &Value {
    &envelope
        .data
        .as_ref()
        .expect("a success envelope must carry data")
        .0
}

/// 元工具输出里的工具名列表。
pub fn listed_names(value: &Value) -> Vec<String> {
    value
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// 测试用「回声」工具定义：必填 `text: string`、拒收多余字段（负向用例依赖这两条）。
pub fn echo_definition(description: &str) -> ToolDefinition {
    ToolDefinition::new(
        "demo.echo.echo",
        description,
        RiskLevel::Low,
        json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
            "additionalProperties": false
        }),
    )
    .expect("the echo definition must be a valid tool declaration")
}

/// 测试用「回声」handler：把 `text` 原样放进载荷。
pub fn echo_handler() -> Arc<dyn ToolHandler> {
    Arc::new(
        |_call: &CallContext,
         arguments: &Map<String, Value>|
         -> assistant_tool_bus::ToolBusResult<ToolOutput> {
            Ok(ToolOutput::json(json!({
                "echoed": arguments.get("text").cloned().unwrap_or(Value::Null),
            })))
        },
    )
}

/// `demo.<domain>.run` 形式的三段式名字（`domain` 以字母开头，满足 naming 规则）。
pub fn demo_definition(domain: &str, description: &str) -> ToolDefinition {
    ToolDefinition::new(
        format!("demo.{domain}.run"),
        description,
        RiskLevel::Low,
        json!({ "type": "object" }),
    )
    .expect("a three-segment name with an object schema must be valid")
}

/// 挂载一个注册表并取回指纹（同时证明 `start` / `shutdown` 全链路可用）。
pub async fn mount_fingerprint(registry: ToolRegistry) -> ToolsetFingerprint {
    let (bus, report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("fingerprint-session"),
        clock(),
    )
    .await
    .expect("in-process bus must start");
    bus.shutdown().await.expect("in-process bus must shut down");
    report.fingerprint
}

/// 断言一封失败信封是「参数不合法」，且原因可读。
pub fn assert_rejected(envelope: &ToolEnvelope, expected_fragment: &str) {
    assert!(!envelope.ok, "invalid arguments must not produce ok=true");
    let error = envelope
        .error
        .as_ref()
        .expect("a failed envelope must carry error");
    assert_eq!(error.code, ErrorCode::ToolInvalidArgs);
    assert!(
        error.message.contains(expected_fragment),
        "error message {:?} must mention {expected_fragment:?}",
        error.message
    );
}
