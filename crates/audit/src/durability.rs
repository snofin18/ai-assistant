//! 审计耐久性档位（`docs/storage-design.md` §3.2 line 106 的配置项
//! `audit.durability = batched | immediate | separate_db_full`）。
//!
//! 边界：本模块**只**是档位的值类型与默认值；不做 IO、不认识连接、不认识 hash。
//!
//! 不变量：
//!   1. 默认档位 = `batched` 100 条 / 200 ms（`docs/storage-design.md` §3.3 line 150 的既定口径）
//!   2. `mode_name()` 是进日志 / 审计的稳定名字，**不得随版本改名**
//!   3. `separate_db_full` 只表达"用户要这个档位"；它是否可用由 [`crate::AuditLog::new`] 判定
//!      （未落地 → 显式报错，绝不静默降级）

/// 批量模式的默认条数上限（`docs/storage-design.md` §3.3：「200 ms 或 100 条」）。
pub const DEFAULT_MAX_EVENTS: usize = 100;

/// 批量模式的默认时间上限（毫秒）。
pub const DEFAULT_MAX_INTERVAL_MS: i64 = 200;

/// 审计写入的耐久性档位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Durability {
    /// 批量：攒够 `max_events` 条、或距上次 flush 超过 `max_interval_ms`，才写一次。
    ///
    /// 崩溃最多丢"最近一个窗口"的事件 —— 高风险动作应改用 [`Durability::Immediate`]
    /// （`docs/storage-design.md` §3.3 line 150 的显式取舍）。
    Batched {
        /// 攒到多少条就 flush。
        max_events: usize,
        /// 距上次 flush 超过多少毫秒就 flush。
        max_interval_ms: i64,
    },
    /// 每条 `append` 立即落库（不缓冲）。慢，但崩溃不丢已确认的事件。
    Immediate,
    /// 独立审计库 + `synchronous = FULL`（`docs/storage-design.md` §3.2）。
    ///
    /// **本卡只解析不落地**：使用它会得到 [`crate::AuditError::UnsupportedDurability`]。
    SeparateDbFull,
}

impl Durability {
    /// 默认档位 = 批量 100 条 / 200 ms。
    #[must_use]
    pub const fn batched_default() -> Self {
        Self::Batched {
            max_events: DEFAULT_MAX_EVENTS,
            max_interval_ms: DEFAULT_MAX_INTERVAL_MS,
        }
    }

    /// 是否需要缓冲（`Immediate` 不缓冲）。
    #[must_use]
    pub const fn buffers_events(self) -> bool {
        matches!(self, Self::Batched { .. })
    }

    /// 机器可读档位名（进日志 / 审计；**不得随版本改名**）。
    #[must_use]
    pub const fn mode_name(self) -> &'static str {
        match self {
            Self::Batched { .. } => "batched",
            Self::Immediate => "immediate",
            Self::SeparateDbFull => "separate_db_full",
        }
    }
}

impl Default for Durability {
    fn default() -> Self {
        Self::batched_default()
    }
}
