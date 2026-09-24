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

use assistant_audit::{AuditLog, Durability};
use assistant_protocol::AuditEvent;
use assistant_storage::{Clock, Database, StoragePaths};
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

/// 在临时目录里打开（必要时创建）主库。
pub fn open_database(dir: &TestDir, clock: &Arc<FixedClock>) -> Database {
    let clock = Arc::clone(clock);
    Database::open(&dir.paths(), clock).expect("打开数据库")
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
