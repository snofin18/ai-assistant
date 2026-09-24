//! 迁移注册表：**机制在这里，表清单不在这里**（ADR-0038）。
//!
//! 职责：给「一组迁移」提供确定性、可校验的容器 —— 拒绝重复版本号、要求版本号从 1 **连续**，
//! 并给出「本次装配期望的 schema 版本」。
//!
//! 边界（不做什么）：
//!   - **不拥有任何迁移**：`crates/storage` 自己的表由 [`crate::MIGRATIONS`] 声明，
//!     别的 crate 的表由**它们自己**声明（`crates/<owner>/migrations/`，登记表见
//!     `docs/storage-design.md` §3.4）
//!   - 不执行 SQL（执行在 [`crate::schema`]）、不碰连接、不做任何 IO
//!
//! 不变量：
//!   1. 同一个 `version` 在一个 [`MigrationSet`] 里**最多出现一次**（[`MigrationSet::register`] 硬拦）
//!   2. 合法集合的版本号 = `1..=expected_version()` **无缺号**（[`MigrationSet::validate`]）
//!   3. 集合内条目**始终按版本号升序**（`register` 按序插入，与调用顺序无关）
//!
//! 相关：ADR-0038、`docs/storage-design.md` §3.4（版本号登记表）

use std::fmt;

/// 一条迁移：版本号 + 名字 + **编译期内嵌**的 SQL。
///
/// `sql` 一律由**拥有者 crate** 用 `include_str!` 内嵌 —— 不在运行时读文件，那会让回放
/// 依赖构建产物之外的目录状态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

impl Migration {
    /// 构造一条迁移。
    ///
    /// `version` 必须 ≥ 1（装配时由 [`MigrationSet::register`] 校验）。
    /// `name` 会写进 `schema_migrations` 供人工核对，**不**参与校验 —— 参与校验的是 SQL 的 sha256。
    #[must_use]
    pub const fn new(version: i64, name: &'static str, sql: &'static str) -> Self {
        Self { version, name, sql }
    }

    /// 版本号（从 1 连续递增；全局唯一）。
    #[must_use]
    pub const fn version(self) -> i64 {
        self.version
    }

    /// 人类可读的名字（如 `0001_init`）。
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// 迁移 SQL。`pub(crate)`：只有迁移框架需要它。
    pub(crate) const fn sql(self) -> &'static str {
        self.sql
    }
}

impl fmt::Debug for Migration {
    /// 手写 `Debug`：默认实现会把整段 SQL 打出来，测试失败信息会被淹掉。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Migration")
            .field("version", &self.version)
            .field("name", &self.name)
            .field("sql_bytes", &self.sql.len())
            .finish()
    }
}

/// 装配 [`MigrationSet`] 时的结构性错误。
///
/// 为什么单独一个类型而不并进 [`crate::StorageError`]：这些是**装配期**错误（编程错误），
/// 不是运行期存储故障；装配点应当**直接失败**，而不是把它当成可退避重试的 IO/DB 问题。
/// 与 [`crate::StorageError`] 同风格：每个变体都有稳定的 [`MigrationSetError::reason_code`]。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MigrationSetError {
    /// 版本号 < 1。
    InvalidVersion {
        /// 被拒绝的版本号。
        version: i64,
        /// 迁移名（便于定位是哪一行声明写错了）。
        name: &'static str,
    },
    /// 同一个版本号被注册了两次（两个 crate 抢了同一个号）。
    DuplicateVersion {
        /// 冲突的版本号。
        version: i64,
        /// 先注册的那条迁移名。
        first: &'static str,
        /// 后注册的那条迁移名。
        second: &'static str,
    },
    /// 版本号不连续（`1..=highest` 里缺号）。
    NonContiguousVersions {
        /// 缺的那个版本号。
        missing: i64,
        /// 集合里的最高版本号。
        highest: i64,
    },
}

impl MigrationSetError {
    /// 稳定的机器可读原因码（进日志 / 审计；**不得随版本改名**）。
    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::InvalidVersion { .. } => "migration_invalid_version",
            Self::DuplicateVersion { .. } => "migration_duplicate_version",
            Self::NonContiguousVersions { .. } => "migration_non_contiguous_versions",
        }
    }
}

impl fmt::Display for MigrationSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion { version, name } => {
                write!(f, "迁移 {name} 的版本号 {version} 非法：必须 ≥ 1")
            }
            Self::DuplicateVersion {
                version,
                first,
                second,
            } => write!(
                f,
                "版本号 {version} 被注册两次（先 {first}，后 {second}）：每个版本号全局唯一"
            ),
            Self::NonContiguousVersions { missing, highest } => write!(
                f,
                "迁移版本号不连续：已到 {highest} 但缺 {missing}（版本号必须从 1 连续递增）"
            ),
        }
    }
}

impl std::error::Error for MigrationSetError {}

/// 一组**已按版本号升序**的迁移。
///
/// 典型用法（**唯一装配点**，ADR-0038 D3）：
///
/// ```
/// use assistant_storage::{Migration, MigrationSet};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut set = MigrationSet::new();
/// set.register(Migration::new(1, "0001_init", "CREATE TABLE a (id TEXT);"))?;
/// set.register(Migration::new(2, "0002_b", "CREATE TABLE b (id TEXT);"))?;
/// set.validate()?;
/// assert_eq!(set.expected_version(), 2);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationSet {
    /// 按 `version` 升序、且版本号互不相同（由 `register` 维持）。
    migrations: Vec<Migration>,
}

impl MigrationSet {
    /// 空集合。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// 注册一条迁移（按版本号插入到正确位置）。
    ///
    /// # Errors
    /// - [`MigrationSetError::InvalidVersion`]：`version < 1`
    /// - [`MigrationSetError::DuplicateVersion`]：该版本号已被注册（两个 crate 抢号）
    pub fn register(&mut self, migration: Migration) -> Result<(), MigrationSetError> {
        if migration.version < 1 {
            return Err(MigrationSetError::InvalidVersion {
                version: migration.version,
                name: migration.name,
            });
        }
        if let Some(existing) = self
            .migrations
            .iter()
            .find(|registered| registered.version == migration.version)
        {
            return Err(MigrationSetError::DuplicateVersion {
                version: migration.version,
                first: existing.name,
                second: migration.name,
            });
        }
        let position = self
            .migrations
            .partition_point(|registered| registered.version < migration.version);
        self.migrations.insert(position, migration);
        Ok(())
    }

    /// 批量注册（语义同 [`MigrationSet::register`]；中途失败时**已注册的保留**，由调用方决定是否继续）。
    ///
    /// # Errors
    /// 同 [`MigrationSet::register`]。
    pub fn register_all(&mut self, migrations: &[Migration]) -> Result<(), MigrationSetError> {
        for migration in migrations {
            self.register(*migration)?;
        }
        Ok(())
    }

    /// 校验版本号从 1 **连续**到 [`MigrationSet::expected_version`]。
    ///
    /// 为什么连续是硬要求：版本号是「迁移链的位置」，缺号意味着**某一版的 DDL 丢了**，
    /// 而库却可能已经按它建过表 —— 这正是"静默损坏"的形态（铁律 1）。
    ///
    /// # Errors
    /// [`MigrationSetError::NonContiguousVersions`]：`1..=highest` 里有缺号。
    pub fn validate(&self) -> Result<(), MigrationSetError> {
        let highest = self.expected_version();
        // 用 `zip` 而不是手写计数器：位置 i（0-based）处的版本号必须正好是 i + 1。
        for (expected, migration) in (1_i64..).zip(self.migrations.iter()) {
            if migration.version != expected {
                return Err(MigrationSetError::NonContiguousVersions {
                    missing: expected,
                    highest,
                });
            }
        }
        Ok(())
    }

    /// 本集合期望的 schema 版本 = 最高版本号（空集合 = 0）。
    #[must_use]
    pub fn expected_version(&self) -> i64 {
        self.migrations.last().map_or(0, |last| last.version)
    }

    /// 条目数。
    #[must_use]
    pub const fn len(&self) -> usize {
        self.migrations.len()
    }

    /// 是否为空。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.migrations.is_empty()
    }

    /// 按版本号升序迭代。
    pub fn iter(&self) -> std::slice::Iter<'_, Migration> {
        self.migrations.iter()
    }
}

impl<'set> IntoIterator for &'set MigrationSet {
    type Item = &'set Migration;
    type IntoIter = std::slice::Iter<'set, Migration>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
