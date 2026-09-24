//! 审计层错误类型 + 到 `ErrorCategory` 的映射。
//!
//! 边界：本模块**只**定义错误类型与两个纯函数（`reason_code` / `error_category`）；
//! 不做 IO、不做 SQL、不做 hash。
//!
//! 不变量：
//!   1. 每个变体都有稳定的 `reason_code()`（`snake_case`）—— 它进日志与审计，**不得随版本漂移**
//!   2. 每个变体都映射到架构 v2 §8.7 的 13 类之一；除"数据库忙/被锁"外一律 `Fatal`
//!      （审计链损坏 = 必须人工介入；把它降级成"自动重试"正是静默失败的开端）
//!   3. 审计层**不发明**新的 `ErrorCategory`
//!
//! 相关：`docs/spec/error-codes.md`、架构 v2 §8.7、`crates/storage/src/error.rs`（同风格）

use std::fmt;

use assistant_protocol::ErrorCategory;

/// 审计层错误。
///
/// `#[non_exhaustive]`：后续卡（保留期轮转 / 独立审计库）会新增变体，匹配方必须保留 `_ =>` 分支。
#[derive(Debug)]
#[non_exhaustive]
pub enum AuditError {
    /// SQLite 自身报错（未细分）。含"触发器拒绝了 UPDATE / DELETE"这一类。
    Sqlite(rusqlite::Error),
    /// `AuditEvent` 的 JSON 序列化 / 反序列化失败。
    Json(serde_json::Error),
    /// 调用方传入了非法参数（`max_events = 0`、`max_interval_ms <= 0` 等）。
    InvalidArgument {
        /// 出错的字段名。
        field: &'static str,
        /// 说明。
        detail: String,
    },
    /// 请求了尚未落地的 `durability` 档位（当前只有 `separate_db_full`）。
    ///
    /// 为什么是**错误**而不是"悄悄按 `batched` 跑"：静默降级会让"每条都已持久"的合规假设
    /// 变成谎言（铁律 1）。真实落地见 `docs/PARKING_LOT.md` PL-042。
    UnsupportedDurability {
        /// 档位名（进日志）。
        mode: &'static str,
        /// 说明。
        detail: String,
    },
    /// hash chain 不自洽（严格模式 [`crate::AuditLog::verify_chain_strict`]）。
    ChainBroken {
        /// 人可读说明（含断链种类与行数）。
        detail: String,
    },
}

impl AuditError {
    /// 稳定的机器可读原因码（`snake_case`）。**不得随版本改名**（它进日志与审计）。
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "sqlite",
            Self::Json(_) => "json",
            Self::InvalidArgument { .. } => "invalid_argument",
            Self::UnsupportedDurability { .. } => "unsupported_durability",
            Self::ChainBroken { .. } => "chain_broken",
        }
    }

    /// 映射到架构 v2 §8.7 的错误分类。
    ///
    /// 为什么除"忙 / 被锁"外一律 `Fatal`：审计错误的默认处置是**人工介入**
    /// —— 事件没写进去 / 链对不上，都属于"证据链可能已损坏"。
    #[must_use]
    pub const fn error_category(&self) -> ErrorCategory {
        match self {
            Self::Sqlite(inner) if is_transient_sqlite(inner) => ErrorCategory::Transient,
            Self::Sqlite(_)
            | Self::Json(_)
            | Self::InvalidArgument { .. }
            | Self::UnsupportedDurability { .. }
            | Self::ChainBroken { .. } => ErrorCategory::Fatal,
        }
    }

    /// 是否需要人工介入（`Fatal` 需要；`Transient` 可由调用方退避重试）。
    #[must_use]
    pub const fn requires_human(&self) -> bool {
        matches!(self.error_category(), ErrorCategory::Fatal)
    }
}

/// 数据库忙 / 被锁是审计层唯一可自动重试的情形（单写者设计下正常不应出现）。
const fn is_transient_sqlite(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => matches!(
            code.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        ),
        _ => false,
    }
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(inner) => write!(f, "SQLite 报错：{inner}"),
            Self::Json(inner) => write!(f, "审计事件 JSON 报错：{inner}"),
            Self::InvalidArgument { field, detail } => {
                write!(f, "参数 {field} 非法：{detail}")
            }
            Self::UnsupportedDurability { mode, detail } => {
                write!(f, "durability={mode} 尚未落地：{detail}")
            }
            Self::ChainBroken { detail } => write!(f, "审计 hash chain 断裂：{detail}"),
        }
    }
}

impl std::error::Error for AuditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(inner) => Some(inner),
            Self::Json(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for AuditError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<serde_json::Error> for AuditError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// 审计层结果别名。
pub type AuditResult<T> = Result<T, AuditError>;
