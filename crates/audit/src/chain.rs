//! hash chain 的规范化与计算。
//!
//! 为什么单独一个模块：这是本 crate **唯一**的密码学决策点，必须能单独读、单独测、单独审。
//!
//! 规范化规则（**改这条 = 改契约，需 ADR**）：
//!   1. 把 `self_hash` 置空，其余字段原样（`prev_hash` 保留链上的值）
//!   2. 用 `serde_json::to_string` 序列化：结构体字段顺序 = 声明顺序（固定），
//!      而 `serde_json::Value` 内部的 map 默认是 `BTreeMap`（键有序）→ 结果**确定**
//!   3. `self_hash` = `sha256(HASH_DOMAIN ‖ 0x1F ‖ prev_hash ‖ 0x1F ‖ 规范化 JSON)` 的小写 hex
//!
//! 分隔符 `0x1F`（Unit Separator）的作用：让"`prev_hash` 的尾字节"与"JSON 的首字节"不会被
//! 拼接歧义吃掉（`prev_hash` 是 hex 或空串，理论上无歧义；但显式分隔符让规则不依赖该假设）。
//!
//! 相关：`docs/spec/audit-event.md` §4、架构 v2 §15.1

use sha2::{Digest, Sha256};

use assistant_protocol::AuditEvent;

use crate::error::AuditResult;

/// 首事件的 `prev_hash`（协议约定 = **空串**，不是 `null`）。
pub const GENESIS_PREV_HASH: &str = "";

/// hash 的领域分隔前缀：防止审计 hash 与其它 SHA-256 用途（如 blob 寻址）发生跨协议碰撞。
pub const HASH_DOMAIN: &str = "assistant-audit/v1";

/// 拼接分隔符（Unit Separator）。
const SEPARATOR: u8 = 0x1f;

/// 事件 → 规范化 JSON（`self_hash` 置空，其余字段原样）。
///
/// # Errors
/// 仅当 `serde_json` 序列化失败。`AuditEvent` 不含无法序列化的类型，正常不会发生。
pub fn canonical_payload(event: &AuditEvent) -> AuditResult<String> {
    let mut canonical = event.clone();
    canonical.self_hash = String::new();
    Ok(serde_json::to_string(&canonical)?)
}

/// `prev_hash` + 规范化 JSON → 本条 `self_hash`（小写 hex，64 字符）。
///
/// `prev_hash` 同时出现在规范化 JSON 里（它已是事件的一个字段）；这里再拼一次是**刻意的**：
/// 链指针因此被"显式地"纳入摘要，读代码的人不必先知道 JSON 里有什么。
#[must_use]
pub fn compute_self_hash(prev_hash: &str, canonical_payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN.as_bytes());
    hasher.update([SEPARATOR]);
    hasher.update(prev_hash.as_bytes());
    hasher.update([SEPARATOR]);
    hasher.update(canonical_payload.as_bytes());
    to_hex(&hasher.finalize())
}

/// 字节 → 小写 hex。不用 `format!` 逐字节拼（会分配），也不用索引（workspace 禁 `indexing_slicing`）。
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            // nibble ∈ 0..=15 ⇒ from_digit 必然成功；这里用 if let 而不是 unwrap（workspace 禁 unwrap）
            if let Some(ch) = char::from_digit(u32::from(nibble), 16) {
                out.push(ch);
            }
        }
    }
    out
}
