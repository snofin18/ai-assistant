//! 存储层错误类型 + 到 `ErrorCategory` 的映射。
//!
//! 边界：本模块**只**定义错误类型与两个纯函数（`reason_code` / `error_category`）；
//! 不做 IO、不做 SQL。
//!
//! 不变量：
//!   1. 每个变体都有稳定的 `reason_code()`（`snake_case`）—— 它进日志与审计，**不得随版本漂移**
//!   2. 每个变体都映射到架构 v2 §8.7 的 13 类之一；除"数据库忙/被锁"外一律 `Fatal`
//!      （v2 §8.7 对 `Fatal` 的定义即 "Host crashed, storage corrupted"）
//!   3. 存储层不发明新的 `ErrorCategory`：`evidence_missing` / `evidence_corrupt` 是
//!      **`reason_code`**（细粒度），分类仍是 `Fatal`（粗粒度）—— 与 `docs/storage-design.md` §8 一致
//!
//! 相关：`docs/spec/error-codes.md`、架构 v2 §8.7、`docs/storage-design.md` §8

use std::fmt;
use std::path::PathBuf;

use assistant_protocol::ErrorCategory;

/// 存储层错误。
///
/// `#[non_exhaustive]`：后续卡（TASK-013 审计、TASK-028 FTS5）会新增变体，
/// 匹配方必须保留 `_ =>` 分支。
#[derive(Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// 库里的 schema 版本与二进制期望值不一致。
    ///
    /// 三种触发：① 有业务表但没有版本表（外来/被篡改的库）② 版本高于二进制（降级启动）
    /// ③ 版本表里有二进制不认识的版本号。三种都**拒绝启动**，不做任何"自动修好"。
    SchemaVersionMismatch {
        /// 二进制期望的版本。
        expected: i64,
        /// 库里读到的版本；没有版本表时为 `None`。
        found: Option<i64>,
        /// 人可读说明（进错误日志）。
        detail: String,
    },
    /// 已应用的迁移内容与记账的 sha256 不一致 = 迁移文件被事后改动。
    MigrationChecksumMismatch {
        /// 迁移版本号。
        version: i64,
        /// 记账时的 sha256（`schema_migrations.checksum`）。
        recorded: String,
        /// 当前文件的 sha256。
        actual: String,
    },
    /// 迁移执行失败；该迁移的事务已回滚，库停在迁移前的版本。
    MigrationFailed {
        /// 迁移版本号。
        version: i64,
        /// SQLite 的报错文本。
        detail: String,
    },
    /// SQLite 自身报错（未细分）。
    Sqlite(rusqlite::Error),
    /// 文件系统报错（带路径，便于定位）。
    Io {
        /// 出错的路径。
        path: PathBuf,
        /// 底层 IO 错误。
        source: std::io::Error,
    },
    /// 元数据表里有该 blob，但文件不在 —— `storage-design.md` §8 的 `evidence_missing`。
    EvidenceMissing {
        /// blob 标识（sha256 小写 hex）。
        blob_id: String,
        /// 期望存在但缺失的绝对路径。
        path: PathBuf,
    },
    /// blob 内容与 `blob_id` 不符 —— `storage-design.md` §8 的 `evidence_corrupt`。
    EvidenceCorrupt {
        /// blob 标识。
        blob_id: String,
        /// 说明（长度不符 / sha256 不符 / 解压失败）。
        detail: String,
    },
    /// 元数据表里没有这个 blob（从未登记，或已被 GC 回收）。
    BlobUnknown {
        /// blob 标识。
        blob_id: String,
    },
    /// 调用方传入了非法参数（空字符串、超出范围等）。
    InvalidArgument {
        /// 出错的字段名。
        field: &'static str,
        /// 说明。
        detail: String,
    },
    /// `blob_id` 不是 64 位小写 hex 的 sha256。
    InvalidBlobId {
        /// 调用方给的原值。
        value: String,
    },
    /// zstd 压缩 / 解压失败。
    Compression {
        /// 说明。
        detail: String,
    },
}

impl StorageError {
    /// 稳定的机器可读原因码（`snake_case`）。**不得随版本改名**（它进日志与审计）。
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::SchemaVersionMismatch { .. } => "schema_version_mismatch",
            Self::MigrationChecksumMismatch { .. } => "migration_checksum_mismatch",
            Self::MigrationFailed { .. } => "migration_failed",
            Self::Sqlite(_) => "sqlite",
            Self::Io { .. } => "io",
            Self::EvidenceMissing { .. } => "evidence_missing",
            Self::EvidenceCorrupt { .. } => "evidence_corrupt",
            Self::BlobUnknown { .. } => "blob_unknown",
            Self::InvalidArgument { .. } => "invalid_argument",
            Self::InvalidBlobId { .. } => "invalid_blob_id",
            Self::Compression { .. } => "compression",
        }
    }

    /// 映射到架构 v2 §8.7 的错误分类。
    ///
    /// 为什么除"忙/被锁"外一律 `Fatal`：存储层错误的默认处置是**人工介入**。
    /// 把"数据可能不一致"降级成"自动重试"正是静默失败的开端（铁律 1）。
    #[must_use]
    pub const fn error_category(&self) -> ErrorCategory {
        match self {
            Self::Sqlite(inner) if is_transient_sqlite(inner) => ErrorCategory::Transient,
            Self::Sqlite(_)
            | Self::SchemaVersionMismatch { .. }
            | Self::MigrationChecksumMismatch { .. }
            | Self::MigrationFailed { .. }
            | Self::Io { .. }
            | Self::EvidenceMissing { .. }
            | Self::EvidenceCorrupt { .. }
            | Self::BlobUnknown { .. }
            | Self::InvalidArgument { .. }
            | Self::InvalidBlobId { .. }
            | Self::Compression { .. } => ErrorCategory::Fatal,
        }
    }

    /// 是否需要人工介入（`Fatal` 需要；`Transient` 可由调用方退避重试）。
    #[must_use]
    pub const fn requires_human(&self) -> bool {
        matches!(self.error_category(), ErrorCategory::Fatal)
    }
}

/// 数据库忙 / 被锁是存储层唯一可自动重试的情形（单写者设计下正常不应出现）。
const fn is_transient_sqlite(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => matches!(
            code.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        ),
        _ => false,
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaVersionMismatch {
                expected,
                found,
                detail,
            } => write!(
                f,
                "schema 版本不匹配：期望 {expected}，库里读到 {found:?}；{detail}"
            ),
            Self::MigrationChecksumMismatch {
                version,
                recorded,
                actual,
            } => write!(
                f,
                "迁移 {version} 的内容与记账不符（记账 {recorded}，实际 {actual}）—— 迁移文件被事后改动"
            ),
            Self::MigrationFailed { version, detail } => {
                write!(f, "迁移 {version} 执行失败：{detail}")
            }
            Self::Sqlite(inner) => write!(f, "SQLite 报错：{inner}"),
            Self::Io { path, source } => write!(f, "IO 报错 {}：{source}", path.display()),
            Self::EvidenceMissing { blob_id, path } => write!(
                f,
                "evidence_missing：blob {blob_id} 已登记但文件不存在（{}）",
                path.display()
            ),
            Self::EvidenceCorrupt { blob_id, detail } => {
                write!(f, "evidence_corrupt：blob {blob_id} 内容不符（{detail}）")
            }
            Self::BlobUnknown { blob_id } => {
                write!(f, "blob {blob_id} 未登记（从未写入或已被 GC 回收）")
            }
            Self::InvalidArgument { field, detail } => {
                write!(f, "参数 {field} 非法：{detail}")
            }
            Self::InvalidBlobId { value } => {
                write!(f, "blob_id 非法：{value:?}（要求 64 位小写 hex 的 sha256）")
            }
            Self::Compression { detail } => write!(f, "压缩/解压失败：{detail}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(inner) => Some(inner),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

/// 存储层结果别名。
pub type StorageResult<T> = Result<T, StorageError>;
