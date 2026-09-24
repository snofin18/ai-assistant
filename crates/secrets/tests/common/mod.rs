//! 集成测试共享夹具：固定时钟 / 记录型审计出口 / 必然失败的审计出口。
//!
//! 为什么放 `tests/common/mod.rs` 而不是每个测试文件各写一份：夹具是**契约的一部分**
//! （"时间必须注入"、"审计必须能被断言"），复制两份必然漂移。`tests/` 下的子目录不会
//! 被当成独立测试目标，所以放这里不会被重复编译成测试 crate。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(dead_code)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use assistant_secrets::{SecretAccessAuditor, SecretAccessRecord, SecretError, SecretResult};
use assistant_storage::Clock;

/// 固定时钟（生产用 `SystemClock`；测试要能确定性断言审计时间戳）。
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
}

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}

/// 记录型审计出口：把所有记录留在内存里供断言。
///
/// `Clone` 是**句柄语义**（内部 `Arc<Mutex<..>>`）：装饰器持一份，测试持另一份，看的是同一份记录。
/// 这样不必为 `Arc<T>` 加一个 blanket impl（那会扩大本 crate 的公共 API 面）。
#[derive(Default, Clone)]
pub struct RecordingAuditor {
    records: Arc<Mutex<Vec<SecretAccessRecord>>>,
}

impl RecordingAuditor {
    /// 空记录器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 已记录条数。
    pub fn count(&self) -> usize {
        self.records.lock().expect("记录器的锁").len()
    }

    /// 全部记录的副本。
    pub fn records(&self) -> Vec<SecretAccessRecord> {
        self.records.lock().expect("记录器的锁").clone()
    }
}

impl SecretAccessAuditor for RecordingAuditor {
    fn record_access(&self, record: &SecretAccessRecord) -> SecretResult<()> {
        self.records
            .lock()
            .expect("记录器的锁")
            .push(record.clone());
        Ok(())
    }
}

/// 必然失败的审计出口：用来证明 fail-closed（审计写不进去 ⇒ 该次访问被拒绝）。
pub struct FailingAuditor;

impl SecretAccessAuditor for FailingAuditor {
    fn record_access(&self, _record: &SecretAccessRecord) -> SecretResult<()> {
        Err(SecretError::BackendFailure {
            detail: "审计后端故意失败（测试夹具）".to_string(),
        })
    }
}
