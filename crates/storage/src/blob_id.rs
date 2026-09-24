//! blob 的**值类型**：内容寻址标识、用途分类、引用者标识。
//!
//! 为什么与 `content.rs` 分开：本模块**不碰 IO、不碰 SQL**（纯值语义 + 校验），
//! 单独成模块才能让 `content.rs`（blob 池实现）留在 ADR-0033 的 600 行软上限内。
//!
//! 不变量：
//!   1. `BlobId` 恒为 64 位**小写** hex —— 大小写混用会让同内容出现两个 id，去重失效
//!   2. `BlobKind` / `BlobOwner` 的字符串形式是进 DB 的稳定契约，**不得随版本改名**
//!   3. 任何非法输入都返回带 `ErrorCode` 的错误，绝不静默截断 / 转小写 / 降级成 `Other`
//!
//! 相关：`docs/storage-design.md` §3.3、架构 v2 §15.1

use sha2::{Digest, Sha256};

use crate::error::{StorageError, StorageResult};

const SHA256_HEX_LEN: usize = 64;

/// 内容寻址标识 = 内容 sha256 的小写 hex。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlobId(String);

impl BlobId {
    /// 解析一个已有的 `blob_id`。
    ///
    /// # Errors
    /// 长度不是 64 或含非小写 hex 字符时返回 [`StorageError::InvalidBlobId`]。
    pub fn parse(value: &str) -> StorageResult<Self> {
        let is_lower_hex = value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if value.len() != SHA256_HEX_LEN || !is_lower_hex {
            return Err(StorageError::InvalidBlobId {
                value: value.to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// 由内容算出 `blob_id`（**不**写盘、不压缩：id 始终针对**原始**内容）。
    #[must_use]
    pub fn of_content(content: &[u8]) -> Self {
        Self(hex(&Sha256::digest(content)))
    }

    /// 小写 hex 形式。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 相对池根的路径：`<前 2 位>/<完整 64 位>`。
    #[must_use]
    pub fn relative_path(&self) -> String {
        let (prefix, _rest) = self.0.split_at(2);
        format!("{prefix}/{}", self.0)
    }
}

impl std::fmt::Display for BlobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// blob 的用途分类（进 `blobs.kind`，便于人工检查与按类清理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlobKind {
    /// UI 树快照（W4）。
    TreeSnapshot,
    /// 截图（W4）。
    Screenshot,
    /// 其它（后续卡新增用途前先落这里，避免"未知 kind 写不进去"）。
    Other,
}

impl BlobKind {
    /// 存进 DB 的字符串形式（**不得**随版本改名）。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TreeSnapshot => "tree_snapshot",
            Self::Screenshot => "screenshot",
            Self::Other => "other",
        }
    }

    /// 从 DB 字符串解析。
    ///
    /// # Errors
    /// 未知取值时返回 [`StorageError::InvalidArgument`]（不静默降级成 `Other`）。
    pub fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "tree_snapshot" => Ok(Self::TreeSnapshot),
            "screenshot" => Ok(Self::Screenshot),
            "other" => Ok(Self::Other),
            _ => Err(StorageError::InvalidArgument {
                field: "kind",
                detail: format!("未知 blob kind：{value}"),
            }),
        }
    }
}

/// blob 的引用者（谁在用它）：`kind` + `id` 两段式，便于按 owner 批量清理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobOwner {
    kind: String,
    id: String,
}

impl BlobOwner {
    /// 构造引用者标识。
    ///
    /// # Errors
    /// `kind` 或 `id` 为空串时返回 [`StorageError::InvalidArgument`]。
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> StorageResult<Self> {
        let kind = kind.into();
        let id = id.into();
        if kind.is_empty() {
            return Err(StorageError::InvalidArgument {
                field: "owner.kind",
                detail: "不能为空".to_owned(),
            });
        }
        if id.is_empty() {
            return Err(StorageError::InvalidArgument {
                field: "owner.id",
                detail: "不能为空".to_owned(),
            });
        }
        Ok(Self { kind, id })
    }

    /// 引用者类别（如 `step` / `evidence`）。
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// 引用者标识（如 `step_id` / `evidence_id`）。
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// 字节 → 小写 hex（不用 `format!` 逐字节拼，也不用索引）。
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            // nibble ∈ 0..=15 ⇒ from_digit 必然成功（不用 unwrap：workspace 禁 unwrap_used）
            if let Some(ch) = char::from_digit(u32::from(nibble), 16) {
                out.push(ch);
            }
        }
    }
    out
}
