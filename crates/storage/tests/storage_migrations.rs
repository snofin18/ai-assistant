//! 迁移注册表的负向用例（TASK-202 / ADR-0038）：**装配期**错误必须当场红灯。
//!
//! 为什么单独一个文件：这些用例测的是「装配点怎么把各 crate 的 `MIGRATIONS` 拼成一个集合」，
//! 与 `storage_integration.rs` 的「库 + 文件一致性」不是同一类契约；分开放也让
//! `storage_integration.rs` 停在 600 行以内（gov §5.4 的软上限，hygiene 会报 Warning）。
//!
//! 正向路径（storage 自己的集合能建库、记账行数 == 期望版本号）在 `storage_integration.rs`。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_storage::{Database, Migration, MigrationSet};
use common::{FixedClock, TestDir};

// ───────────────────────── 迁移集装配（ADR-0038）─────────────────────────

/// 负向用例 ⓪-a：装配集合里有**重号** → 注册当场被拒绝（不是等到开库才炸）。
///
/// 这条用例是 ADR-0038 D1 的机器判据：两个 crate 抢同一个版本号必须**立刻**红灯。
#[test]
fn test_migration_set_rejects_duplicate_version() {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("注册 storage 自己的迁移");

    let error = set
        .register(Migration::new(
            1,
            "0001_again",
            "CREATE TABLE duplicate (id TEXT);",
        ))
        .expect_err("同一个版本号注册两次必须被拒绝");

    assert_eq!(error.reason_code(), "migration_duplicate_version");
    assert_eq!(set.len(), 1, "被拒绝的那条不得进入集合");
    assert_eq!(set.expected_version(), 1);
}

/// 负向用例 ⓪-b：装配集合**缺号**（只有 0002 没有 0001）→ `open()` 拒绝启动，
/// 且原因是 `invalid_argument`（**装配错误**，不是库坏了）。
#[test]
fn test_open_rejects_non_contiguous_migration_set() {
    let dir = TestDir::new("gap-version");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));

    let mut set = MigrationSet::new();
    set.register(Migration::new(
        2,
        "0002_orphan",
        "CREATE TABLE orphan (id TEXT);",
    ))
    .expect("注册单条 0002");
    assert_eq!(
        set.validate()
            .expect_err("缺 0001 必须被拒绝")
            .reason_code(),
        "migration_non_contiguous_versions"
    );

    let error = Database::open(&dir.paths(), clock, &set).expect_err("必须拒绝启动");
    assert_eq!(error.reason_code(), "invalid_argument");
}
