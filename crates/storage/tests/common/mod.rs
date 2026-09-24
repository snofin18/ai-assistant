//! 集成测试共享夹具：临时目录 / 固定时钟 / 计数小工具。
//!
//! 为什么用 `tests/common/mod.rs` 而不是每个测试文件各写一份：夹具本身是**契约的一部分**
//! （"测试只用临时目录"、"时间必须注入"），复制三份必然漂移。`tests/` 下的子目录不会被
//! 当成独立测试目标，所以放这里不会被重复编译成测试 crate。
//!
//! `dead_code` 必须 allow：每个测试 crate 只用到夹具的一部分，未用到的那部分在别的 crate 里
//! 是活的 —— 这不是"没人用的代码"，而是"跨测试 crate 共享的代码"。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(dead_code)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use assistant_storage::{Clock, Database, MigrationSet, StoragePaths};
use rusqlite::Connection;

/// 每个用例独占一个临时目录；`Drop` 时尽力清理（清理失败不能污染断言结果）。
pub struct TestDir {
    root: PathBuf,
}

impl TestDir {
    /// 新建一个进程内唯一的临时目录（已创建）。`Drop` 时尽力清理。
    pub fn new(label: &str) -> Self {
        // 进程 id + 进程内单调计数器 = 并发跑测试也不会撞目录（不依赖系统时间，避免抖动）
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "assistant-storage-test-{}-{label}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("创建临时目录");
        Self { root }
    }

    /// 以本临时目录为数据根目录的布局描述。
    pub fn paths(&self) -> StoragePaths {
        StoragePaths::new(&self.root)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // 尽力清理；失败不 panic（Drop 里 panic 会污染其它用例的结果），也不影响断言
        std::fs::remove_dir_all(&self.root).ok();
    }
}

/// 固定 / 可推进的测试时钟（生产用 `SystemClock`，测试要能确定性验证 TTL）。
pub struct FixedClock {
    now_ms: AtomicI64,
}

impl FixedClock {
    /// 建一个停在 `now_ms` 的时钟。
    pub const fn new(now_ms: i64) -> Self {
        Self {
            now_ms: AtomicI64::new(now_ms),
        }
    }

    /// 把"现在"推进到 `now_ms`（只前进，不回拨）。
    pub fn advance_to(&self, now_ms: i64) {
        self.now_ms.store(now_ms, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}

/// `crates/storage` **自己**那几张表的迁移集。
///
/// 为什么本 crate 的测试只装配自己的迁移：ADR-0038 之后「别的 crate 加表」与 storage 的测试**无关**
/// —— 这正是注册表要解决的问题（TASK-013 时它曾打爆这里 3 处断言）。
pub fn migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("装配 storage 自己的迁移");
    set.validate().expect("storage 的迁移必须从 1 连续");
    set
}

/// 在临时目录里用**本 crate 自己**的迁移集打开（必要时创建）主库。
pub fn open_database(dir: &TestDir, clock: &Arc<FixedClock>) -> Database {
    open_database_with(dir, clock, &migrations())
}

/// 用**指定**迁移集打开主库（负向用例需要「非法集合」或「只含某一部分」的集合）。
pub fn open_database_with(
    dir: &TestDir,
    clock: &Arc<FixedClock>,
    migrations: &MigrationSet,
) -> Database {
    // 先绑定再传参：`Arc<FixedClock>` → `Arc<dyn Clock>` 的 unsized coercion 只在
    // 实参位置发生，直接内联进 `Arc::clone` 会把泛型参数推成 `dyn Clock` 而失败。
    let clock = Arc::clone(clock);
    Database::open(&dir.paths(), clock, migrations).expect("打开数据库")
}

/// 递归统计某目录下的**文件**数（用于证明"同内容只存一份"）。
pub fn count_files(root: &Path) -> usize {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("读取目录") {
            let entry = entry.expect("目录项");
            if entry.file_type().expect("文件类型").is_dir() {
                stack.push(entry.path());
            } else {
                total += 1;
            }
        }
    }
    total
}

/// 表行数（测试内的小工具；表名来自本文件里的字面量，不含外部输入）。
pub fn count_rows(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("计数")
}

/// 文件当前长度（字节）。
pub fn file_size(path: &Path) -> i64 {
    i64::try_from(std::fs::metadata(path).expect("文件元数据").len()).expect("长度在 i64 内")
}
