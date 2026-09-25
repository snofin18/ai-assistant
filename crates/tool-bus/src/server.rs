//! in-process MCP server：把「已挂载的内置工具」暴露为 MCP 工具。
//!
//! 为什么用 `rmcp` 的泛型 `ServerHandler`（而不是 `#[tool_router]` 宏）：本项目工具 schema
//! 的**事实源是 `assistant_protocol`（`protocol/**/*.json`）**，不是 Rust 类型。宏路由走
//! schemars 的类型驱动校验，会让同一份 schema 出现第二处定义 —— 正是 ADR-0030 要防的事。
//! 另外宏路由**不做 draft-07 参数校验**（`get_tool` 在泛型路径上从不被调用），
//! 校验必须由本文件显式执行（不变量 1）。
//!
//! ## 返回路径（**所有**结果都是信封）
//!
//! | 情形 | 返回 | `error.code` |
//! |---|---|---|
//! | 工具未挂载 | 失败信封（`structured_error`） | `TargetNotFound` |
//! | 参数不合法 | 失败信封，**不进 handler** | `ToolInvalidArgs` |
//! | handler 报错 | 失败信封 | 该错误的 `error_code()` |
//! | 载荷超预算且截不动 | 失败信封（fail-closed） | `Fatal` |
//! | 成功 | 成功信封（`structured`） | — |
//!
//! 为什么「工具未挂载」也是**工具级**错误而不是 JSON-RPC 协议错：调用方是模型，它看到
//! 可读原因后能自我纠正；把它变成 `Err(McpError)` 会让该信息在客户端侧变成不可解析的
//! 传输错误。唯一的 `Err(McpError)` 是「没有调用身份」——那只可能由总线自己用错引起。
//!
//! 相关：架构 v2 §5.3 / §5.4、`docs/spec/tool-schema.md` §4 不变量 2 / 6。

use std::sync::Arc;

use assistant_protocol::{ErrorCode, ToolEnvelope};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::Value;

use crate::clock::Clock;
use crate::envelope::{EnvelopeRequest, assemble_envelope, error_envelope};
use crate::error::ToolBusError;
use crate::handler::{CallContext, META_KEY_STEP_ID, META_KEY_TASK_ID};
use crate::mount::MountedToolset;
use crate::schema::validate_arguments;

/// 工具通道的 in-process MCP server。
///
/// 生命周期：由 [`ToolBus::start`](crate::ToolBus::start) 创建，与 client 一起跑在同一
/// tokio 运行时里；`MountedToolset` 是**只读**的，因此并发 `tools/call` 之间没有共享可变
/// 状态（工具自身的并发语义由 handler 负责，见 `ToolHandler` 的文档）。
pub struct ToolBusServer {
    mounted: Arc<MountedToolset>,
    default_max_bytes: Option<usize>,
    clock: Arc<dyn Clock>,
}

impl ToolBusServer {
    /// 绑定一个已挂载工具集。
    pub(crate) fn new(
        mounted: Arc<MountedToolset>,
        default_max_bytes: Option<usize>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            mounted,
            default_max_bytes,
            clock,
        }
    }

    /// `tools/call` 的同步实现体。
    ///
    /// 为什么不是 `async`：这条路径上**没有**需要等待的东西 —— 参数校验是纯函数，handler
    /// 是同步 trait 方法。将来接外部 MCP server（阶段 3）时才会出现真实 IO，届时这里再
    /// 变成 `async`（接口形状不变，因为 `ServerHandler` 收的是 `impl Future`）。
    ///
    /// # Errors
    /// [`McpError::invalid_params`]：请求缺少调用身份（`_meta`）—— 只可能是总线用错，
    /// 不可能是模型引起。其余情形**都**以信封形式返回（见模块头的表格）。
    fn handle_call_tool(
        &self,
        request: CallToolRequestParams,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let tool_name = request.name.to_string();
        let call = read_call_context(context).ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "`tools/call` is missing `_meta.{META_KEY_TASK_ID}` / `_meta.{META_KEY_STEP_ID}`; \
                     this server may only be called by the in-process tool bus"
                ),
                None,
            )
        })?;
        let arguments = request.arguments.unwrap_or_default();

        let Some(mounted_tool) = self.mounted.get(&tool_name) else {
            // 未挂载 ≠ 静默失败：模型拿到可读原因（`TargetNotFound`），可以改用别的工具。
            return Ok(error_result(
                &tool_name,
                &call,
                ErrorCode::TargetNotFound,
                &format!("tool `{tool_name}` is not mounted in the current toolset"),
            ));
        };
        let definition = &mounted_tool.definition;

        // 不变量 1：参数不合法**永不进入** handler。
        let violations = validate_arguments(definition.input_schema(), &arguments);
        if !violations.is_empty() {
            return Ok(error_result(
                &tool_name,
                &call,
                ErrorCode::ToolInvalidArgs,
                &violations.join("; "),
            ));
        }

        let started_ms = self.clock.now_unix_ms();
        let output = match mounted_tool.handler.call(&call, &arguments) {
            Ok(output) => output,
            Err(error) => {
                return Ok(error_result(
                    &tool_name,
                    &call,
                    error.error_code(),
                    &error.to_string(),
                ));
            }
        };
        let duration_ms = elapsed_ms(started_ms, self.clock.now_unix_ms());

        let (data, source, evidence, output_max_bytes) = output.into_parts();
        let mut envelope_request =
            EnvelopeRequest::new(tool_name.clone(), call.task_id(), call.step_id(), data)
                .with_metrics(duration_ms, 1);
        if let Some(source) = source {
            envelope_request = envelope_request.untrusted(source);
        }
        if let Some(evidence) = evidence {
            envelope_request = envelope_request.with_evidence(evidence);
        }
        if let Some(max_bytes) = output_max_bytes.or(self.default_max_bytes) {
            envelope_request = envelope_request.with_max_bytes(max_bytes);
        }

        match assemble_envelope(&envelope_request) {
            Ok(envelope) => Ok(ok_result(&envelope)),
            // 组装失败（含「超预算且截不动」）→ 失败信封，绝不放过可疑载荷（铁律 1 / 4）。
            Err(error) => Ok(error_result(
                &tool_name,
                &call,
                error.error_code(),
                &error.to_string(),
            )),
        }
    }

    /// `tools/list` 的同步实现体：把每个已挂载定义转成 MCP `Tool`。
    ///
    /// # Errors
    /// [`McpError::internal_error`]：某个定义转不成 MCP `Tool`（根输入 schema 不是 JSON
    /// 对象）。构造期已拒绝这类输入，因此这是**不可达**路径 —— 保留它是为了不写 `unwrap`
    /// （铁律 1），真发生也必须是显式失败而不是少列一个工具。
    fn handle_list_tools(&self) -> Result<ListToolsResult, McpError> {
        // 先把每个定义转成 MCP `Tool`（转换失败属于内部缺陷），再包成结果。
        let mut tools: Vec<rmcp::model::Tool> = Vec::with_capacity(self.mounted.len());
        for mounted_tool in self.mounted.tools() {
            tools.push(
                mounted_tool
                    .definition
                    .to_mcp_tool()
                    .map_err(|error| internal_error(&error))?,
            );
        }
        // `with_all_items` 而不是 `Default`：后者对 `paginated_result!` 生成的类型会给出
        // 空 vec（内部走的是 `with_all_items(Default::default())`）。
        Ok(ListToolsResult::with_all_items(tools))
    }
}

impl ServerHandler for ToolBusServer {
    /// 通告能力：只开 `tools`。
    ///
    /// 为什么不开 `prompts` / `resources` / `logging`：本项目把「工具」作为唯一能力面
    /// （架构 v2 §5.4），多开一类能力就等于多一条没有治理的旁路。
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("assistant-tool-bus", env!("CARGO_PKG_VERSION")),
        )
    }

    /// `tools/list`：返回当前挂载集（名字升序，`BTreeMap` 保证可复现）。
    ///
    /// 同步实现体在 [`ToolBusServer::handle_list_tools`]；这里只把结果包成 `Future`
    /// （与 `call_tool` 同例：这条路径上没有需要等待的东西）。
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(self.handle_list_tools())
    }

    /// `tools/call`：校验 → 执行 → 组装信封。
    ///
    /// 同步实现体在 [`ToolBusServer::handle_call_tool`]；这里只把结果包成 `Future`
    /// （没有任何需要等待的东西 —— 见那个方法的说明）。
    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, McpError>> + Send + '_ {
        std::future::ready(self.handle_call_tool(request, &context))
    }
}

/// 从请求上下文里取调用身份（总线经 `_meta` 传过来，见 `handler.rs` 的键说明）。
fn read_call_context(context: &RequestContext<RoleServer>) -> Option<CallContext> {
    let meta = &context.meta.0;
    let task_id = meta.get(META_KEY_TASK_ID).and_then(Value::as_str)?;
    let step_id = meta.get(META_KEY_STEP_ID).and_then(Value::as_str)?;
    Some(CallContext::new(task_id, step_id))
}

/// 成功信封 → `CallToolResponse::Complete`（结构化内容 + 同一份 JSON 的文本块，
/// 便于纯文本客户端阅读）。
fn ok_result(envelope: &ToolEnvelope) -> CallToolResponse {
    match serde_json::to_value(envelope) {
        Ok(value) => CallToolResult::structured(value).into(),
        // 协议类型一定能序列化；真到了这里说明它坏了。此时**不能**报成功（铁律 1），
        // 于是退化成显式的纯文本错误。
        Err(error) => CallToolResult::error(vec![ContentBlock::text(format!(
            "failed to serialize the success envelope: {error}"
        ))])
        .into(),
    }
}

/// 失败 → 失败信封 → `CallToolResponse`（**不**返回 `Err`：这是工具级失败，模型必须看到原因）。
fn error_result(
    tool: &str,
    call: &CallContext,
    code: ErrorCode,
    message: &str,
) -> CallToolResponse {
    let envelope = error_envelope(tool, call.task_id(), call.step_id(), code, message);
    match serde_json::to_value(&envelope) {
        Ok(value) => CallToolResult::structured_error(value).into(),
        // `ToolEnvelope` 一定能序列化；真到了这里说明协议类型坏了，此时**不能**假装成功，
        // 退化成纯文本错误（仍然是显式失败，不是静默通过）。
        Err(error) => CallToolResult::error(vec![ContentBlock::text(format!(
            "failed to serialize the error envelope ({error}); original error code: {code:?}"
        ))])
        .into(),
    }
}

/// 本 crate 的错误 → MCP 协议错（只用于**内部缺陷**：schema 转换失败等，不是模型的问题）。
fn internal_error(error: &ToolBusError) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

/// 两个时刻之间的毫秒数；时钟回拨（`end < start`）时记 0，不产生负数 / 不 panic。
fn elapsed_ms(start_ms: i64, end_ms: i64) -> u64 {
    u64::try_from(end_ms.saturating_sub(start_ms)).unwrap_or(0)
}
