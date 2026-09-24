//! 密钥层错误类型 + 到 `ErrorCategory` 的映射。
//!
//! 边界：本模块**只**定义错误类型与三个纯函数（`reason_code` / `error_category` / `requires_human`）；
//! 不做 IO、不碰 `keyring`、不碰文件系统。
//!
//! 不变量：
//!   1. 每个变体都有稳定的 `reason_code()`（`snake_case`）—— 它进日志与审计，**不得随版本漂移**
//!   2. 每个变体都映射到架构 v2 §8.7 的 13 类之一
//!   3. **「没有这条密钥」不是错误**：它是 `Ok(None)`（见 [`crate::SecretStore::get`]）。本枚举
//!      刻意**没有** `NotFound` 变体 —— 一旦加回来，调用方就会开始用 `Err` 表达"未配置"，
//!      铁律 1 要求的「没有」与「读失败」分辨率随之丢失
//!   4. **错误里永远不含明文**：所有 `detail` 只放平台报错文本与字段名，不放密钥内容
//!
//! 相关：`docs/spec/error-codes.md`、架构 v2 §8.7 / §12.5、`crates/storage/src/error.rs`（同风格）

use std::fmt;

use assistant_protocol::ErrorCategory;

/// 密钥层错误。
///
/// `#[non_exhaustive]`：后续卡（轮换 / 配额 / BYOK 装配）会新增变体，匹配方必须保留 `_ =>` 分支。
#[derive(Debug)]
#[non_exhaustive]
pub enum SecretError {
    /// 密钥名不合法（空 / 含 NUL / 超长 / 字符集外）。
    InvalidName {
        /// 说明（**不含**密钥内容）。
        detail: String,
    },
    /// 密钥值不合法（超过长度上限）。
    InvalidValue {
        /// 说明（**不含**密钥内容）。
        detail: String,
    },
    /// 平台密钥库不可用：无默认后端、平台不受支持、或 Secret Service 未运行。
    KeychainUnavailable {
        /// 平台报错文本。
        detail: String,
    },
    /// 平台密钥库存在但拒绝访问（密钥库被锁、权限不足）。
    AccessDenied {
        /// 平台报错文本。
        detail: String,
    },
    /// 当前后端不支持该操作。
    UnsupportedOperation {
        /// 说明。
        detail: String,
    },
    /// 后端运行期失败（未细分）。
    BackendFailure {
        /// 平台报错文本。
        detail: String,
    },
    /// 取回的条目内容已损坏（非 UTF-8 / 格式不符 / 库本身格式异常）。
    CorruptEntry {
        /// 说明。
        detail: String,
    },
    /// 审计写不进去 → 该次存取被拒绝（fail-closed）。
    ///
    /// 为什么是**错误**而不是"先放行、审计失败只记警告"：架构 v2 §12.5 要求「密钥访问本身要审计」。
    /// 放行一次没被记录的密钥访问 = 审计承诺变成谎言（铁律 1）。
    AuditRejected {
        /// 说明（含被拒绝的操作与审计层报错文本，**不含**密钥内容）。
        detail: String,
    },
}

impl SecretError {
    /// 稳定的机器可读原因码（`snake_case`）。**不得随版本改名**（它进日志与审计）。
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::InvalidName { .. } => "invalid_name",
            Self::InvalidValue { .. } => "invalid_value",
            Self::KeychainUnavailable { .. } => "keychain_unavailable",
            Self::AccessDenied { .. } => "access_denied",
            Self::UnsupportedOperation { .. } => "unsupported_operation",
            Self::BackendFailure { .. } => "backend_failure",
            Self::CorruptEntry { .. } => "corrupt_entry",
            Self::AuditRejected { .. } => "audit_rejected",
        }
    }

    /// 映射到架构 v2 §8.7 的错误分类。
    ///
    /// 为什么"名字不合法"归 `Fatal` 而不归 `ToolInvalidArgs`：后者按 §8.7 是**模型输出**的
    /// 参数校验失败（可重新提示模型）；密钥名来自配置与调用方代码，出错属于程序缺陷 ——
    /// 与 `StorageError::InvalidArgument` 同判（`crates/storage/src/error.rs`）。
    #[must_use]
    pub const fn error_category(&self) -> ErrorCategory {
        match self {
            Self::KeychainUnavailable { .. } | Self::AccessDenied { .. } => {
                ErrorCategory::PlatformPermission
            }
            Self::UnsupportedOperation { .. } => ErrorCategory::CapabilityMissing,
            Self::InvalidName { .. }
            | Self::InvalidValue { .. }
            | Self::BackendFailure { .. }
            | Self::CorruptEntry { .. }
            | Self::AuditRejected { .. } => ErrorCategory::Fatal,
        }
    }

    /// 是否需要人工介入。true = 按架构 v2 §8.7 归 Fatal，必须人工介入（本 crate 目前没有可自动重试的变体）。
    #[must_use]
    pub const fn requires_human(&self) -> bool {
        matches!(self.error_category(), ErrorCategory::Fatal)
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { detail } => write!(f, "密钥名不合法：{detail}"),
            Self::InvalidValue { detail } => write!(f, "密钥值不合法：{detail}"),
            Self::KeychainUnavailable { detail } => write!(f, "OS 密钥库不可用：{detail}"),
            Self::AccessDenied { detail } => write!(f, "OS 密钥库拒绝访问：{detail}"),
            Self::UnsupportedOperation { detail } => write!(f, "后端不支持该操作：{detail}"),
            Self::BackendFailure { detail } => write!(f, "OS 密钥库报错：{detail}"),
            Self::CorruptEntry { detail } => write!(f, "密钥条目已损坏：{detail}"),
            Self::AuditRejected { detail } => {
                write!(f, "审计写不进去，该次密钥访问已被拒绝：{detail}")
            }
        }
    }
}

impl std::error::Error for SecretError {}

/// 密钥层结果别名。
pub type SecretResult<T> = Result<T, SecretError>;
