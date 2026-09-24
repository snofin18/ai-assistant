//! 内容寻址 blob 池（L2）：zstd 压缩 + sha256 寻址 + 引用计数 + GC + 一致性扫描。
//!
//! 布局：`<root>/blobs/<sha256 前 2 位>/<sha256>`（256 个子目录，避免单目录百万文件）。
//! 元数据在 SQLite 的 `blobs` / `blob_refs` 两张表里（见 `migrations/0001_init.sql`）。
//!
//! 不变量：
//!   1. `blob_id` **就是**内容 sha256 的小写 hex（64 字符）；同内容 ⇒ 同 id ⇒ 只存一份
//!   2. 文件内容不可变：写进去的字节永远等于 `blob_id` 所代表的内容
//!   3. **每次读取都重算 sha256**；不符 → [`StorageError::EvidenceCorrupt`]，不返回"大概正确"的数据
//!   4. 元数据行在而文件不在 → [`StorageError::EvidenceMissing`]；**绝不**当成"没有这个 blob"（铁律 1）
//!   5. 引用计数 = `blob_refs` 的行数（一行一个引用 ⇒ 重复引用天然幂等，不会多减一次）
//!   6. GC 只删"引用数为 0 **且** 创建时间早于 TTL"的 blob
//!
//! 已知限制（见 crate README）：TTL 以 `created_at` 为基准（"最后被解引用时刻"需要额外列）。
//!
//! 值类型（`BlobId` / `BlobKind` / `BlobOwner`）在 `crate::blob_id`：它们不碰 IO，
//! 分出去才能让本文件留在 ADR-0033 的 600 行软上限内。
//!
//! 相关：`docs/storage-design.md` §3.3 / §8、架构 v2 §15.1

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, params};

use crate::blob_id::{BlobId, BlobKind, BlobOwner};
use crate::error::{StorageError, StorageResult};
use crate::paths::StoragePaths;
use crate::time_source::Clock;

/// zstd 压缩级别（`storage-design.md` §3.3：level 3 = 速度与压缩比的最佳折中）。
pub const COMPRESSION_LEVEL: i32 = 3;

/// 一致性扫描发现的一条问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityIssue {
    /// 出问题的 blob。
    pub blob_id: BlobId,
    /// 问题类别。
    pub kind: IntegrityIssueKind,
}

/// 一致性问题的类别。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntegrityIssueKind {
    /// 元数据行在，文件不在 → `evidence_missing`。
    FileMissing,
    /// 文件在但内容与 `blob_id` 不符 → `evidence_corrupt`（附说明）。
    ContentMismatch(String),
}

/// GC 的执行结果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GarbageCollection {
    /// 已删除的 blob（文件 + 元数据行）。
    pub deleted: Vec<BlobId>,
    /// 实际回收的字节数（按**压缩后**大小计）。
    pub reclaimed_bytes: i64,
    /// 元数据行在、但文件本就不在的 blob（顺带清掉的行）。
    pub rows_without_file: Vec<BlobId>,
}

/// 内容寻址 blob 池。
///
/// `Debug` 手写：`Arc<dyn Clock>` 不实现 `Debug`（trait 对象不带 Debug 约束），
/// 而"打印出池根目录"正是排查数据目录用错时最需要的信息。
pub struct BlobStore {
    root: PathBuf,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for BlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobStore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// `blobs` 表里与**读取**相关的列：`bytes` = 解压后长度，读取时用作解压容量上限。
#[derive(Debug, Clone, Copy)]
struct BlobMetadata {
    bytes: i64,
}

impl BlobStore {
    /// 以数据目录布局 + 注入时钟构造。
    #[must_use]
    pub fn new(paths: &StoragePaths, clock: Arc<dyn Clock>) -> Self {
        Self {
            root: paths.blob_root(),
            clock,
        }
    }

    /// blob 池根目录。
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 某个 blob 的绝对路径。
    #[must_use]
    pub fn path_for(&self, blob_id: &BlobId) -> PathBuf {
        self.root.join(blob_id.relative_path())
    }

    /// 写入内容（已存在则**去重**，不重复占盘、不重复记账）。
    ///
    /// # Errors
    /// - [`StorageError::EvidenceMissing`]：元数据行在但文件不在（**不**静默重写 —— 先让人看到数据丢了）
    /// - [`StorageError::EvidenceCorrupt`]：同名孤儿文件的内容与 `blob_id` 不符
    /// - [`StorageError::Compression`] / [`StorageError::Io`] / [`StorageError::Sqlite`]
    pub fn put(&self, conn: &Connection, kind: BlobKind, content: &[u8]) -> StorageResult<BlobId> {
        let blob_id = BlobId::of_content(content);
        let path = self.path_for(&blob_id);

        if Self::metadata(conn, &blob_id)?.is_some() {
            if !path.is_file() {
                return Err(StorageError::EvidenceMissing {
                    blob_id: blob_id.as_str().to_owned(),
                    path,
                });
            }
            return Ok(blob_id);
        }

        let compressed = compress(content)?;
        // `compressed_bytes` 记的是**文件真实长度**，而不是本次压缩结果的长度：收养孤儿文件时
        // 两者可能不同（不同 zstd 版本的压缩输出长度可以不一样），而一致性扫描正是拿这个数字
        // 去核对文件大小 —— 记错了会让扫描误报。
        let compressed_bytes = if path.is_file() {
            // 孤儿文件（上次写盘成功但记账失败）。内容寻址下同名即同内容：
            // 校验通过就"收养"它（不删不覆盖），不符则报 evidence_corrupt 让人处理。
            let existing = Self::read_verified(&blob_id, &path, content.len())?;
            if existing != content {
                return Err(StorageError::EvidenceCorrupt {
                    blob_id: blob_id.as_str().to_owned(),
                    detail: "孤儿文件解压后的内容与本次写入不符".to_owned(),
                });
            }
            file_length(&path)?
        } else {
            Self::write_atomically(&path, &compressed)?;
            as_i64(compressed.len())
        };

        conn.execute(
            "INSERT INTO blobs (blob_id, kind, bytes, compressed_bytes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                blob_id.as_str(),
                kind.as_str(),
                as_i64(content.len()),
                compressed_bytes,
                self.clock.now_unix_ms()
            ],
        )?;
        Ok(blob_id)
    }

    /// 读取内容并校验（解压 + 重算 sha256）。
    ///
    /// # Errors
    /// - [`StorageError::BlobUnknown`]：元数据里没有这个 blob
    /// - [`StorageError::EvidenceMissing`]：元数据行在但文件不在
    /// - [`StorageError::EvidenceCorrupt`]：解压失败 / sha256 与 `blob_id` 不符
    pub fn get(&self, conn: &Connection, blob_id: &BlobId) -> StorageResult<Vec<u8>> {
        let stored = Self::metadata(conn, blob_id)?.ok_or_else(|| StorageError::BlobUnknown {
            blob_id: blob_id.as_str().to_owned(),
        })?;
        let path = self.path_for(blob_id);
        if !path.is_file() {
            return Err(StorageError::EvidenceMissing {
                blob_id: blob_id.as_str().to_owned(),
                path,
            });
        }
        let capacity = usize::try_from(stored.bytes).unwrap_or(usize::MAX);
        Self::read_verified(blob_id, &path, capacity)
    }

    /// 增加一条引用（同一 owner 重复引用 = 幂等）。
    ///
    /// 返回值：`true` = 本次真的新增了一条引用；`false` = 该 owner 早就引用过。
    ///
    /// # Errors
    /// - [`StorageError::BlobUnknown`]：blob 未登记（引用一个不存在的 blob 是调用方 bug）
    /// - [`StorageError::EvidenceMissing`]：元数据行在但文件不在
    /// - [`StorageError::Sqlite`]
    pub fn add_reference(
        &self,
        conn: &Connection,
        blob_id: &BlobId,
        owner: &BlobOwner,
    ) -> StorageResult<bool> {
        if Self::metadata(conn, blob_id)?.is_none() {
            return Err(StorageError::BlobUnknown {
                blob_id: blob_id.as_str().to_owned(),
            });
        }
        let path = self.path_for(blob_id);
        if !path.is_file() {
            return Err(StorageError::EvidenceMissing {
                blob_id: blob_id.as_str().to_owned(),
                path,
            });
        }
        let changed = conn.execute(
            "INSERT OR IGNORE INTO blob_refs (blob_id, owner_kind, owner_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                blob_id.as_str(),
                owner.kind(),
                owner.id(),
                self.clock.now_unix_ms()
            ],
        )?;
        Ok(changed == 1)
    }

    /// 删除一条引用（幂等：不存在时返回 `false`，不算错误）。
    ///
    /// # Errors
    /// 仅 [`StorageError::Sqlite`]（DB 层失败）。
    pub fn remove_reference(
        &self,
        conn: &Connection,
        blob_id: &BlobId,
        owner: &BlobOwner,
    ) -> StorageResult<bool> {
        let changed = conn.execute(
            "DELETE FROM blob_refs WHERE blob_id = ?1 AND owner_kind = ?2 AND owner_id = ?3",
            params![blob_id.as_str(), owner.kind(), owner.id()],
        )?;
        Ok(changed == 1)
    }

    /// 当前引用数（= `blob_refs` 的行数）。
    ///
    /// # Errors
    /// 仅 [`StorageError::Sqlite`]。
    pub fn reference_count(&self, conn: &Connection, blob_id: &BlobId) -> StorageResult<i64> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM blob_refs WHERE blob_id = ?1",
            params![blob_id.as_str()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// 回收"引用数为 0 且 `created_at <= now - ttl_ms`"的 blob（文件 + 元数据行）。
    ///
    /// 调用方负责节奏（`storage-design.md` §9：后台低优先级、可暂停、用户操作时让路）。
    ///
    /// # Errors
    /// - [`StorageError::InvalidArgument`]：`ttl_ms` 为负
    /// - [`StorageError::Io`]：删文件失败（权限 / 占用）→ 该 blob 的行**不**删，保持"文件与行同进同退"
    /// - [`StorageError::Sqlite`]
    pub fn collect_garbage(
        &self,
        conn: &Connection,
        ttl_ms: i64,
    ) -> StorageResult<GarbageCollection> {
        if ttl_ms < 0 {
            return Err(StorageError::InvalidArgument {
                field: "ttl_ms",
                detail: format!("必须 >= 0，收到 {ttl_ms}"),
            });
        }
        let cutoff = self.clock.now_unix_ms().saturating_sub(ttl_ms);
        let mut statement = conn.prepare(
            "SELECT b.blob_id, b.compressed_bytes FROM blobs b
             WHERE b.created_at <= ?1
               AND NOT EXISTS (SELECT 1 FROM blob_refs r WHERE r.blob_id = b.blob_id)
             ORDER BY b.created_at",
        )?;
        let rows = statement.query_map(params![cutoff], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut candidates = Vec::new();
        for row in rows {
            let (blob_id, compressed_bytes) = row?;
            candidates.push((BlobId::parse(&blob_id)?, compressed_bytes));
        }
        drop(statement);

        let mut report = GarbageCollection::default();
        for (blob_id, compressed_bytes) in candidates {
            let path = self.path_for(&blob_id);
            match std::fs::remove_file(&path) {
                Ok(()) => report.reclaimed_bytes += compressed_bytes,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    report.rows_without_file.push(blob_id.clone());
                }
                Err(source) => return Err(StorageError::Io { path, source }),
            }
            conn.execute(
                "DELETE FROM blobs WHERE blob_id = ?1",
                params![blob_id.as_str()],
            )?;
            report.deleted.push(blob_id);
        }
        Ok(report)
    }

    /// 一致性扫描：找出"行在文件不在"（总是检查）与"内容不符"（`deep = true` 时逐条重算 sha256）。
    ///
    /// 非 deep 模式是启动时的廉价扫描；deep 模式适合维护窗口或人工排障。
    ///
    /// # Errors
    /// - [`StorageError::InvalidBlobId`]：表里有非法的 `blob_id`（说明表被外部改动过）
    /// - [`StorageError::Sqlite`] / [`StorageError::Io`]
    pub fn verify_integrity(
        &self,
        conn: &Connection,
        deep: bool,
    ) -> StorageResult<Vec<IntegrityIssue>> {
        let mut statement =
            conn.prepare("SELECT blob_id, bytes, compressed_bytes FROM blobs ORDER BY blob_id")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut listed = Vec::new();
        for row in rows {
            let (blob_id, bytes, compressed_bytes) = row?;
            listed.push((BlobId::parse(&blob_id)?, bytes, compressed_bytes));
        }
        drop(statement);

        let mut issues = Vec::new();
        for (blob_id, bytes, compressed_bytes) in listed {
            let path = self.path_for(&blob_id);
            if !path.is_file() {
                issues.push(IntegrityIssue {
                    blob_id,
                    kind: IntegrityIssueKind::FileMissing,
                });
                continue;
            }
            // 廉价检查（不 deep 也做）：文件长度必须等于记账的**压缩后**长度。
            // 截断 / 半写会在这里立刻暴露，不必解压整块数据。
            match file_length(&path) {
                Ok(length) if length == compressed_bytes => {}
                Ok(length) => {
                    issues.push(IntegrityIssue {
                        blob_id,
                        kind: IntegrityIssueKind::ContentMismatch(format!(
                            "压缩后长度不符：记账 {compressed_bytes}，实际 {length}"
                        )),
                    });
                    continue;
                }
                Err(other) => return Err(other),
            }
            if deep {
                let capacity = usize::try_from(bytes).unwrap_or(usize::MAX);
                match Self::read_verified(&blob_id, &path, capacity) {
                    Ok(_) => {}
                    Err(StorageError::EvidenceCorrupt { detail, .. }) => {
                        issues.push(IntegrityIssue {
                            blob_id,
                            kind: IntegrityIssueKind::ContentMismatch(detail),
                        });
                    }
                    Err(StorageError::EvidenceMissing { .. }) => {
                        issues.push(IntegrityIssue {
                            blob_id,
                            kind: IntegrityIssueKind::FileMissing,
                        });
                    }
                    Err(other) => return Err(other),
                }
            }
        }
        Ok(issues)
    }

    /// 读文件 → 解压（容量上限 = 记账的原始长度）→ 重算 sha256 并与 `blob_id` 比对。
    fn read_verified(blob_id: &BlobId, path: &Path, capacity: usize) -> StorageResult<Vec<u8>> {
        let raw = std::fs::read(path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                StorageError::EvidenceMissing {
                    blob_id: blob_id.as_str().to_owned(),
                    path: path.to_path_buf(),
                }
            } else {
                StorageError::Io {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;
        let content = zstd::bulk::decompress(&raw, capacity).map_err(|error| {
            StorageError::EvidenceCorrupt {
                blob_id: blob_id.as_str().to_owned(),
                detail: format!("解压失败（容量上限 {capacity} 字节）：{error}"),
            }
        })?;
        let actual = BlobId::of_content(&content);
        if actual != *blob_id {
            return Err(StorageError::EvidenceCorrupt {
                blob_id: blob_id.as_str().to_owned(),
                detail: format!("sha256 不符：实测 {}", actual.as_str()),
            });
        }
        Ok(content)
    }

    /// 先写 `<path>.tmp` 再改名（原子替换），避免崩溃留下"半个 blob"被当成有效内容。
    ///
    /// 改名失败时错误里的路径是**临时文件**的路径（提示人工清理），主错误不被吞掉。
    fn write_atomically(path: &Path, bytes: &[u8]) -> StorageResult<()> {
        let parent = path.parent().ok_or_else(|| StorageError::InvalidArgument {
            field: "blob_path",
            detail: format!("没有父目录：{}", path.display()),
        })?;
        std::fs::create_dir_all(parent).map_err(|source| StorageError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, bytes).map_err(|source| StorageError::Io {
            path: temporary.clone(),
            source,
        })?;
        std::fs::rename(&temporary, path).map_err(|source| StorageError::Io {
            path: temporary,
            source,
        })?;
        Ok(())
    }

    fn metadata(conn: &Connection, blob_id: &BlobId) -> StorageResult<Option<BlobMetadata>> {
        let mut statement = conn.prepare("SELECT bytes FROM blobs WHERE blob_id = ?1")?;
        let mut rows = statement.query(params![blob_id.as_str()])?;
        match rows.next()? {
            Some(row) => Ok(Some(BlobMetadata { bytes: row.get(0)? })),
            None => Ok(None),
        }
    }
}

/// `usize` → `i64`：超过 `i64::MAX` 只可能是数据异常，饱和到上限（不 panic、不静默取模）。
fn as_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// 文件当前长度（字节），按 `i64` 记账。
///
/// # Errors
/// 读不到元数据（文件被删 / 权限不足）→ [`StorageError::Io`]。
fn file_length(path: &Path) -> StorageResult<i64> {
    let length = std::fs::metadata(path)
        .map_err(|source| StorageError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    Ok(as_i64(usize::try_from(length).unwrap_or(usize::MAX)))
}

/// zstd 压缩（level 3）。
fn compress(content: &[u8]) -> StorageResult<Vec<u8>> {
    zstd::bulk::compress(content, COMPRESSION_LEVEL).map_err(|error| StorageError::Compression {
        detail: error.to_string(),
    })
}
