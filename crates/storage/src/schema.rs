//! 只前进不回滚的迁移框架（自建；不引 `sqlx` / `refinery` —— 见 ADR-0038）。
//!
//! 为什么自建够用：本层只需要"按版本号顺序执行内嵌 SQL + 记账 + 启动校验"三件事。
//! `sqlx` 会连带 async 运行时，`refinery` 会引入运行时目录扫描 —— 两者都比这 200 行更重。
//!
//! **表清单不在这里**（ADR-0038）：本模块只声明 `crates/storage` **自己**那几张表的迁移
//! （[`MIGRATIONS`]）；别的 crate 的表由它们自己声明，装配点在开库前把各集合合并。
//!
//! 不变量：
//!   1. **只前进不回滚**：任何结构调整都新增 `migrations/000N_*.sql`，不改已发布的文件
//!   2. 已应用的迁移内容被 sha256 记账；事后改动 → 启动**拒绝**（`MigrationChecksumMismatch`）
//!   3. 每个迁移在**独立事务**里执行：失败即回滚，库停在迁移前的版本（不会留下半个 schema）
//!   4. 版本号必须从 1 开始**连续**（[`MigrationSet::validate`]）：缺号 → 拒绝启动
//!   5. 库已应用的**每个**版本都必须出现在装配后的 [`MigrationSet`] 里：不认识的版本 → 拒绝启动
//!   6. `schema_migrations` 是迁移框架自身的记账表，**不在**架构 v2 §15.1 的业务表清单里
//!
//! 相关：ADR-0038、`docs/storage-design.md` §3.2 / §3.4 / §8、架构 v2 §15.2

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::{StorageError, StorageResult};
use crate::migrations::{Migration, MigrationSet};

/// `crates/storage` **自己**拥有的迁移（本 crate 建的那几张表）。
///
/// 这**不是**全库清单：应用侧必须把各 crate 的 `MIGRATIONS` 合并成一个 [`MigrationSet`]，
/// 再交给 [`crate::Database::open`]（ADR-0038 D2 / D3）。
pub const MIGRATIONS: &[Migration] = &[Migration::new(
    1,
    "0001_init",
    include_str!("../migrations/0001_init.sql"),
)];

/// 迁移记账表名。
const VERSION_TABLE: &str = "schema_migrations";

const CREATE_VERSION_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    checksum   TEXT    NOT NULL CHECK (length(checksum) = 64),
    applied_at INTEGER NOT NULL
);";

/// 已应用的迁移行。
#[derive(Debug, Clone)]
struct AppliedMigration {
    version: i64,
    name: String,
    checksum: String,
}

/// 校验库状态并把所有未应用的迁移按序执行。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：`migrations` 自身不合法（版本号重复 / 缺号 / < 1）——
///   这是**装配错误**，不是库的问题
/// - [`StorageError::SchemaVersionMismatch`]：有业务表却无版本表 / 库里有本集合不认识的版本 /
///   库版本高于本集合期望
/// - [`StorageError::MigrationChecksumMismatch`]：已应用迁移的内容被事后改动
/// - [`StorageError::MigrationFailed`]：迁移 SQL 执行失败（该迁移事务已回滚）
/// - [`StorageError::Sqlite`]：读取版本表本身失败
pub fn apply_pending(
    conn: &mut Connection,
    now_ms: i64,
    migrations: &MigrationSet,
) -> StorageResult<()> {
    migrations.validate()?;
    let expected_version = migrations.expected_version();

    if !table_exists(conn, VERSION_TABLE)? {
        let existing = business_table_count(conn)?;
        if existing > 0 {
            return Err(StorageError::SchemaVersionMismatch {
                expected: expected_version,
                found: None,
                detail: format!(
                    "库里有 {existing} 张业务表但没有 {VERSION_TABLE} 表：这是外来库或被篡改的库，拒绝启动"
                ),
            });
        }
        conn.execute_batch(CREATE_VERSION_TABLE_SQL)?;
    }

    let applied = load_applied(conn)?;
    verify_applied(&applied, migrations)?;

    let applied_max = applied.iter().map(|row| row.version).max().unwrap_or(0);
    if applied_max > expected_version {
        return Err(StorageError::SchemaVersionMismatch {
            expected: expected_version,
            found: Some(applied_max),
            detail: "库的 schema 版本高于本二进制：降级启动会静默损坏数据，拒绝启动".to_owned(),
        });
    }
    for migration in migrations
        .iter()
        .filter(|item| item.version() > applied_max)
    {
        apply_one(conn, migration, now_ms)?;
    }
    Ok(())
}

/// 库里当前已应用到的最高版本（没有记录时为 0）。
///
/// # Errors
/// 读取版本表失败时返回 [`StorageError::Sqlite`]。
pub fn applied_version(conn: &Connection) -> StorageResult<i64> {
    let version: Option<i64> =
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    Ok(version.unwrap_or(0))
}

/// 校验已应用的迁移集合：每个都必须①是装配集合里的迁移 ②内容与记账一致 ③版本连续无缺号。
fn verify_applied(applied: &[AppliedMigration], migrations: &MigrationSet) -> StorageResult<()> {
    for row in applied {
        let Some(migration) = migrations
            .iter()
            .find(|candidate| candidate.version() == row.version)
        else {
            return Err(StorageError::SchemaVersionMismatch {
                expected: migrations.expected_version(),
                found: Some(row.version),
                detail: format!(
                    "版本表里有本二进制不认识的迁移版本 {}（名字 {}）—— \
                     装配时是否漏了某个 crate 的 MIGRATIONS？",
                    row.version, row.name
                ),
            });
        };
        let actual = checksum(migration.sql());
        if actual != row.checksum {
            return Err(StorageError::MigrationChecksumMismatch {
                version: row.version,
                recorded: row.checksum.clone(),
                actual,
            });
        }
    }
    // 缺号检查：applied_max 以下的每个装配迁移都必须出现在版本表里。
    let applied_max = applied.iter().map(|row| row.version).max().unwrap_or(0);
    for migration in migrations
        .iter()
        .filter(|candidate| candidate.version() <= applied_max)
    {
        if !applied.iter().any(|row| row.version == migration.version()) {
            return Err(StorageError::SchemaVersionMismatch {
                expected: migrations.expected_version(),
                found: Some(applied_max),
                detail: format!(
                    "版本表缺号：已应用到 {applied_max}，但没有迁移 {} 的记录",
                    migration.version()
                ),
            });
        }
    }
    Ok(())
}

/// 在独立事务里执行一个迁移并记账。
fn apply_one(conn: &mut Connection, migration: &Migration, now_ms: i64) -> StorageResult<()> {
    let version = migration.version();
    let tx = conn.transaction()?;
    let fail = |error: &rusqlite::Error| StorageError::MigrationFailed {
        version,
        detail: error.to_string(),
    };
    tx.execute_batch(migration.sql()).map_err(|e| fail(&e))?;
    tx.execute(
        "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            version,
            migration.name(),
            checksum(migration.sql()),
            now_ms
        ],
    )
    .map_err(|e| fail(&e))?;
    tx.commit().map_err(|e| fail(&e))?;
    Ok(())
}

fn load_applied(conn: &Connection) -> StorageResult<Vec<AppliedMigration>> {
    let mut statement =
        conn.prepare("SELECT version, name, checksum FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| {
        Ok(AppliedMigration {
            version: row.get(0)?,
            name: row.get(1)?,
            checksum: row.get(2)?,
        })
    })?;
    let mut applied = Vec::new();
    for row in rows {
        applied.push(row?);
    }
    Ok(applied)
}

fn table_exists(conn: &Connection, name: &str) -> StorageResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        rusqlite::params![name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn business_table_count(conn: &Connection) -> StorageResult<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// 迁移 SQL 的 sha256（小写 hex，64 字符）。
fn checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    to_hex(&digest)
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
