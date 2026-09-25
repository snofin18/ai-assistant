//! 本 crate 的**唯一**错误出口：每个变体都能映射到一个 `assistant_protocol::ErrorCode`。
//!
//! 为什么每个变体都要能映射：铁律 1「无静默失败」要求失败**带 `ErrorCode`**。上层
//! （core / policy / model-gateway）不需要理解本 crate 的内部错误类型，只需要
//! [`ToolBusError::error_code`] 给出的 13 类之一。
//!
//! 相关：`docs/spec/error-codes.md`、架构 v2 §8.7 / §5.3。

use assistant_protocol::ErrorCode;

/// 本 crate 的 `Result` 别名。
pub type ToolBusResult<T> = Result<T, ToolBusError>;

/// JSON-RPC / MCP 的「method not found」错误码。
///
/// 单独抽成常量是为了不让 `-32601` 这个魔数散落在映射逻辑里，并让「方法不存在 =
/// 能力缺失（不可重试）」这条判断有唯一出处。
const MCP_METHOD_NOT_FOUND: i32 = -32601;

/// 工具通道的错误分类。
///
/// 分类口径（为什么这样分，而不是一律 `Fatal`）：
/// - **注册期契约违反**（重名 / 名字不合规 / schema 无法强制 / 注解与风险级冲突）→
///   [`ErrorCode::Fatal`]。这是**工具作者**的缺陷；把它报成 `ToolInvalidArgs` 会让上层
///   去"重试 + 改参数"，永远修不好。
/// - **调用参数非法** → [`ErrorCode::ToolInvalidArgs`]（且**不进入** handler）。
/// - **工具不存在 / 未挂载** → [`ErrorCode::TargetNotFound`]（"目标找不到"的同类语义）。
/// - **传输抖动** → [`ErrorCode::Transient`]（可重试）；MCP `method not found` 是
///   [`ErrorCode::CapabilityMissing`]（不可重试）。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolBusError {
    /// 同名工具被重复注册。
    #[error("tool `{tool}` is already registered")]
    DuplicateTool {
        /// 工具名。
        tool: String,
    },

    /// 工具名不符合 `<app>.<domain>.<action>`（naming.md §7 / tool-schema 不变量 1）。
    #[error("tool name `{name}` must match `<app>.<domain>.<action>` (naming.md §7)")]
    InvalidToolName {
        /// 违规的工具名。
        name: String,
    },

    /// 工具 schema 用了本 crate **无法强制**的构造 → 拒绝注册（fail-closed）。
    #[error(
        "tool `{tool}` declares schema constraints the tool bus cannot enforce (fail-closed): {problems:?}"
    )]
    UnsupportedSchemaKeyword {
        /// 工具名。
        tool: String,
        /// 逐条列出的问题（JSON pointer + 关键字）。
        problems: Vec<String>,
    },

    /// `risk_level` 低于 MCP 注解推导出的下界（架构 v2 §5.2 的双向映射）。
    #[error(
        "tool `{tool}`: risk_level={declared} understates its MCP annotations ({required}); see arch v2 §5.2"
    )]
    RiskAnnotationMismatch {
        /// 工具名。
        tool: String,
        /// 声明值。
        declared: String,
        /// 由注解推导出的下界。
        required: String,
    },

    /// 调用参数不符合工具 schema。
    #[error("arguments for tool `{tool}` are invalid: {reason}")]
    InvalidArguments {
        /// 工具名。
        tool: String,
        /// 人可读的原因（JSON pointer + 违反了哪条约束）。
        reason: String,
    },

    /// 工具未在当前工具集中挂载。
    #[error("tool `{tool}` is not mounted in the current toolset")]
    ToolNotMounted {
        /// 工具名。
        tool: String,
    },

    /// 挂载选择引用了未注册的工具名（或没有任何工具匹配某个 app 前缀）。
    #[error("mount selection names unknown tool `{tool}`")]
    UnknownTool {
        /// 被引用的工具名 / `<app>.*`。
        tool: String,
    },

    /// 载荷超预算，且按契约无法再截断（fail-closed：绝不放过超限载荷）。
    #[error(
        "tool `{tool}`: {actual_bytes} bytes exceeds the {max_bytes}-byte budget and cannot be truncated further (fail-closed)"
    )]
    PayloadTooLarge {
        /// 工具名。
        tool: String,
        /// 截断后仍然超出的大小（字节）。
        actual_bytes: usize,
        /// 声明的预算（字节）。
        max_bytes: usize,
    },

    /// `untrusted = true` 但没有给出 `source`（envelope 不变量：不可信内容必须有来源）。
    #[error("tool `{tool}` marked its payload untrusted without a source kind")]
    UntrustedWithoutSource {
        /// 工具名。
        tool: String,
    },

    /// 信封组装失败（本 crate 组装失败 = 内部缺陷，不是模型的问题）。
    #[error("envelope assembly failed for tool `{tool}`: {reason}")]
    EnvelopeAssembly {
        /// 工具名。
        tool: String,
        /// 原因（serde 的报错文本）。
        reason: String,
    },

    /// MCP 结果里没有信封。
    #[error("tool `{tool}` returned no ToolEnvelope (no structuredContent, no JSON text block)")]
    EnvelopeMissing {
        /// 工具名。
        tool: String,
    },

    /// 工具声明（`ToolSchema`）组装失败（内部缺陷）。
    ///
    /// 为什么需要这条：`assistant_protocol` 的生成类型全部 `#[non_exhaustive]` 且
    /// **没有构造器**（DRIFT-020-3），本 crate 只能经 serde 组装 `ToolSchema`，
    /// 于是必须有一条失败出口 —— 而不是 `unwrap`。
    #[error("tool `{tool}`: failed to assemble its ToolSchema: {reason}")]
    SchemaAssembly {
        /// 工具名。
        tool: String,
        /// 原因（serde 的报错文本）。
        reason: String,
    },

    /// 审计事件组装失败（内部缺陷）。
    #[error("audit event assembly failed: {reason}")]
    AuditEventAssembly {
        /// 原因（serde 的报错文本）。
        reason: String,
    },

    /// 指纹计算失败（内部缺陷：`Value` 序列化理论上不会失败）。
    #[error("toolset fingerprint serialization failed: {reason}")]
    FingerprintSerialization {
        /// 原因（serde 的报错文本）。
        reason: String,
    },

    /// 传输 / 协议层失败（可重试）。
    #[error("MCP transport failure: {reason}")]
    Transport {
        /// 原因文本。
        reason: String,
    },

    /// MCP 协议错误（来自对端 JSON-RPC error）。
    #[error("MCP error {code}: {message}")]
    Mcp {
        /// JSON-RPC 错误码。
        code: i32,
        /// 错误消息。
        message: String,
    },
}

impl ToolBusError {
    /// 映射到协议错误码（`assistant_protocol::ErrorCode`）。
    ///
    /// 幂等 / 无副作用：纯函数，只读 `self`。
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::DuplicateTool { .. }
            | Self::InvalidToolName { .. }
            | Self::UnsupportedSchemaKeyword { .. }
            | Self::RiskAnnotationMismatch { .. }
            | Self::PayloadTooLarge { .. }
            | Self::UntrustedWithoutSource { .. }
            | Self::SchemaAssembly { .. }
            | Self::EnvelopeAssembly { .. }
            | Self::EnvelopeMissing { .. }
            | Self::AuditEventAssembly { .. }
            | Self::FingerprintSerialization { .. } => ErrorCode::Fatal,
            Self::InvalidArguments { .. } => ErrorCode::ToolInvalidArgs,
            Self::ToolNotMounted { .. } | Self::UnknownTool { .. } => ErrorCode::TargetNotFound,
            Self::Transport { .. } => ErrorCode::Transient,
            Self::Mcp { code, .. } => {
                if *code == MCP_METHOD_NOT_FOUND {
                    ErrorCode::CapabilityMissing
                } else {
                    ErrorCode::Transient
                }
            }
        }
    }
}
