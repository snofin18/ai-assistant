//! # assistant-storage crate（TASK-012；迁移机制见 ADR-0038）
//!
//! 阶段 1A1 基础设施：**持久化落点**。四层存储里的 L1（SQLite/WAL）与 L2（内容寻址 blob）。
//!
//! ## 职责
//!
//! - L1：主库连接 + PRAGMA 基线 + **只前进不回滚**的迁移框架 + **本 crate 自己**那几张表
//!   （W1 任务/步骤/检查点、W6 用量）
//! - L2：内容寻址 blob 池（zstd level 3 + sha256 寻址 + 去重 + 引用计数 + GC + 一致性扫描）
//! - **迁移注册表**（[`MigrationSet`]）：给「一组迁移」提供唯一性 / 连续性的硬校验
//!
//! ## 边界（不做什么）
//!
//! - 不含业务规则：状态机、重试、预算、撤销锚点管理、审计 hash chain 都**不**在这里
//! - **不拥有全库表清单**（ADR-0038）：本 crate 只声明 [`MIGRATIONS`]（自己的 0001）；
//!   `audit_logs` 由 `crates/audit` 自己的迁移 `0002` 建，读写与 hash chain 也归它。
//!   不建 FTS5 记忆表（TASK-028）、不实现影子副本的写入策略
//! - 不调用任何平台 API（`arch` 护栏会拦）；不打开只读连接池（归 TASK-028 之后的 Core 装配）
//! - 不做加密 / SQLCipher、不做冷归档、不做在线备份 CLI
//!
//! ## 不变量
//!
//! 1. **无静默失败**（铁律 1）：schema 版本不符 / 迁移集非法 / blob 校验失败 / 行与文件不一致
//!    —— 一律报错，绝不"用默认值继续跑"
//! 2. **只有 Core 持写连接**（`docs/storage-design.md` §7）；本 crate 不提供多写者并发入口
//! 3. **只前进不回滚**：迁移失败即回滚该迁移的事务，库停在旧版本并拒绝继续
//! 4. **迁移清单按拥有者拆分**（ADR-0038）：`Database::open` 的迁移集是**必填参数**，
//!    不存在"默认只开 storage 自己那部分"的静默路径；版本号登记表见 `docs/storage-design.md` §3.4
//! 5. 时钟 / FS 根目录**注入**（`Clock` / `StoragePaths`）：测试用固定时钟 + 临时目录即可回放
//! 6. 运行时产物（`*.db` / `-wal` / `-shm` / `blobs/` / `shadow/`）**绝不**落在仓库内（`.gitignore` 已覆盖）
//!
//! ## 典型用法
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use assistant_storage::{
//!     BlobKind, BlobOwner, Database, MIGRATIONS, MigrationSet, StoragePaths, SystemClock,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let paths = StoragePaths::new("D:/data/assistant");
//! // 唯一装配点（ADR-0038 D3）：把各 crate 的 MIGRATIONS 合并成一个集合
//! let mut migrations = MigrationSet::new();
//! migrations.register_all(MIGRATIONS)?;
//! let database = Database::open(&paths, Arc::new(SystemClock), &migrations)?;
//! let blobs = database.blob_store();
//!
//! let id = blobs.put(database.connection(), BlobKind::TreeSnapshot, b"{}")?;
//! let owner = BlobOwner::new("step", "s_1")?;
//! blobs.add_reference(database.connection(), &id, &owner)?;
//! assert_eq!(blobs.get(database.connection(), &id)?, b"{}");
//! # Ok(())
//! # }
//! ```
//!
//! ## 相关 spec / 文档
//!
//! `docs/storage-design.md`（§3.2 PRAGMA、§3.3 blob、§3.4 迁移登记表、§4 表映射、§7 并发、§8 一致性）、
//! 架构 v2 §15、**ADR-0038**（迁移注册表）、`docs/spec/error-codes.md`（错误分类）、
//! `tasks/TASK-012-*.md` / `tasks/TASK-202-*.md`（本 crate 的两张卡）。

#![deny(unsafe_code)]

mod blob_id;
mod content;
mod error;
mod migrations;
mod paths;
mod records;
pub(crate) mod schema;
mod time_source;

pub use blob_id::{BlobId, BlobKind, BlobOwner};
pub use content::{
    BlobStore, COMPRESSION_LEVEL, GarbageCollection, IntegrityIssue, IntegrityIssueKind,
};
pub use error::{StorageError, StorageResult};
pub use migrations::{Migration, MigrationSet, MigrationSetError};
pub use paths::StoragePaths;
pub use records::{
    CheckpointRecord, TaskRecord, TaskStepRecord, UsageRecord, count_usage_records,
    insert_checkpoint, insert_task, insert_task_step, insert_usage_record, load_latest_checkpoint,
    load_task, load_task_steps,
};
pub use schema::MIGRATIONS;
pub use time_source::{Clock, SystemClock};

use std::fmt;
use std::sync::Arc;

use rusqlite::Connection;

/// 主库句柄（一个写连接 + 数据目录布局 + 注入的时钟）。
///
/// 为什么把三样东西绑在一起：`docs/storage-design.md` §7 规定"只有 Core 持有写连接"，
/// 把连接与它操作的目录、它使用的时间源放在同一个值里，可以让"用错目录 / 用错时钟"在
/// 类型层面变得不可能。
pub struct Database {
    connection: Connection,
    paths: StoragePaths,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

impl Database {
    /// 打开（必要时创建）主库：建目录 → 开连接 → 设 PRAGMA → 校验并应用迁移。
    ///
    /// `migrations` 是**必填**的（ADR-0038 D1/D3）：它是装配点合并好的全库迁移集，
    /// 本 crate 无法自己知道"别的 crate 有哪些表"。传空集会得到一个只有记账表的库
    /// —— 这是调用方的装配错误，会在第一次用表时报错（**不**静默）。
    ///
    /// 幂等：对已初始化的库重复调用只会做校验，不会重复建表。
    ///
    /// # Errors
    /// - [`StorageError::Io`]：数据目录创建失败
    /// - [`StorageError::InvalidArgument`]：`migrations` 自身不合法（版本号重复 / 缺号）
    /// - [`StorageError::SchemaVersionMismatch`]：库版本与本集合不符（外来库 / 降级 / 被篡改 /
    ///   库里存在本集合不认识的版本）
    /// - [`StorageError::MigrationChecksumMismatch`]：已应用的迁移文件被事后改动
    /// - [`StorageError::MigrationFailed`]：迁移 SQL 失败（该迁移已回滚）
    /// - [`StorageError::Sqlite`]：连接或 PRAGMA 失败
    pub fn open(
        paths: &StoragePaths,
        clock: Arc<dyn Clock>,
        migrations: &MigrationSet,
    ) -> StorageResult<Self> {
        paths.ensure_layout()?;
        let mut connection = Connection::open(paths.database_file())?;
        apply_pragmas(&connection)?;
        schema::apply_pending(&mut connection, clock.now_unix_ms(), migrations)?;
        Ok(Self {
            connection,
            paths: paths.clone(),
            clock,
        })
    }

    /// 主库连接（写者）。只读消费者应另开 `SQLITE_OPEN_READONLY` 连接（本卡不提供）。
    #[must_use]
    pub const fn connection(&self) -> &Connection {
        &self.connection
    }

    /// 数据目录布局。
    #[must_use]
    pub const fn paths(&self) -> &StoragePaths {
        &self.paths
    }

    /// 注入的时钟（供上层构造同一时间源的其它组件）。
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        Arc::clone(&self.clock)
    }

    /// blob 池句柄（与主库共享同一目录布局与时钟）。
    #[must_use]
    pub fn blob_store(&self) -> BlobStore {
        BlobStore::new(&self.paths, Arc::clone(&self.clock))
    }

    /// 库当前已应用到的 schema 版本（新库刚建好时 = 装配集合的
    /// [`MigrationSet::expected_version`]）。
    ///
    /// # Errors
    /// 仅 [`StorageError::Sqlite`]。
    pub fn schema_version(&self) -> StorageResult<i64> {
        schema::applied_version(&self.connection)
    }

    /// 显式关闭连接（消费 `self`）。
    ///
    /// # Errors
    /// 关闭失败时返回 [`StorageError::Sqlite`]（连接会被丢弃，错误不吞）。
    pub fn close(self) -> StorageResult<()> {
        self.connection
            .close()
            .map_err(|(_connection, error)| StorageError::Sqlite(error))
    }
}

/// 按 `docs/storage-design.md` §3.2 设置 PRAGMA。
///
/// `page_size` 必须最先设（只对**新建**的库生效）；`journal_mode = WAL` 之后读写才不互斥。
fn apply_pragmas(connection: &Connection) -> StorageResult<()> {
    connection.pragma_update(None, "page_size", 8192_i64)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "busy_timeout", 5000_i64)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "mmap_size", 268_435_456_i64)?;
    connection.pragma_update(None, "cache_size", -65_536_i64)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    connection.pragma_update(None, "wal_autocheckpoint", 1000_i64)?;
    Ok(())
}
