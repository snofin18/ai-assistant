//! 审计写入器：hash chain + ring buffer + 单事务批量 flush。
//!
//! 职责：把 [`AuditEvent`] 串进链、缓冲、按 [`Durability`] 决定何时落库；并提供整链校验入口。
//!
//! 边界（不做什么）：
//!   - 不建表（DDL 在 `crates/storage/migrations/0002_audit_logs.sql`，由 storage 的迁移框架应用）
//!   - 不做保留期 / 轮转 / 导出；不做 `detail_json` 外置 blob；不引后台线程
//!   - **不出现任何 `UPDATE` / `DELETE` 语句**（append-only 的第一道锁；库侧触发器是第二道）
//!
//! 不变量：
//!   1. 链尾 = 「缓冲里最后一条的 `self_hash`」，缓冲空时 = 「已落库的最后一条」
//!      —— 因此"缓冲中未落库"的那几条也严格串在一起
//!   2. flush 失败 → 事务回滚 + 缓冲**不清空** + 链尾**不前进**（事件不丢，也不假装写过）
//!   3. `ts` 与"到点判定"都来自注入的 [`Clock`]
//!
//! 相关：架构 v2 §15.1、`docs/storage-design.md` §3.2 / §3.3、`docs/spec/audit-event.md`

use std::sync::Arc;

use rusqlite::{Connection, params};

use assistant_protocol::{AuditActor, AuditEvent};
use assistant_storage::Clock;

use crate::chain::{GENESIS_PREV_HASH, canonical_payload, compute_self_hash};
use crate::durability::Durability;
use crate::error::{AuditError, AuditResult};
use crate::ring_buffer::{PushOutcome, RingBuffer};
use crate::verify::{ChainVerification, verify_rows};

/// 事件关联的任务 / 步骤（对应 `audit_logs.task_id` / `step_id`）。
///
/// 为什么不在 [`AuditEvent`] 上加这两个字段：那是协议 crate 的公共接口（改它需 ADR），
/// 而"哪条审计属于哪个任务"是**本 crate 的装配信息**，不该进协议。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditSubject {
    /// 任务 id；`None` = 会话级 / 系统级事件。
    pub task_id: Option<String>,
    /// 步骤 id；`None` = 不属于某一步（如任务级决策）。
    pub step_id: Option<String>,
}

impl AuditSubject {
    /// 不关联任何任务 / 步骤（会话级 / 系统级事件）。
    #[must_use]
    pub const fn unattached() -> Self {
        Self {
            task_id: None,
            step_id: None,
        }
    }

    /// 关联到某个任务与步骤。
    #[must_use]
    pub fn step(task_id: impl Into<String>, step_id: impl Into<String>) -> Self {
        Self {
            task_id: Some(task_id.into()),
            step_id: Some(step_id.into()),
        }
    }
}

/// `audit_logs` 的一行（`detail_json` 已解析回事件）。
#[derive(Debug, Clone, PartialEq)]
pub struct AuditRecord {
    /// 主键列 `id`（v1 语义 = 本条 `self_hash`）。
    pub id: String,
    /// 链上前一条的 `self_hash`（首条 = [`GENESIS_PREV_HASH`]）。
    pub prev_hash: String,
    /// Unix 毫秒。
    pub ts: i64,
    /// `AuditActor` 的稳定名字（`user` / `agent` / `system` / `tool`）。
    pub actor: String,
    /// 任务 id（可空）。
    pub task_id: Option<String>,
    /// 步骤 id（可空）。
    pub step_id: Option<String>,
    /// 事件类型（`noun.past_verb`，如 `tool.called`）。
    pub event_type: String,
    /// 完整事件（`detail_json` 的解析结果，含 `prev_hash` / `self_hash`）。
    pub event: AuditEvent,
}

/// [`AuditLog::append`] 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendOutcome {
    /// 已进缓冲，**尚未落库**（批量模式下未到点）。
    Buffered {
        /// 缓冲里当前待写的条数。
        pending: usize,
    },
    /// 本次 `append` 触发了 flush（含 `Immediate` 模式的每条）。
    Flushed {
        /// 本次真正写入的行数（≥ 1）。
        written: usize,
    },
}

/// 审计写入器（借用一个写连接；不持有它，也不关闭它）。
pub struct AuditLog<'connection> {
    connection: &'connection Connection,
    clock: Arc<dyn Clock>,
    durability: Durability,
    buffer: RingBuffer<AuditRecord>,
    /// 链上**已落库**的最后一条 `self_hash`（空库 = [`GENESIS_PREV_HASH`]）。
    persisted_hash: String,
    /// 上次 flush 的时钟读数（毫秒）。
    last_flush_ms: i64,
}

impl<'connection> AuditLog<'connection> {
    /// 打开审计写入器：校验 `durability` + 读链尾。
    ///
    /// 幂等：重复调用只重读链尾，不写任何东西。
    ///
    /// # Errors
    /// - [`AuditError::UnsupportedDurability`]：`separate_db_full` 尚未落地
    /// - [`AuditError::InvalidArgument`]：`max_events = 0` 或 `max_interval_ms <= 0`
    /// - [`AuditError::Sqlite`]：读链尾失败（如表不存在 = 迁移没跑）
    pub fn new(
        connection: &'connection Connection,
        clock: Arc<dyn Clock>,
        durability: Durability,
    ) -> AuditResult<Self> {
        match durability {
            Durability::Batched {
                max_events,
                max_interval_ms,
            } => {
                if max_events == 0 {
                    return Err(AuditError::InvalidArgument {
                        field: "max_events",
                        detail:
                            "必须 ≥ 1（0 等于每条都 flush，那种语义请用 Durability::Immediate）"
                                .to_owned(),
                    });
                }
                if max_interval_ms <= 0 {
                    return Err(AuditError::InvalidArgument {
                        field: "max_interval_ms",
                        detail: "必须 ≥ 1（毫秒）".to_owned(),
                    });
                }
            }
            Durability::Immediate => {}
            Durability::SeparateDbFull => {
                return Err(AuditError::UnsupportedDurability {
                    mode: "separate_db_full",
                    detail: "独立审计库 + synchronous=FULL 尚未落地（PL-042）；\
                             本卡只保证 batched / immediate 两档可用"
                        .to_owned(),
                });
            }
        }

        let capacity = match durability {
            Durability::Batched { max_events, .. } => max_events,
            Durability::Immediate | Durability::SeparateDbFull => 1,
        };
        let persisted_hash = load_chain_tail(connection)?;
        let last_flush_ms = clock.now_unix_ms();

        Ok(Self {
            connection,
            clock,
            durability,
            buffer: RingBuffer::with_capacity(capacity),
            persisted_hash,
            last_flush_ms,
        })
    }

    /// 追加一条事件：串链 → 缓冲 → 视 `durability` 决定是否 flush。
    ///
    /// postcondition：返回 [`AppendOutcome::Flushed`] 时事件**已在库中**；返回
    /// [`AppendOutcome::Buffered`] 时事件**只在内存里**（下一次 `append` 到点或显式 `flush` 会写）。
    /// 两条路径都不会"报告成功但什么都没发生"（铁律 1 / 铁律 4）。
    ///
    /// # Errors
    /// - [`AuditError::Json`]：事件无法序列化
    /// - [`AuditError::Sqlite`]：触发了 flush 且写库失败（**缓冲不清空**，事件不丢）
    pub fn append(
        &mut self,
        event: &AuditEvent,
        subject: &AuditSubject,
    ) -> AuditResult<AppendOutcome> {
        let ts = self.clock.now_unix_ms();
        let prev_hash = self.chain_tail().to_owned();

        // 先算出本条 self_hash，再把两个 hash 字段写进事件本身 —— 落库的 detail_json
        // 因此是"权威副本"：读出来就能重算，不必依赖任何外部状态。
        let mut chained = event.clone();
        chained.prev_hash = Some(prev_hash.clone());
        chained.self_hash = String::new();
        let canonical = canonical_payload(&chained)?;
        let self_hash = compute_self_hash(&prev_hash, &canonical);
        chained.self_hash.clone_from(&self_hash);

        let record = AuditRecord {
            id: self_hash,
            prev_hash,
            ts,
            actor: actor_name(chained.actor).to_owned(),
            task_id: subject.task_id.clone(),
            step_id: subject.step_id.clone(),
            event_type: chained.event_type.clone(),
            event: chained,
        };

        let pushed = self.buffer.push(record);
        let must_flush = !self.durability.buffers_events()
            || pushed == PushOutcome::Full
            || self.interval_elapsed();

        if must_flush {
            let written = self.flush()?;
            return Ok(AppendOutcome::Flushed { written });
        }
        Ok(AppendOutcome::Buffered {
            pending: self.buffer.len(),
        })
    }

    /// 把缓冲里的全部事件在**一个事务**里写库。
    ///
    /// 幂等：缓冲为空时返回 `Ok(0)` 且不碰数据库。
    /// **失败不丢事件**：事务回滚 + 缓冲不清空 + 链尾不前进，下一次 `flush` 会重试同一批。
    ///
    /// # Errors
    /// - [`AuditError::Sqlite`]：INSERT 或提交失败
    /// - [`AuditError::Json`]：事件重新序列化失败（理论上不会，`append` 已序列化过一次）
    pub fn flush(&mut self) -> AuditResult<usize> {
        if self.buffer.is_empty() {
            self.last_flush_ms = self.clock.now_unix_ms();
            return Ok(0);
        }

        let transaction = self.connection.unchecked_transaction()?;
        let mut written = 0_usize;
        for record in self.buffer.iter() {
            insert_record(&transaction, record)?;
            written += 1;
        }
        transaction.commit()?;

        if let Some(last) = self.buffer.last() {
            self.persisted_hash.clone_from(&last.id);
        }
        self.buffer.clear();
        self.last_flush_ms = self.clock.now_unix_ms();
        Ok(written)
    }

    /// 缓冲里待写的条数。
    #[must_use]
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    /// 当前耐久性档位。
    #[must_use]
    pub const fn durability(&self) -> Durability {
        self.durability
    }

    /// 已落库链尾的 `self_hash`（空库 = 空串）。
    #[must_use]
    pub fn persisted_hash(&self) -> &str {
        &self.persisted_hash
    }

    /// 读出全部审计行（按插入顺序）。
    ///
    /// # Errors
    /// 读库或 `detail_json` 反序列化失败时返回 [`AuditError::Sqlite`] / [`AuditError::Json`]。
    pub fn records(&self) -> AuditResult<Vec<AuditRecord>> {
        load_records(self.connection)
    }

    /// 重算整条 hash chain。
    ///
    /// 返回**发现**而不是抛错：`is_intact() == false` 表示存在篡改。要求"不自洽即失败"的
    /// 调用方请用 [`AuditLog::verify_chain_strict`]（铁律 1：别让调用方有"忘了看 findings"的机会）。
    ///
    /// # Errors
    /// 读库失败时返回 [`AuditError::Sqlite`] / [`AuditError::Json`]。
    pub fn verify_chain(&self) -> AuditResult<ChainVerification> {
        let rows = load_raw_rows(self.connection)?;
        Ok(verify_rows(&rows))
    }

    /// 同 [`AuditLog::verify_chain`]，但链不自洽时返回 [`AuditError::ChainBroken`]。
    ///
    /// # Errors
    /// 链不自洽（[`AuditError::ChainBroken`]）或读库失败。
    pub fn verify_chain_strict(&self) -> AuditResult<ChainVerification> {
        let verification = self.verify_chain()?;
        if verification.is_intact() {
            Ok(verification)
        } else {
            Err(AuditError::ChainBroken {
                detail: verification.summary(),
            })
        }
    }

    /// 链尾：缓冲里最后一条的 `self_hash`，缓冲空时 = 已落库的链尾。
    fn chain_tail(&self) -> &str {
        self.buffer
            .last()
            .map_or(self.persisted_hash.as_str(), |record| record.id.as_str())
    }

    /// 距上次 flush 是否已超过 `max_interval_ms`（用注入时钟判定）。
    fn interval_elapsed(&self) -> bool {
        match self.durability {
            Durability::Batched {
                max_interval_ms, ..
            } => self.clock.now_unix_ms() - self.last_flush_ms >= max_interval_ms,
            Durability::Immediate | Durability::SeparateDbFull => false,
        }
    }
}

/// `AuditActor` → 稳定列值。
///
/// `AuditActor` 是 `#[non_exhaustive]`：协议新增变体时这里落到 `unknown`。这不是静默失败
/// —— `detail_json` 里存的是**完整事件**（含真实 actor），本列只是给查询用的反规范化副本。
const fn actor_name(actor: AuditActor) -> &'static str {
    match actor {
        AuditActor::User => "user",
        AuditActor::Agent => "agent",
        AuditActor::System => "system",
        AuditActor::Tool => "tool",
        _ => "unknown",
    }
}

const INSERT_RECORD_SQL: &str = "INSERT INTO audit_logs \
    (id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json, hash) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

// 读出顺序用 `rowid`（插入顺序），不是 `ts`：同一毫秒内可以写入多条，`ts` 并列时次序任意。
// 注意：本 crate 的**校验**不依赖行序（`verify.rs` 走链），行序只影响展示与"读链尾"。
const SELECT_RECORDS_SQL: &str = "SELECT id, prev_hash, ts, actor, task_id, step_id, \
    event_type, detail_json FROM audit_logs ORDER BY rowid";

// 校验专用的原始读取：**不**在 SQL 层做 JSON 解析 —— 被篡改的行可能根本不是合法 JSON，
// 那时必须报 `UnreadablePayload` 而不是让整个 `verify_chain` 以 IO 错误告终。
const SELECT_RAW_ROWS_SQL: &str =
    "SELECT id, hash, prev_hash, detail_json FROM audit_logs ORDER BY rowid";

// 链尾 = 最后插入的那一行。为什么可以依赖 `rowid`：本表 append-only（没有任何 DELETE 路径），
// 故 rowid 顺序 == 链顺序。已知限制：`VACUUM` 理论上可能重排无 INTEGER PRIMARY KEY 表的 rowid
// → 见 PL-043；即便真发生，`verify_chain` 也会把分叉暴露成 OrphanedRecord（不会静默）。
const SELECT_CHAIN_TAIL_SQL: &str = "SELECT hash FROM audit_logs ORDER BY rowid DESC LIMIT 1";

/// 普通 `INSERT`（**不是** `INSERT OR REPLACE`）：主键冲突 = 报错，绝不静默覆盖已有审计行。
fn insert_record(connection: &Connection, record: &AuditRecord) -> AuditResult<()> {
    let detail_json = serde_json::to_string(&record.event)?;
    connection.execute(
        INSERT_RECORD_SQL,
        params![
            record.id,
            record.prev_hash,
            record.ts,
            record.actor,
            record.task_id,
            record.step_id,
            record.event_type,
            detail_json,
            // hash 列与 id 列在 v1 语义上同为 self_hash（见卡 §5 DRIFT-013-2）：
            // 两列都写，是为了让"只改其中一列"也能被 verify 检出。
            record.id,
        ],
    )?;
    Ok(())
}

fn load_chain_tail(connection: &Connection) -> AuditResult<String> {
    let mut statement = connection.prepare(SELECT_CHAIN_TAIL_SQL)?;
    let mut rows = statement.query([])?;
    match rows.next()? {
        Some(row) => Ok(row.get(0)?),
        None => Ok(GENESIS_PREV_HASH.to_owned()),
    }
}

fn load_records(connection: &Connection) -> AuditResult<Vec<AuditRecord>> {
    let mut statement = connection.prepare(SELECT_RECORDS_SQL)?;
    let mut rows = statement.query([])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        let detail_json: String = row.get(7)?;
        let event: AuditEvent = serde_json::from_str(&detail_json)?;
        records.push(AuditRecord {
            id: row.get(0)?,
            prev_hash: row.get(1)?,
            ts: row.get(2)?,
            actor: row.get(3)?,
            task_id: row.get(4)?,
            step_id: row.get(5)?,
            event_type: row.get(6)?,
            event,
        });
    }
    Ok(records)
}

/// 校验专用的一行（`detail_json` **原样**，不解析）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAuditRow {
    /// `id` 列。
    pub id: String,
    /// `hash` 列。
    pub hash: String,
    /// `prev_hash` 列。
    pub prev_hash: String,
    /// `detail_json` 列（原样字符串）。
    pub detail_json: String,
}

fn load_raw_rows(connection: &Connection) -> AuditResult<Vec<RawAuditRow>> {
    let mut statement = connection.prepare(SELECT_RAW_ROWS_SQL)?;
    let mut rows = statement.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(RawAuditRow {
            id: row.get(0)?,
            hash: row.get(1)?,
            prev_hash: row.get(2)?,
            detail_json: row.get(3)?,
        });
    }
    Ok(out)
}
