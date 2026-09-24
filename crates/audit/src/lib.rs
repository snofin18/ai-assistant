//! # assistant-audit crate（TASK-013）
//!
//! 阶段 1A1 基础设施：**追加不可改的审计流**。把 [`assistant_protocol::AuditEvent`] 写进 L1 的
//! `audit_logs` 表，用 SHA-256 hash chain 让篡改可检测，并用 ring buffer 批量 flush 把 fsync
//! 摊薄到 **< 1 ms/条**。
//!
//! ## 职责
//!
//! - 迁移 `0002_audit_logs`：表 + `idx_audit_ts` + **数据库侧** append-only 触发器
//! - [`AuditLog`]：`append`（串链）→ 缓冲 → 单事务批量 `flush`
//! - [`Durability`]：`batched`（默认 100 条 / 200 ms）/ `immediate` / `separate_db_full`
//! - [`AuditLog::verify_chain`]：重算整条链，检出「改内容 / 改链指针 / 删中间行」
//!
//! ## 边界（不做什么）
//!
//! - **不做审批 / 放行判定**：审计只**记录**发生过什么，判定在 policy（铁律 6）
//! - 不做保留期 / 容量轮转 / 合规导出、不做 `detail_json` 外置 blob（后续卡）
//! - **`separate_db_full` 只解析不落地**：调用即返回 [`AuditError::UnsupportedDurability`]，
//!   绝不静默降级成 `batched`（铁律 1）；真实落地见 PL-042
//! - 不引后台线程 / 定时器（flush 由 `append` 驱动，故不依赖任何 async 运行时）
//! - 不调任何平台 API；不定义第二份事件结构体（载荷类型来自 `crates/protocol`，铁律 10）
//!
//! ## 不变量
//!
//! 1. **只追加**：本 crate 内没有任何 `UPDATE` / `DELETE` 语句；库侧另有触发器兜底
//! 2. **链不可断**：`self_hash = sha256(domain ‖ prev_hash ‖ 规范化 JSON)`，规范化 JSON 里
//!    `self_hash` 置空 —— 改任何一字节都会让 `verify_chain` 报错
//! 3. **flush 要么全成要么全不成**：单事务；失败时缓冲**不清空**、链尾**不前进**（不丢事件，
//!    也不假装写过）
//! 4. **时钟注入**：`ts` 与「200 ms 到点」都取自 [`assistant_storage::Clock`]（测试可回放）
//! 5. **`id` = 本条 `self_hash`**：链位置 + 内容共同决定，天然唯一（见本卡 §5 DRIFT-013-2）
//!
//! ## 典型用法
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use assistant_audit::{AuditLog, Durability};
//! use assistant_storage::{Database, StoragePaths, SystemClock};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let database = Database::open(&StoragePaths::new("D:/data/assistant"), Arc::new(SystemClock))?;
//! let mut log = AuditLog::new(database.connection(), database.clock(), Durability::default())?;
//!
//! // 典型写入路径：log.append(&event, &AuditSubject::unattached())?
//! // —— 攒满 100 条或距上次 flush 超过 200 ms 时自动落库
//! assert_eq!(log.flush()?, 0); // 缓冲为空时 flush 是 no-op（不会"报告成功但什么都没发生"）
//! assert!(log.verify_chain()?.is_intact());
//! # Ok(())
//! # }
//! ```
//!
//! ## 相关 spec / 文档
//!
//! 架构 v2 §15.1（表结构）/ §15.3（加密与保留）、`docs/storage-design.md` §3.2 / §3.3 / §4、
//! `docs/spec/audit-event.md`、`docs/spec/error-codes.md`、
//! `tasks/TASK-013-audit-append-hash-chain-flush.md`（本卡正文 + 执行记录）。

#![deny(unsafe_code)]

mod chain;
mod durability;
mod error;
mod log;
mod ring_buffer;
mod verify;

pub use chain::{GENESIS_PREV_HASH, HASH_DOMAIN, canonical_payload, compute_self_hash};
pub use durability::{DEFAULT_MAX_EVENTS, DEFAULT_MAX_INTERVAL_MS, Durability};
pub use error::{AuditError, AuditResult};
pub use log::{AppendOutcome, AuditLog, AuditRecord, AuditSubject};
pub use ring_buffer::{PushOutcome, RingBuffer};
pub use verify::{ChainVerification, TamperFinding, TamperKind};
