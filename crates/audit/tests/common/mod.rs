//! 集成测试共享夹具：临时目录 / 可推进时钟 / 示例事件 / 表级探针。
//!
//! 为什么用 `tests/common/mod.rs`：夹具本身是**契约的一部分**（"测试只用临时目录"、
//! "时间必须注入"、"事件一律从 JSON 构造"），复制多份必然漂移。`tests/` 下的子目录
//! 不会被当成独立测试目标，所以放这里不会被重复编译成测试 crate。
//!
//! `dead_code` 必须 allow：每个测试 crate 只用到夹具的一部分，未用到的那部分在别的 crate
//! 里是活的 —— 这不是"没人用的代码"，而是"跨测试 crate 共享的代码"。
//!
//! 测试内允许 `expect` / `unwrap` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(dead_code)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use assistant_audit::{AuditLog, Durability, canonical_payload, compute_self_hash};
use assistant_protocol::AuditEvent;
use assistant_storage::{Clock, Database, MigrationSet, StoragePaths};
use rusqlite::Connection;

/// 每个用例独占一个临时目录；`Drop` 时尽力清理（清理失败不能污染断言结果）。
pub struct TestDir {
    root: PathBuf,
}

impl TestDir {
    /// 新建一个进程内唯一的临时目录（已创建）。
    pub fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "assistant-audit-test-{}-{label}-{unique}",
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
        std::fs::remove_dir_all(&self.root).ok();
    }
}

/// 固定 / 可推进的测试时钟（生产用 `SystemClock`；批量模式的"200 ms 到点"必须能确定性验证）。
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

    /// 相对当前时间前进 `delta_ms`。
    pub fn advance_by(&self, delta_ms: i64) {
        let current = self.now_ms.load(Ordering::SeqCst);
        self.now_ms.store(current + delta_ms, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}

/// **装配点**（ADR-0038 D3）：`crates/storage` 自己的表 + 本 crate 的 `audit_logs`。
///
/// 为什么这是"唯一装配点"：`crates/audit` 是当下**唯一**的跨 crate 消费者；
/// 正式的装配点归 Host（TASK-019~028）。别的 crate 加表时，改的是它**自己**的
/// `MIGRATIONS` + 这一处装配，**不再**改 `crates/storage` 的源码与测试。
pub fn migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("装配 storage 自己的迁移");
    set.register_all(assistant_audit::MIGRATIONS)
        .expect("装配 audit 自己的迁移");
    set.validate().expect("迁移集必须从 1 连续");
    set
}

/// **真实**的 v2 库形状：`crates/storage` 的 0001 + `crates/audit` 的 **0002**（不含 0003）。
///
/// 为什么需要它：0003 是**重建表**的迁移（ADR-0040 D4），必须验证「已有数据 + 旧列形状」的库
/// 能原地升级。用完整集合造不出 v2 库；而手工 `DROP TABLE` 造出的「旧版本库」是**假状态**
/// （ADR-0038 之后已被本 crate 的 pitfalls 否掉）。这里从**拥有者自己声明**的 `MIGRATIONS`
/// 里取 0002，故升级用例的起点是**真库**。
pub fn migrations_at_v2() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("装配 storage 自己的迁移");
    let audit_0002 = assistant_audit::MIGRATIONS
        .iter()
        .find(|item| item.version() == 2)
        .copied()
        .expect("audit 必须声明 0002");
    set.register(audit_0002).expect("注册 audit 0002");
    set.validate().expect("v2 集合必须从 1 连续");
    set
}

/// 只含 `crates/storage` 自己迁移的集合（= 历史 v1 库的形状，用于升级路径与"漏注册"负向用例）。
pub fn storage_only_migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("装配 storage 自己的迁移");
    set.validate().expect("storage 的迁移必须从 1 连续");
    set
}

/// 在临时目录里用**装配后**的迁移集打开（必要时创建）主库。
pub fn open_database(dir: &TestDir, clock: &Arc<FixedClock>) -> Database {
    open_database_with(dir, clock, &migrations())
}

/// 用**指定**迁移集打开主库（负向用例需要「只含 storage」的集合）。
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

/// 批量模式、窗口极长的写入器：只会在满 `max_events` 时 flush，便于逐步断言。
pub fn batched_log<'a>(
    connection: &'a Connection,
    clock: &Arc<FixedClock>,
    max_events: usize,
) -> AuditLog<'a> {
    AuditLog::new(
        connection,
        Arc::clone(clock) as Arc<dyn Clock>,
        Durability::Batched {
            max_events,
            max_interval_ms: 10_000,
        },
    )
    .expect("打开审计写入器")
}

/// 每条立即落库的写入器。
pub fn immediate_log<'a>(connection: &'a Connection, clock: &Arc<FixedClock>) -> AuditLog<'a> {
    AuditLog::new(
        connection,
        Arc::clone(clock) as Arc<dyn Clock>,
        Durability::Immediate,
    )
    .expect("打开审计写入器")
}

/// 构造一个示例审计事件。
///
/// 为什么走 `serde_json::from_value` 而不是结构体字面量：`AuditEvent` 是 `#[non_exhaustive]`
/// （协议 crate 的向后兼容约定），**跨 crate 不允许**用字面量构造。走 JSON 同时也顺带证明
/// "协议 JSON 形状 ↔ Rust 类型"是一致的。
pub fn sample_event(session_id: &str, action: &str, ts: &str) -> AuditEvent {
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "event_type": "tool.called",
        "ts": ts,
        "session_id": session_id,
        "actor": "agent",
        "action": action,
        "self_hash": ""
    }))
    .expect("构造 AuditEvent")
}

/// 同一事件的另一个变体（用于证明"改一个字段 ⇒ hash 变"）。
pub fn sample_event_with_action(action: &str) -> AuditEvent {
    sample_event("s_1", action, "2026-09-24T00:00:00Z")
}

/// 往**旧形状**（v2：有 `hash` 列、无 `sequence`）的 `audit_logs` 里按链序写入 `count` 行。
///
/// 为什么要这样写而不是直接用 `AuditLog`：`AuditLog` 的 SQL 已经跟着迁移 0003 走（`ORDER BY sequence`），
/// 在 v2 表上会直接报 `no such column: sequence` —— 这正是「代码只认最新 schema」的正确形态。
/// 于是升级用例必须用**当时那份 DDL 允许的形状**造数据；hash 仍走 crate 的公开链算法
/// （`canonical_payload` + `compute_self_hash`），故造出来的库对 `verify_chain` 是**真**自洽的。
///
/// 返回 `(各行的 id, 链尾)`。
pub fn seed_legacy_v2_rows(connection: &Connection, count: usize) -> (Vec<String>, String) {
    let mut ids = Vec::with_capacity(count);
    let mut tail = assistant_audit::GENESIS_PREV_HASH.to_owned();
    for index in 0..count {
        let mut event = sample_event("s_1", &format!("legacy_{index}"), "2026-09-24T00:00:00Z");
        event.prev_hash = Some(tail.clone());
        event.self_hash = String::new();
        let canonical = canonical_payload(&event).expect("规范化事件");
        let self_hash = compute_self_hash(&tail, &canonical);
        event.self_hash.clone_from(&self_hash);
        let detail_json = serde_json::to_string(&event).expect("序列化事件");

        connection
            .execute(
                "INSERT INTO audit_logs \
                 (id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json, hash) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    self_hash,
                    tail,
                    // 全部同毫秒：顺带证明「读出的顺序来自链序而不是 ts」（与 verify.rs 的走链判据同源）
                    1_700_000_000_000_i64,
                    "agent",
                    Option::<String>::None,
                    Option::<String>::None,
                    "tool.called",
                    detail_json,
                    self_hash,
                ],
            )
            .expect("写入 v2 形状的审计行");

        ids.push(self_hash.clone());
        tail = self_hash;
    }
    (ids, tail)
}

/// `sqlite_master` 里是否存在某个对象（表 / 索引 / 触发器）。
pub fn object_exists(connection: &Connection, kind: &str, name: &str) -> bool {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
            rusqlite::params![kind, name],
            |row| row.get(0),
        )
        .expect("查 sqlite_master");
    count > 0
}

/// `audit_logs` 当前行数。
pub fn audit_row_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM audit_logs", [], |row| row.get(0))
        .expect("统计 audit_logs")
}

/// 读出全部审计行。
pub fn all_records(log: &AuditLog<'_>) -> Vec<assistant_audit::AuditRecord> {
    log.records().expect("读出审计行")
}

/// 摘掉某条 append-only 护栏（模拟"绕过 crate 直接改库"的攻击者）。
pub fn drop_trigger(connection: &Connection, name: &str) {
    connection
        .execute_batch(&format!("DROP TRIGGER {name};"))
        .expect("摘掉护栏");
}
