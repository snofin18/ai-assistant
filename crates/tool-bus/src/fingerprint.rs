//! 工具集指纹：回答「这一轮模型到底看到了哪些工具、哪一版」。
//!
//! 为什么需要它（架构 v2 §5.5 第 4 条）：审计要能复现「模型当时看到了什么」。只要把
//! 指纹记进会话，事后就能判断某次幻觉调用是不是因为工具集与当时不一致造成的。
//!
//! ## 指纹的组成（**改这条 = 改契约**）
//!
//! 对每个挂载的工具，取 `name` / `schema.version` / `description` / `risk_level` /
//! 规范化后的 `input` / 规范化后的 `output`；按 `name` 升序排成数组后做 SHA-256。
//!
//! - **为什么带 `description`**：描述是模型看到的指令性文本（架构 v2 §5.2 要求给模型看
//!   的说明单独写），改了描述 = 模型看到的东西变了 → 指纹必须变。
//! - **为什么带 `risk_level`**：风险级会改变 policy 的放行行为（TASK-021），属于会话事实。
//! - **规范化 = 递归按 key 排序**：JSON 对象的键序在语义上无意义，必须消除
//!   （`serde_json` 的 map 默认是 `BTreeMap`，但这里显式排序，不依赖实现细节）。
//!   **数组顺序保留**（draft-07 里数组元素有序，例如 `allOf` / `enum` 的书写顺序属于事实）。
//! - **不用 `DefaultHasher`**：它跨进程 / 跨版本不稳定，不能写进审计。
//!
//! 相关：架构 v2 §5.5 第 4 条、`docs/spec/audit-event.md` §4（hash 口径）、ADR-0040。

use assistant_protocol::RiskLevel;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::{ToolBusError, ToolBusResult};

/// 工具集指纹：SHA-256 的小写 hex（64 字符）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolsetFingerprint(String);

impl ToolsetFingerprint {
    /// 指纹的 hex 文本（64 个小写十六进制字符）。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 取出底层字符串。
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ToolsetFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// 参与指纹计算的一个工具视图（借用，避免为算指纹而克隆 schema）。
pub struct FingerprintEntry<'a> {
    pub(crate) name: &'a str,
    pub(crate) version: &'a str,
    pub(crate) description: &'a str,
    pub(crate) risk_level: RiskLevel,
    pub(crate) input: &'a Value,
    pub(crate) output: &'a Value,
}

/// 计算工具集指纹。
///
/// 顺序无关：内部先按 `name` 升序排序再序列化，因此「先挂 A 再挂 B」与「先 B 后 A」同指纹。
///
/// # Errors
/// - [`ToolBusError::FingerprintSerialization`]：`serde_json` 序列化失败
///   （`Value` 不含无法序列化的内容，正常不会发生；保留错误路径是为了不写 `unwrap`）。
pub fn compute_toolset_fingerprint(
    entries: &[FingerprintEntry<'_>],
) -> ToolBusResult<ToolsetFingerprint> {
    let mut manifest: Vec<Value> = entries.iter().map(entry_to_value).collect();
    // 排序键 = name（工具名在注册表里唯一，因此排序是**全序**、结果确定）。
    manifest.sort_by(|left, right| sort_key(left).cmp(sort_key(right)));

    let bytes = serde_json::to_vec(&Value::Array(manifest)).map_err(|error| {
        ToolBusError::FingerprintSerialization {
            reason: error.to_string(),
        }
    })?;
    Ok(ToolsetFingerprint(to_hex(&Sha256::digest(bytes))))
}

/// 单个工具的规范化表示。
fn entry_to_value(entry: &FingerprintEntry<'_>) -> Value {
    let mut object = Map::new();
    object.insert("name".to_owned(), Value::String(entry.name.to_owned()));
    object.insert(
        "version".to_owned(),
        Value::String(entry.version.to_owned()),
    );
    object.insert(
        "description".to_owned(),
        Value::String(entry.description.to_owned()),
    );
    object.insert(
        "risk_level".to_owned(),
        Value::String(risk_level_token(entry.risk_level).to_owned()),
    );
    object.insert("input".to_owned(), canonicalize(entry.input));
    object.insert("output".to_owned(), canonicalize(entry.output));
    Value::Object(object)
}

/// 排序键：`name`（上面 `entry_to_value` 保证一定存在；取不到时退化为空串，
/// 也不会 panic —— 不确定性只可能来自本文件的缺陷）。
fn sort_key(value: &Value) -> &str {
    value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// `risk_level` → 稳定 token。
///
/// 刻意**不**用 serde 序列化：指纹是契约，不能因为将来给枚举加 `#[serde(rename)]` 就漂移。
const fn risk_level_token(risk_level: RiskLevel) -> &'static str {
    match risk_level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Critical => "critical",
        // `RiskLevel` 是 `#[non_exhaustive]`：协议新增风险级时给一个与任何真实取值都
        // 不同的 token（**不**冒充已有级别 —— 冒充会让指纹静默指错版本）。
        _ => "unrecognized-risk-level",
    }
}

/// 递归规范化：对象按 key 升序重建；数组保持顺序（只规范化元素本身）。
fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = Map::new();
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(inner) = object.get(key) {
                    sorted.insert(key.clone(), canonicalize(inner));
                }
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// 字节 → 小写 hex（不用 `format!` 逐字节拼，也不用下标：workspace 禁 `indexing_slicing`）。
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(ch) = char::from_digit(u32::from(nibble), 16) {
                out.push(ch);
            }
        }
    }
    out
}
