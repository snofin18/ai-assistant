//! 密钥访问审计：记录的形状 + 写出接口（注入点）。
//!
//! 边界：本模块**只**定义记录与接口 —— **不**定义协议事件类型、**不**落库、**不**做策略判定。
//! 把记录映射成 `assistant_protocol::AuditEvent` 需要新增 `event_type` 取值（当前 17 项封闭枚举
//! 里没有 `secret.*`）= 改 schema（漂移触发器 ③ / ⑩）→ 归后续卡（见任务卡 §5 的 `DRIFT-014-1`）。
//!
//! 不变量：
//!   1. [`SecretAccessRecord`] **结构上不可能**携带明文：字段只有名字 / 操作 / 结果 / 时间戳
//!   2. 名字是**引用不是秘密**（架构 v2 L2624 的 `key_ref`），进日志与审计是合规的
//!   3. 时间戳来自注入的 [`assistant_storage::Clock`]（AGENTS.md §5.3），测试可回放
//!
//! 相关：架构 v2 §12.5（「密钥访问本身要审计」）

use std::fmt;

use crate::SecretName;
use crate::error::SecretResult;

/// 密钥访问操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretAccessOperation {
    /// 读（含"存在性探测"）。
    Read,
    /// 写（含覆盖）。
    Write,
    /// 删。
    Delete,
}

impl SecretAccessOperation {
    /// 稳定的机器可读名（进审计记录）。**不得随版本改名**。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Delete => "delete",
        }
    }
}

impl fmt::Display for SecretAccessOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 访问结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretAccessOutcome {
    /// 访问已执行（读到了 / 写成功 / 删掉了一条）。
    Allowed,
    /// 条目不存在（读不到 / 没删掉任何东西）。
    NotFound,
    /// 被上层策略拒绝 —— 本 crate **不做**策略判定（铁律 6），该取值留给未来的 policy 层。
    Denied,
    /// 后端报错。
    Failed,
}

impl SecretAccessOutcome {
    /// 稳定的机器可读名（进审计记录）。**不得随版本改名**。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::NotFound => "not_found",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for SecretAccessOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 一条密钥访问记录。
///
/// **刻意没有** value / plaintext / content 字段 —— 明文进不了审计（架构 v2 §12.5）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretAccessRecord {
    name: SecretName,
    operation: SecretAccessOperation,
    outcome: SecretAccessOutcome,
    ts_unix_ms: i64,
}

impl SecretAccessRecord {
    /// 构造。
    #[must_use]
    pub const fn new(
        name: SecretName,
        operation: SecretAccessOperation,
        outcome: SecretAccessOutcome,
        ts_unix_ms: i64,
    ) -> Self {
        Self {
            name,
            operation,
            outcome,
            ts_unix_ms,
        }
    }

    /// 被访问的密钥名（引用，不是秘密）。
    #[must_use]
    pub const fn name(&self) -> &SecretName {
        &self.name
    }

    /// 操作。
    #[must_use]
    pub const fn operation(&self) -> SecretAccessOperation {
        self.operation
    }

    /// 结果。
    #[must_use]
    pub const fn outcome(&self) -> SecretAccessOutcome {
        self.outcome
    }

    /// 时间戳（Unix 毫秒，来自注入的时钟）。
    #[must_use]
    pub const fn ts_unix_ms(&self) -> i64 {
        self.ts_unix_ms
    }
}

impl fmt::Display for SecretAccessRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "secret.{} {} name={} ts={}",
            self.outcome, self.operation, self.name, self.ts_unix_ms
        )
    }
}

/// 审计写出接口（注入点）。
///
/// 实现方（后续卡的装配点）负责把记录翻译成协议事件并落进 `assistant-audit`。
pub trait SecretAccessAuditor: Send + Sync {
    /// 写出一条访问记录。
    ///
    /// # Errors
    /// 写出失败 → `Err`。**调用方必须把它当成"这次访问被拒绝"**（fail-closed，见
    /// [`crate::AuditedSecretStore`]）。
    fn record_access(&self, record: &SecretAccessRecord) -> SecretResult<()>;
}
