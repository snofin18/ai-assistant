//! handler 侧的类型：调用身份、工具实现接口、以及工具返回的载荷。
//!
//! 为什么单独一个模块：这三样东西是**适配器作者**唯一需要读的接口（`crates/tool-bus` 的
//! 其余部分都是通道内部实现）。把「你要实现什么」与「通道怎么装配」分开放，才谈得上
//! 「一眼可懂」。
//!
//! 不变量（与 `lib.rs` 的 6 条一致，这里只重复与 handler 直接相关的）：
//! 1. [`ToolHandler::call`] **只在参数通过 `ToolSchema.input` 校验之后**被调用。
//! 2. handler 的返回值一定会被包进 `assistant_protocol::ToolEnvelope`（成功、失败都是）。
//! 3. handler 只能通过 [`ToolOutput`] 声明「不可信 / 证据 / 预算」，**不能自己截断**
//!    （截断必须走 `envelope::assemble_envelope`，否则「截断不静默」就有第二份实现）。
//!
//! 相关：架构 v2 §5.2 / §5.3、`docs/spec/tool-schema.md` §4。

use serde_json::{Map, Value};

use crate::envelope::{EvidenceDescriptor, SourceDescriptor};
use crate::error::ToolBusResult;

/// `tools/call` 的 `_meta` 键：任务 id。
///
/// 这是**传输细节**：调用身份要跨 MCP 边界送到 server 侧（MCP 的 `tools/call` 请求体里
/// 没有位置放它），而 `_meta` 是 MCP 为扩展信息预留的通道。上层（core / policy）用不到它，
/// 也看不到它。
pub const META_KEY_TASK_ID: &str = "assistant/task_id";

/// `tools/call` 的 `_meta` 键：步骤 id（与 [`META_KEY_TASK_ID`] 同源）。
pub const META_KEY_STEP_ID: &str = "assistant/step_id";

/// 一次工具调用的身份：哪个任务的哪一步。
///
/// 为什么必须显式传递而不是从线程局部 / 全局变量取「当前任务」：同一进程里多个任务并发
/// 使用工具通道，隐式状态在并发下必然串味（铁律 8「element/句柄不得跨进程」的同源动机 ——
/// 上下文必须跟着调用走，不跟着进程走）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallContext {
    task_id: String,
    step_id: String,
}

impl CallContext {
    /// 由任务 id + 步骤 id 构造。
    #[must_use]
    pub fn new(task_id: impl Into<String>, step_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            step_id: step_id.into(),
        }
    }

    /// 任务 id（进信封的 `task_id`）。
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// 步骤 id（进信封的 `step_id`）。
    #[must_use]
    pub fn step_id(&self) -> &str {
        &self.step_id
    }
}

/// 工具实现者要满足的接口。
///
/// 语义：`call` 只在参数已通过 `ToolSchema.input` 校验之后被调用，因此实现者可以假定
/// `arguments` 的**形状**合法；但**不得**假定取值可信 —— 它来自模型，属五类不可信输入之一
/// （铁律 2）。真正的权限判断不在这一层（铁律 3：策略引擎是唯一放行点）。
///
/// 副作用 / 幂等：由具体工具声明（`ToolSchema.idempotent`）。本 crate **不做**重试，
/// 因此实现者不必把 `call` 写成可重入的。
///
/// 并发：必须 `Send + Sync`（同一进程里多任务并发调用同一个工具实例）。
pub trait ToolHandler: Send + Sync {
    /// 执行一次调用。
    ///
    /// # Errors
    /// 任何失败都必须返回 [`ToolBusError`](crate::ToolBusError)；它的
    /// [`error_code`](crate::ToolBusError::error_code) 会成为信封 `error.code`，
    /// 它的 `Display` 文本会成为信封 `error.message`（铁律 1：无静默失败）。
    fn call(&self, call: &CallContext, arguments: &Map<String, Value>)
    -> ToolBusResult<ToolOutput>;
}

/// 闭包也是 handler。
///
/// 为什么给这个 blanket impl：内置工具与测试里大量是「一行逻辑」，为它们各定义一个
/// 结构体只会淹没真正的实现。它不与用户自己的 `impl ToolHandler for X` 冲突 ——
/// 除非 `X` 本身实现了 `Fn(&CallContext, &Map<..>)`（那不是本 crate 的用法）。
impl<F> ToolHandler for F
where
    F: Fn(&CallContext, &Map<String, Value>) -> ToolBusResult<ToolOutput> + Send + Sync,
{
    fn call(
        &self,
        call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> ToolBusResult<ToolOutput> {
        self(call, arguments)
    }
}

/// 工具返回的载荷 + 与载荷同源的元信息（来源 / 证据 / 预算）。
///
/// 边界：这里**不做**截断，只声明预算意图。截断由 `envelope::assemble_envelope` 统一执行，
/// 保证「超限一定带 `truncated.occurred = true`」只有一处实现（不变量 3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    data: Value,
    source: Option<SourceDescriptor>,
    evidence: Option<EvidenceDescriptor>,
    max_bytes: Option<usize>,
}

impl ToolOutput {
    /// 可信载荷（信封 `untrusted = false`）。
    ///
    /// `data` 必须是 JSON 对象或 `null`（`envelope-1.0.json` 的约束）。不满足时会在组装
    /// 信封阶段报 [`ToolBusError::EnvelopeAssembly`](crate::ToolBusError::EnvelopeAssembly)，
    /// 而不是被悄悄包一层。
    #[must_use]
    pub const fn json(data: Value) -> Self {
        Self {
            data,
            source: None,
            evidence: None,
            max_bytes: None,
        }
    }

    /// **不可信**载荷（架构 v2 §12.4）：内容来自应用 / 网页 / 文件 / 剪贴板。
    ///
    /// 为什么来源是必填参数：`untrusted = true` 却没有 `source` 的信封会被
    /// [`assemble_envelope`](crate::assemble_envelope) 拒绝
    /// （[`UntrustedWithoutSource`](crate::ToolBusError::UntrustedWithoutSource)）。
    /// 与其让调用方先构造再失败，不如在类型上就不提供这个组合。
    #[must_use]
    pub const fn untrusted(source: SourceDescriptor, data: Value) -> Self {
        Self {
            data,
            source: Some(source),
            evidence: None,
            max_bytes: None,
        }
    }

    /// 附加证据（树快照 / 截图），供 undo / verify / replay 引用。
    #[must_use]
    pub fn with_evidence(mut self, evidence: EvidenceDescriptor) -> Self {
        self.evidence = Some(evidence);
        self
    }

    /// 覆盖本次调用的载荷预算（字节）；未设则用
    /// [`ToolBusConfig::default_max_bytes`](crate::ToolBusConfig) 的默认值。
    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = Some(max_bytes);
        self
    }

    /// 是否标记为不可信。
    #[must_use]
    pub const fn is_untrusted(&self) -> bool {
        self.source.is_some()
    }

    /// 来源（可信载荷为 `None`）。
    #[must_use]
    pub const fn source(&self) -> Option<&SourceDescriptor> {
        self.source.as_ref()
    }

    /// 证据（未附加为 `None`）。
    #[must_use]
    pub const fn evidence(&self) -> Option<&EvidenceDescriptor> {
        self.evidence.as_ref()
    }

    /// 本次调用的载荷预算覆盖值（未覆盖为 `None`）。
    #[must_use]
    pub const fn max_bytes(&self) -> Option<usize> {
        self.max_bytes
    }

    /// 拆成组装信封需要的四部分（`server.rs` 用；不对外暴露内部字段，避免调用方
    /// 绕过「来源必填」的约束自己拼信封）。
    pub(crate) fn into_parts(
        self,
    ) -> (
        Value,
        Option<SourceDescriptor>,
        Option<EvidenceDescriptor>,
        Option<usize>,
    ) {
        (self.data, self.source, self.evidence, self.max_bytes)
    }
}
