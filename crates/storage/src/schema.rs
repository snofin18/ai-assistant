//! 只前进不回滚的迁移框架（自建；不引 `sqlx` / `refinery` —— 见 TASK-012 的 Out of scope）。
//!
//! 为什么自建够用：本卡只需要"按版本号顺序执行内嵌 SQL + 记账 + 启动校验"三件事。
//! `sqlx` 会连带 async 运行时，`refinery` 会引入运行时目录扫描 —— 两者都比这 150 行更重。
//!
//! 不变量：
//!   1. **只前进不回滚**：任何结构调整都新增 `migrations/000N_*.sql`，不改已发布的文件
//!   2. 已应用的迁移文件内容被 sha256 记账；事后改动 → 启动**拒绝**（`MigrationChecksumMismatch`）
//!   3. 每个迁移在**独立事务**里执行：失败即回滚，库停在迁移前的版本（不会留下半个 schema）
//!   4. 版本号必须从 1 开始**连续**：缺号 = 版本表被篡改 → 拒绝启动
//!   5. `schema_migrations` 是迁移框架自身的记账表，**不在**架构 v2 §15.1 的业务表清单里
//!
//! 相关：`docs/storage-design.md` §3.2 / §8、架构 v2 §15.2

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::{StorageError, StorageResult};

/// 二进制期望的 schema 版本 = 内嵌迁移的最高版本。
///
/// 改这个值的唯一方式 = 新增一个 `migrations/000N_*.sql` 并把它登记进 [`MIGRATIONS`]。
pub const SCHEMA_VERSION: i64 = 2;

/// 迁移记账表名。
const VERSION_TABLE: &str = "schema_migrations";

/// 一条内嵌迁移。
#[derive(Debug, Clone, Copy)]
struct Migration {
    /// 版本号（从 1 连续递增）。
    pub(crate) version: i64,
    /// 人类可读的名字（进 DB，便于人工检查；**不**参与校验）。
    pub(crate) name: &'static str,
    /// 迁移 SQL（编译期内嵌，运行时不读文件）。
    pub(crate) sql: &'static str,
}

/// 全部内嵌迁移，按版本号升序。
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_init",
        sql: include_str!("../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "0002_audit_logs",
        sql: include_str!("../migrations/0002_audit_logs.sql"),
    },
];

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
/// - [`StorageError::SchemaVersionMismatch`]：有业务表却无版本表 / 版本高于二进制 / 版本表缺号或含未知版本
/// - [`StorageError::MigrationChecksumMismatch`]：已应用迁移的文件内容被事后改动
/// - [`StorageError::MigrationFailed`]：迁移 SQL 执行失败（该迁移事务已回滚）
/// - [`StorageError::Sqlite`]：读取版本表本身失败
pub fn apply_pending(conn: &mut Connection, now_ms: i64) -> StorageResult<()> {
    if !table_exists(conn, VERSION_TABLE)? {
        let existing = business_table_count(conn)?;
        if existing > 0 {
            return Err(StorageError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                found: None,
                detail: format!(
                    "库里有 {existing} 张业务表但没有 {VERSION_TABLE} 表：这是外来库或被篡改的库，拒绝启动"
                ),
            });
        }
        conn.execute_batch(CREATE_VERSION_TABLE_SQL)?;
    }

    let applied = load_applied(conn)?;
    verify_applied(&applied)?;

    let applied_max = applied.iter().map(|row| row.version).max().unwrap_or(0);
    if applied_max > SCHEMA_VERSION {
        return Err(StorageError::SchemaVersionMismatch {
            expected: SCHEMA_VERSION,
            found: Some(applied_max),
            detail: "库的 schema 版本高于本二进制：降级启动会静默损坏数据，拒绝启动".to_owned(),
        });
    }
    for migration in MIGRATIONS.iter().filter(|m| m.version > applied_max) {
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

/// 校验已应用的迁移集合：每个都必须①是内嵌迁移 ②内容与记账一致 ③版本连续无缺号。
fn verify_applied(applied: &[AppliedMigration]) -> StorageResult<()> {
    for row in applied {
        let Some(migration) = MIGRATIONS.iter().find(|m| m.version == row.version) else {
            return Err(StorageError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                found: Some(row.version),
                detail: format!(
                    "版本表里有本二进制不认识的迁移版本 {}（名字 {}）",
                    row.version, row.name
                ),
            });
        };
        let actual = checksum(migration.sql);
        if actual != row.checksum {
            return Err(StorageError::MigrationChecksumMismatch {
                version: row.version,
                recorded: row.checksum.clone(),
                actual,
            });
        }
    }
    // 缺号检查：applied_max 以下的每个内嵌迁移都必须出现在版本表里。
    let applied_max = applied.iter().map(|row| row.version).max().unwrap_or(0);
    for migration in MIGRATIONS.iter().filter(|m| m.version <= applied_max) {
        if !applied.iter().any(|row| row.version == migration.version) {
            return Err(StorageError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                found: Some(applied_max),
                detail: format!(
                    "版本表缺号：已应用到 {applied_max}，但没有迁移 {} 的记录",
                    migration.version
                ),
            });
        }
    }
    Ok(())
}

/// 在独立事务里执行一个迁移并记账。
fn apply_one(conn: &mut Connection, migration: &Migration, now_ms: i64) -> StorageResult<()> {
    let tx = conn.transaction()?;
    let fail = |error: &rusqlite::Error| StorageError::MigrationFailed {
        version: migration.version,
        detail: error.to_string(),
    };
    tx.execute_batch(migration.sql).map_err(|e| fail(&e))?;
    tx.execute(
        "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            migration.version,
            migration.name,
            checksum(migration.sql),
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
