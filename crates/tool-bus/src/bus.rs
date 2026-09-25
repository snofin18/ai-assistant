//! `ToolBus`：上层（core / model-gateway）使用的**唯一**工具通道入口。
//!
//! 上层只看到三个动作：`start`（挂载工具集并建立通道）、`call_tool`（调一次工具）、
//! `shutdown`（收摊）。MCP 的 `initialize` / `tools/list` / `tools/call`、
//! `tokio::io::duplex` 的内存管道、`_meta` 里的调用身份 —— **全部是本模块的内部实现**，
//! 不出现在公共 API 里（架构 v2 §5.4 + §11.4）。
//!
//! ## 为什么传输是 `tokio::io::duplex`（任务卡 Q4）
//!
//! 架构 v2 §5.4 要求「内置工具 = in-process MCP server，同机零网络」。`rmcp` 提供了
//! `IntoTransport for (R, W)`（任意 `AsyncRead + AsyncWrite` 对），`tokio::io::duplex`
//! 就是一对内存管道：无 socket、无子进程、无网络、无 `unsafe`。
//! 于是「内部工具也以 MCP 表达」这条架构要求**不需要**引入 stdio / http 传输，
//! 不需要证书、端口、进程管理。
//!
//! ## 运行时要求（**调用方必须知道**）
//!
//! [`ToolBus::start`] 必须在 tokio 运行时内调用（两侧的 serve 循环都会 `tokio::spawn`），
//! 且 `ToolBus` 与它绑定的运行时共存亡。测试用 `#[tokio::test]`。
//!
//! ## 与策略层的关系
//!
//! 本 crate **不做任何放行判断**（铁律 3）：`call_tool` 不会因为工具风险级而拒绝调用 ——
//! 那是 TASK-021 的 `crates/policy` 的职责，且必须在**本 crate 之前**完成。
//!
//! 相关：架构 v2 §5.3 / §5.4 / §5.5 / §11.4、`docs/spec/tool-schema.md`、
//! `tasks/TASK-020-tool-bus-mcp-rmcp-server.md`。

use std::sync::Arc;
use std::time::Duration;

use assistant_protocol::ToolEnvelope;
use rmcp::model::{CallToolRequestParams, RequestMetaObject, RequestParamsMeta};
use rmcp::service::{RunningService, Service, ServiceRole};
use rmcp::{ClientCacheConfig, RoleClient, RoleServer};
use serde_json::{Map, Value};

use crate::clock::Clock;
use crate::error::{ToolBusError, ToolBusResult};
use crate::handler::{CallContext, META_KEY_STEP_ID, META_KEY_TASK_ID};
use crate::mount::{MountReport, MountSelection, ToolRegistry};
use crate::server::ToolBusServer;

/// in-process 管道容量（字节）。
///
/// 只影响**吞吐**不影响正确性：读写两侧都在同一进程，64 KiB 足够让一次 `tools/call` 的
/// 请求与响应各占一次往返，不会因为背压而死锁（两侧由不同任务驱动）。
const TRANSPORT_BUFFER_BYTES: usize = 64 * 1024;

/// `shutdown` 的单侧关停上限。
///
/// 为什么要有上限：关停不能无限等（否则一个卡住的 handler 会把整个会话拖住）；
/// 超时是**显式失败**（[`ToolBusError::Transport`]），不是「假装关好了」。
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// 工具通道配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolBusConfig {
    session_id: String,
    default_max_bytes: Option<usize>,
    client_response_cache: bool,
}

impl ToolBusConfig {
    /// 最小配置：无默认载荷预算、关闭 client 响应缓存。
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            default_max_bytes: None,
            // 默认关：缓存会让 `tools/list` 看到**上一次**的清单，直接破坏「工具集指纹
            // 反映当下」这条不变量（任务卡 Out of scope 也明确「缓存不做」）。
            client_response_cache: false,
        }
    }

    /// 本次会话的默认载荷预算（字节）。工具可用
    /// [`ToolOutput::with_max_bytes`](crate::ToolOutput::with_max_bytes) 单独覆盖。
    #[must_use]
    pub const fn with_default_max_bytes(mut self, max_bytes: usize) -> Self {
        self.default_max_bytes = Some(max_bytes);
        self
    }

    /// 是否启用 `rmcp` client 的响应缓存（默认 `false`；打开前先读 §5.5 与指纹口径）。
    #[must_use]
    pub const fn with_client_response_cache(mut self, enabled: bool) -> Self {
        self.client_response_cache = enabled;
        self
    }

    /// 会话 id（进超限审计事件与调用身份）。
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 默认载荷预算。
    #[must_use]
    pub const fn default_max_bytes(&self) -> Option<usize> {
        self.default_max_bytes
    }

    /// 是否启用响应缓存。
    #[must_use]
    pub const fn client_response_cache(&self) -> bool {
        self.client_response_cache
    }
}

/// 工具通道：一对 in-process 的 MCP client + server。
///
/// 生命周期：`start` 创建、`call_tool` 使用、`shutdown` 结束。持有它即持有运行时限
/// （两侧 serve 循环的任务句柄）。
pub struct ToolBus {
    client: RunningService<RoleClient, ()>,
    server: RunningService<RoleServer, ToolBusServer>,
}

impl ToolBus {
    /// 挂载工具集、建立通道、返回 `(总线, 挂载报告)`。
    ///
    /// 挂载报告**必须**被调用方记进会话（架构 v2 §5.5 第 4 条：审计要能复现「模型当时
    /// 看到了什么」）；本函数把它交出去而不是藏起来。
    ///
    /// 幂等 / 副作用：每次调用都会创建新的管道与新的 serve 任务（不是幂等操作）。
    ///
    /// # Errors
    /// - [`ToolBusError::UnknownTool`]：挂载选择引用了未注册的工具
    /// - [`ToolBusError::Transport`]：MCP 初始化握手失败（含超时 / 连接关闭）
    /// - 其余：`ToolRegistry::mount` 的错误（指纹 / 审计事件组装）
    pub async fn start(
        registry: ToolRegistry,
        selection: MountSelection,
        config: ToolBusConfig,
        clock: Arc<dyn Clock>,
    ) -> ToolBusResult<(Self, MountReport)> {
        let mounted = Arc::new(registry.mount(&selection, config.session_id(), clock.as_ref())?);
        let report = mounted.report().clone();
        let handler = ToolBusServer::new(
            Arc::clone(&mounted),
            config.default_max_bytes(),
            Arc::clone(&clock),
        );
        let (server_side, client_side) = tokio::io::duplex(TRANSPORT_BUFFER_BYTES);

        // 两侧**必须并发**推进：`serve_server` 的初始化握手在等对端的 `initialize`，
        // 而 `serve_client` 正在发它。先 await 完一边 = 死锁。
        // 两个 future 的错误类型不同，因此用 `join!`（不是 `try_join!`）再各自映射。
        let (client_result, server_result) = tokio::join!(
            rmcp::serve_client((), client_side),
            rmcp::serve_server(handler, server_side),
        );
        let client = client_result.map_err(transport_failure)?;
        let server = server_result.map_err(transport_failure)?;

        if !config.client_response_cache() {
            client
                .peer()
                .set_response_cache_config(ClientCacheConfig::disabled())
                .await;
        }

        Ok((Self { client, server }, report))
    }

    /// 调一次工具，返回**信封**。
    ///
    /// 返回 `Ok(envelope)` 不表示工具成功 —— 工具失败也是信封（`envelope.ok == false` +
    /// `envelope.error`）。`Err(..)` 只表示**通道层**出问题（传输断了 / 对端没回信封），
    /// 此时调用方无法拿到任何可用的工具结果。
    ///
    /// # Errors
    /// - [`ToolBusError::Mcp`]：对端返回 JSON-RPC 错误
    /// - [`ToolBusError::Transport`]：传输层失败
    /// - [`ToolBusError::EnvelopeMissing`] / [`ToolBusError::EnvelopeAssembly`]：
    ///   对端没有回信封 / 回的 JSON 不是本项目的信封（内部缺陷）
    pub async fn call_tool(
        &self,
        tool: &str,
        arguments: Map<String, Value>,
        call: &CallContext,
    ) -> ToolBusResult<ToolEnvelope> {
        let mut params = CallToolRequestParams::new(tool.to_owned()).with_arguments(arguments);
        params.set_meta(call_context_meta(call));
        let result = self
            .client
            .call_tool(params)
            .await
            .map_err(map_service_error)?;
        extract_envelope(tool, &result)
    }

    /// 通过 MCP `tools/list` 取回**服务端实际暴露**的工具名（升序）。
    ///
    /// 为什么需要它：挂载报告是**本侧**的记录，而模型看到的是**服务端** `tools/list` 的
    /// 结果；两者必须一致才有意义（架构 v2 §5.5 第 4 条：审计要能复现「模型当时看到了
    /// 什么」）。返回值刻意只有名字，不是 `rmcp::model::Tool` —— MCP 细节不出公共 API
    /// （架构 v2 §11.4）。
    ///
    /// 幂等 / 副作用：只读，不改变服务端状态。
    ///
    /// # Errors
    /// - [`ToolBusError::Mcp`]：服务端返回 JSON-RPC 错误
    /// - [`ToolBusError::Transport`]：传输层失败
    pub async fn list_tool_names(&self) -> ToolBusResult<Vec<String>> {
        let tools = self
            .client
            .list_all_tools()
            .await
            .map_err(map_service_error)?;
        let mut names: Vec<String> = tools
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();
        Ok(names)
    }

    /// 关停通道（先 client 后 server，各自有 [`SHUTDOWN_TIMEOUT`] 上限）。
    ///
    /// 消耗 `self`：关掉之后再调用它是编程错误，让类型系统拦下它比运行时报错便宜。
    ///
    /// # Errors
    /// [`ToolBusError::Transport`]：任一侧关停超时或任务 join 失败。**不**吞掉 ——
    /// 关停失败意味着可能留下仍在跑的 serve 任务，调用方必须知道。
    pub async fn shutdown(self) -> ToolBusResult<()> {
        let Self {
            mut client,
            mut server,
        } = self;
        close_service(&mut client, "mcp client").await?;
        close_service(&mut server, "mcp server").await
    }
}

/// 调用身份 → MCP `_meta`。
///
/// 为什么走 `_meta` 而不是在 server 侧存「当前调用」：并发调用下任何进程级的「当前」
/// 都会串味（`handler.rs` 的 `CallContext` 文档有完整理由）。
fn call_context_meta(call: &CallContext) -> RequestMetaObject {
    let mut meta = RequestMetaObject::new();
    meta.0.0.insert(
        META_KEY_TASK_ID.to_owned(),
        Value::String(call.task_id().to_owned()),
    );
    meta.0.0.insert(
        META_KEY_STEP_ID.to_owned(),
        Value::String(call.step_id().to_owned()),
    );
    meta
}

/// 从 MCP 结果里取回信封。
///
/// 优先读 `structuredContent`（本 crate 的 server 一定填它）；退化路径是「第一个能解析成
/// 信封的文本块」，用于外部 MCP server 只回文本的情形。两条路都拿不到 → **报错**，
/// 绝不返回一个凑合的空信封（铁律 1）。
fn extract_envelope(
    tool: &str,
    result: &rmcp::model::CallToolResult,
) -> ToolBusResult<ToolEnvelope> {
    if let Some(value) = &result.structured_content {
        return serde_json::from_value(value.clone()).map_err(|error| {
            ToolBusError::EnvelopeAssembly {
                tool: tool.to_owned(),
                reason: error.to_string(),
            }
        });
    }
    for block in &result.content {
        if let Some(text) = block.as_text()
            && let Ok(envelope) = serde_json::from_str::<ToolEnvelope>(&text.text)
        {
            return Ok(envelope);
        }
    }
    Err(ToolBusError::EnvelopeMissing {
        tool: tool.to_owned(),
    })
}

/// `rmcp` 的 `ServiceError` → 本 crate 的错误（保留 JSON-RPC 错误码，便于区分「能力缺失」）。
fn map_service_error(error: rmcp::ServiceError) -> ToolBusError {
    match error {
        rmcp::ServiceError::McpError(data) => ToolBusError::Mcp {
            code: data.code.0,
            message: data.message.to_string(),
        },
        // `ServiceError` 是 `#[non_exhaustive]`：其余变体（传输断开 / 取消 / 超时 …）
        // 一律按可重试的传输失败处理，并把原文带进 `reason`（不丢信息）。
        other => ToolBusError::Transport {
            reason: other.to_string(),
        },
    }
}

/// 初始化 / 关停失败 → [`ToolBusError::Transport`]（泛型以便同时覆盖 client 与 server 的
/// 不同错误类型）。
fn transport_failure<E: std::fmt::Display>(error: E) -> ToolBusError {
    ToolBusError::Transport {
        reason: error.to_string(),
    }
}

/// 关停一个 serve 侧，并把「超时」也当成显式失败。
async fn close_service<R, S>(service: &mut RunningService<R, S>, label: &str) -> ToolBusResult<()>
where
    R: ServiceRole,
    S: Service<R>,
{
    match service.close_with_timeout(SHUTDOWN_TIMEOUT).await {
        Ok(Some(_reason)) => Ok(()),
        Ok(None) => Err(ToolBusError::Transport {
            reason: format!("{label} did not finish shutting down within {SHUTDOWN_TIMEOUT:?}"),
        }),
        Err(error) => Err(ToolBusError::Transport {
            reason: format!("{label} shutdown failed: {error}"),
        }),
    }
}
