//! # assistant-tool-bus —— 工具通道的唯一入口
//!
//! 职责：把「工具从哪来、怎么被校验、怎么被挂载、怎么被调用、结果长什么样」收敛成
//! **唯一一条通道**（架构 v2 §5.2 / §5.3 / §5.4 / §5.5）。
//!
//! 落地方式：`rmcp`（MCP 官方 Rust SDK）在**同进程**内起一个 MCP server，
//! 传输是 `tokio::io::duplex` 的内存管道（无 socket、无子进程、无网络），上层通过
//! MCP `tools/list` / `tools/call` 与它交互。将来接入外部 MCP server 时**协议不变**，
//! 只换传输 —— 这是架构 v2 §5.4「从第一天起内部工具也以 MCP 表达」的落地。
//!
//! ## 边界（不做什么）
//!
//! - **不做任何放行判断**（铁律 3）：白名单 / 风险分级 / 默认拒绝 / 审批决策全部归
//!   `crates/policy`（TASK-021）。本 crate 只做「校验参数 → 调 handler → 组装信封」。
//! - 不接真实 Adapter 工具（TASK-035 起）；本 crate 只用测试用内置工具证明链路可用。
//! - 不加载 / 不治理外部 MCP server（stdio、streamable-http 一律没有接入，属阶段 3）。
//! - 不做结果缓存、不做 token 预算、不写审计库（审计链由 `crates/audit` 持有）。
//! - **不自建第二套**信封 / 错误码 / 工具 schema：全部复用 `assistant_protocol`。
//!
//! ## 不变量
//!
//! 1. **参数校验发生在 handler 之前**：不符合 `ToolSchema.input` 的调用直接返回
//!    `ErrorCode::ToolInvalidArgs`，**永不进入 handler**（铁律 1 / 铁律 4）。
//! 2. **只要到达 handler，返回路径上一定有信封**：成功、工具失败、参数非法三种结果
//!    都是同一个 `assistant_protocol::ToolEnvelope` 形状。
//! 3. **截断不静默**：载荷超 `max_bytes` 一定带 `truncated.occurred = true` +
//!    `reason` + `original_bytes`；无法按契约截断时让调用**失败**，绝不放过超限载荷。
//! 4. **指纹只由「真正挂载的工具」决定**：同一集合任意挂载顺序 → 同一指纹；
//!    集合里任何一个字节变化 → 指纹变化。
//! 5. **单次挂载的工具数 > 40 必须留下告警**（架构 v2 §5.5 第 5 条）：挂载报告里带
//!    结构化 `ToolsetOversizeWarning` + 未串链的 `AuditEvent`，禁止只打一行日志。
//! 6. 零 `unsafe`、零 `#[allow]`（`tests/` 与 `#[cfg(test)]` 除外，依据 AGENTS.md §5.3）。
//!
//! ## 典型用法
//!
//! ```no_run
//! # async fn demo() -> Result<(), assistant_tool_bus::ToolBusError> {
//! use std::sync::Arc;
//!
//! use assistant_protocol::RiskLevel;
//! use assistant_tool_bus::{
//!     CallContext, MountSelection, SystemClock, ToolBus, ToolBusConfig, ToolDefinition,
//!     ToolHandler, ToolOutput, ToolRegistry,
//! };
//!
//! struct Echo;
//!
//! impl ToolHandler for Echo {
//!     fn call(
//!         &self,
//!         _call: &CallContext,
//!         arguments: &serde_json::Map<String, serde_json::Value>,
//!     ) -> Result<ToolOutput, assistant_tool_bus::ToolBusError> {
//!         Ok(ToolOutput::json(serde_json::json!({ "echo": arguments })))
//!     }
//! }
//!
//! let mut registry = ToolRegistry::new();
//! let definition = ToolDefinition::new(
//!     "demo.echo.echo",
//!     "把入参原样返回（示例工具）",
//!     RiskLevel::Low,
//!     serde_json::json!({ "type": "object" }),
//! )?;
//! registry.register(definition, Arc::new(Echo))?;
//!
//! let clock = Arc::new(SystemClock);
//! let (bus, _report) = ToolBus::start(
//!     registry,
//!     MountSelection::all(),
//!     ToolBusConfig::new("session-1"),
//!     clock,
//! )
//! .await?;
//! let envelope = bus
//!     .call_tool(
//!         "demo.echo.echo",
//!         serde_json::Map::new(),
//!         &CallContext::new("task-1", "step-1"),
//!     )
//!     .await?;
//! assert!(envelope.ok);
//! bus.shutdown().await?;
//! # Ok(())
//! # }
//! ```
//!
//! 相关：架构 v2 §5.2 / §5.3 / §5.4 / §5.5 / §12.4、`docs/spec/tool-schema.md`、
//! `docs/spec/error-codes.md`、ADR-0019（负向用例）/ ADR-0021（受控词）/ ADR-0024（依赖登记）、
//! `tasks/TASK-020-tool-bus-mcp-rmcp-server.md`。

#![deny(unsafe_code)]

mod bus;
mod clock;
mod envelope;
mod error;
mod fingerprint;
mod handler;
mod meta;
mod mount;
mod registry;
mod schema;
mod server;

pub use bus::{ToolBus, ToolBusConfig};
pub use clock::{Clock, SystemClock, rfc3339_utc_from_unix_ms};
pub use envelope::{
    EnvelopeRequest, EvidenceDescriptor, SourceDescriptor, assemble_envelope, error_envelope,
};
pub use error::{ToolBusError, ToolBusResult};
pub use fingerprint::ToolsetFingerprint;
pub use handler::{CallContext, ToolHandler, ToolOutput};
pub use mount::{
    MAX_MOUNTED_TOOLS_WITHOUT_WARNING, MountReport, MountSelection, ToolRegistry,
    ToolsetOversizeWarning,
};
pub use registry::ToolDefinition;
pub use schema::{
    ANNOTATION_KEYWORDS, NON_DRAFT07_KEYWORDS, REJECTED_FOR_NOW_KEYWORDS,
    REJECTED_FOREVER_KEYWORDS, SUPPORTED_KEYWORDS, collect_unenforceable_constructs,
    validate_arguments,
};
