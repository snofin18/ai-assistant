//! hash chain 的独立校验器。
//!
//! 为什么和写入路径分开：校验是"事后取证"的入口，必须能在**不信任写入代码**的前提下单独
//! 读懂、单独测试。本模块只读 [`RawAuditRow`]（`detail_json` 原样字符串），不碰连接、不写任何东西。
//!
//! ## 为什么"走链"而不是"按时间排序后逐行比对"
//!
//! 审计事件的 `ts` 是**毫秒**粒度：同一毫秒内可以写入很多条（批量模式尤其如此），于是
//! `ORDER BY ts` 在并列时的次序是任意的 —— 按"排序后的行序"比对 `prev_hash` 会把**合法的**
//! 审计流误判成断链（实测：固定时钟下 2000 条全部同毫秒）。
//!
//! 正确做法是把链当成**图**：用 `prev_hash → 行` 建索引，从 [`GENESIS_PREV_HASH`] 沿链前进，
//! 走不到的行就是断链。判据与物理行序无关，且能精确定位"中间行被删"。
//!
//! ## 为什么 `detail_json` 不预先解析
//!
//! 被篡改的行可能根本不是合法 JSON。若在读取层就解析，整次校验会以"反序列化失败"告终，
//! 把"这一行被改了"混同于"库读不出来"。这里改为**逐行容错**：解析失败 → `UnreadablePayload`，
//! 其余行继续校验。
//!
//! 相关：`docs/spec/audit-event.md` §4、架构 v2 §15.1

use std::collections::{HashMap, HashSet};

use assistant_protocol::AuditEvent;

use crate::chain::{GENESIS_PREV_HASH, canonical_payload, compute_self_hash};
use crate::log::RawAuditRow;

/// 断链 / 篡改的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TamperKind {
    /// 本行重算的 `self_hash` 不等于存下来的值（**内容被改**），或 `id` / `hash` 两列不一致。
    HashMismatch,
    /// 从 [`GENESIS_PREV_HASH`] 沿链走不到本行（改了 `prev_hash`，或**中间行被删**）。
    OrphanedRecord,
    /// `detail_json` 解析不回事件（行被写坏 / 被外部工具改成非法 JSON）。
    UnreadablePayload,
}

impl TamperKind {
    /// 稳定的机器可读名字（进日志 / 错误信息；**不得随版本改名**）。
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HashMismatch => "hash_mismatch",
            Self::OrphanedRecord => "orphaned_record",
            Self::UnreadablePayload => "unreadable_payload",
        }
    }
}

/// 一处发现。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamperFinding {
    /// 出错行的 `id`。
    pub id: String,
    /// 失败种类。
    pub kind: TamperKind,
    /// 期望值（该种类不适用时为 `None`）。
    pub expected: Option<String>,
    /// 实际值（该种类不适用时为 `None`）。
    pub actual: Option<String>,
}

/// 校验结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainVerification {
    /// 参与校验的行数。
    pub checked: usize,
    /// 从创世沿链可达的行数（正常 = `checked`）。
    pub reachable: usize,
    /// 发现项；**空 = 链自洽**。
    pub findings: Vec<TamperFinding>,
}

impl ChainVerification {
    /// 链是否自洽。
    #[must_use]
    pub const fn is_intact(&self) -> bool {
        self.findings.is_empty()
    }

    /// 一行摘要（进日志 / 错误信息）。
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_intact() {
            return format!("hash chain 自洽：{} 行全部可达", self.checked);
        }
        let kinds: Vec<&str> = self
            .findings
            .iter()
            .map(|finding| finding.kind.name())
            .collect();
        format!(
            "hash chain 断裂：{} 行中仅 {} 行可达，发现 {} 处问题（{}）",
            self.checked,
            self.reachable,
            self.findings.len(),
            kinds.join(",")
        )
    }
}

/// 逐行自校验 + 从创世走链。
pub fn verify_rows(rows: &[RawAuditRow]) -> ChainVerification {
    let mut findings = Vec::new();

    // ① 逐行：id 与 hash 两列必须一致；hash 必须能由 detail_json + prev_hash 重算出来。
    for row in rows {
        if row.id != row.hash {
            findings.push(TamperFinding {
                id: row.id.clone(),
                kind: TamperKind::HashMismatch,
                expected: Some(row.id.clone()),
                actual: Some(row.hash.clone()),
            });
        }
        match serde_json::from_str::<AuditEvent>(&row.detail_json) {
            Ok(event) => match canonical_payload(&event) {
                Ok(canonical) => {
                    let recomputed = compute_self_hash(&row.prev_hash, &canonical);
                    if recomputed != row.hash {
                        findings.push(TamperFinding {
                            id: row.id.clone(),
                            kind: TamperKind::HashMismatch,
                            expected: Some(recomputed),
                            actual: Some(row.hash.clone()),
                        });
                    }
                }
                Err(_) => findings.push(TamperFinding {
                    id: row.id.clone(),
                    kind: TamperKind::UnreadablePayload,
                    expected: None,
                    actual: None,
                }),
            },
            Err(_) => findings.push(TamperFinding {
                id: row.id.clone(),
                kind: TamperKind::UnreadablePayload,
                expected: None,
                actual: None,
            }),
        }
    }

    // ② 建 `prev_hash → 行` 索引。同一个 `prev_hash` 出现两次（人为分叉）时只认第一条，
    //    第二条会在第 ④ 步被判为不可达 —— 分叉必须暴露，不能悄悄选一条走。
    let mut successors: HashMap<&str, &RawAuditRow> = HashMap::with_capacity(rows.len());
    for row in rows {
        successors.entry(row.prev_hash.as_str()).or_insert(row);
    }

    // ③ 从创世沿链前进，标记可达行。
    let mut reachable: HashSet<&str> = HashSet::with_capacity(rows.len());
    let mut cursor = GENESIS_PREV_HASH;
    while let Some(row) = successors.get(cursor) {
        // 人为构造的环会让游标回到已访问的行；此时停下，环上的行会在第 ④ 步被判不可达。
        if !reachable.insert(row.id.as_str()) {
            break;
        }
        cursor = row.id.as_str();
    }

    // ④ 不可达的行 = 断链（中间行被删 / `prev_hash` 被改）。
    for row in rows {
        if !reachable.contains(row.id.as_str()) {
            findings.push(TamperFinding {
                id: row.id.clone(),
                kind: TamperKind::OrphanedRecord,
                expected: None,
                actual: Some(row.prev_hash.clone()),
            });
        }
    }

    ChainVerification {
        checked: rows.len(),
        reachable: reachable.len(),
        findings,
    }
}
