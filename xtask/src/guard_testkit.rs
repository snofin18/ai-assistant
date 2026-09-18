//! # guard 的测试替身：内存锁存储（仅在 `cfg(test)` 下编译）
//!
//! 职责：用 `BTreeMap` 模拟锁目录，用 `Cell<u64>` 模拟时钟，让 `acquire` 的
//! 「等待 → 超时 → 放弃」「陈旧接管」「时钟回拨」「多文件回滚」这些**依赖真实时间与文件系统**
//! 的分支可以在毫秒内被完整覆盖。
//!
//! ## 为什么必须有它
//! 没有替身就只有两种测法：真的睡 30 秒（CI 不可接受），或者不测 ——
//! 而 ADR-0019 的元门禁要求「每道防线都得证明它在该红的时候真的会红」。
//! 时钟回拨（`ClockAnomaly`）在真机上几乎无法构造，替身是唯一可行的测法。
//!
//! ## 边界（不做什么）
//! - 不模拟并发：`create` 的原子性由 `FileLockStore` 的 `create_new` 保证（ADR-0028 D3），
//!   替身只需模拟「已存在则失败」这一**结果**。真实并发实证见 ADR-0028 验证方式 2。
//! - 不模拟 IO 错误（除了显式的 `fail_next_create`）：错误路径由 `GuardFailure::Io` 的类型系统
//!   保证被处理，不需要每个分支都造一次故障。
//!
//! ## 不变量
//! 1. 键一律用 `guard::normalize_target` 之后的路径，与 `FileLockStore` 的 slug 派生等价
//!    （大小写不敏感，见 `guard::slug_for` 的说明）。
//! 2. `sleep` **推进时钟**：否则超时分支永远走不到。推进量由 `advance_per_sleep_secs` 控制。
//! 3. 替身不 panic、不 unwrap（`clippy::unwrap_used` 是 workspace deny）。

#![cfg(test)]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use crate::guard::{normalize_target, slug_for};
use crate::guard_store::LockStore;

/// 内存版锁存储。
#[derive(Debug, Default)]
pub struct FakeLockStore {
    /// 锁文件表：归一化目标路径 → 锁记录文本。
    files: RefCell<BTreeMap<String, String>>,
    /// 放弃日志的每一行。
    log: RefCell<Vec<String>>,
    /// 当前 Unix 秒。
    clock: Cell<u64>,
    /// `current_pid` 的返回值。
    pid: Cell<u32>,
    /// 每次 `sleep` 把时钟推进多少秒（不变量 2）。
    advance_per_sleep_secs: Cell<u64>,
    /// 累计睡了多少毫秒（用于断言"确实等待过"）。
    slept_millis: Cell<u64>,
    /// `prepare` 被调用了几次。
    prepare_calls: Cell<u32>,
    /// 让下一次 `create` 失败（模拟"刚好被别人抢先"）。
    fail_next_create: Cell<bool>,
    /// 让下一次 `read` 报 IO 错误（模拟"文件存在但读不出来"）。
    fail_next_read: Cell<bool>,
}

impl FakeLockStore {
    /// 造一个时钟停在 `now` 的空锁存储。
    #[must_use]
    pub fn at(now_unix: u64) -> Self {
        Self {
            clock: Cell::new(now_unix),
            pid: Cell::new(4242),
            advance_per_sleep_secs: Cell::new(1),
            ..Self::default()
        }
    }

    /// 预置一把别人持有的锁（用于复现"锁已被占"的各种分支）。
    pub fn place_lock(&self, target: &str, owner: &str, acquired_at_unix: u64) {
        let content = format!(
            "target={}\nowner={owner}\npid=999\ntask=TASK-099\nacquired_at_unix={acquired_at_unix}\nacquired_at_iso=unix:{acquired_at_unix}\nintent=测试预置的锁\n",
            normalize_target(target)
        );
        self.files
            .borrow_mut()
            .insert(normalize_target(target), content);
    }

    /// 预置一个**损坏**的锁文件（半截写入 / 被人手改坏）。
    pub fn place_corrupt_lock(&self, target: &str) {
        self.files
            .borrow_mut()
            .insert(normalize_target(target), "这不是锁记录\n".to_string());
    }

    /// 把时钟拨到 `now_unix`（用于构造时钟回拨：传一个比锁记录更早的值）。
    pub fn set_clock(&self, now_unix: u64) {
        self.clock.set(now_unix);
    }

    /// 设定每次 `sleep` 推进多少秒。
    pub fn set_advance_per_sleep(&self, secs: u64) {
        self.advance_per_sleep_secs.set(secs);
    }

    /// 让下一次 `create` 失败一次。
    pub fn fail_next_create_once(&self) {
        self.fail_next_create.set(true);
    }

    /// 让下一次 `read` 报一次 IO 错误。
    pub fn fail_next_read_once(&self) {
        self.fail_next_read.set(true);
    }

    /// 当前是否存在某个目标的锁。
    #[must_use]
    pub fn is_locked(&self, target: &str) -> bool {
        self.files.borrow().contains_key(&normalize_target(target))
    }

    /// 当前锁文件数。
    #[must_use]
    pub fn lock_count(&self) -> usize {
        self.files.borrow().len()
    }

    /// 某个目标的锁记录文本。
    #[must_use]
    pub fn content_of(&self, target: &str) -> Option<String> {
        self.files.borrow().get(&normalize_target(target)).cloned()
    }

    /// 放弃日志的全部行。
    #[must_use]
    pub fn log_lines(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    /// 累计睡眠毫秒数。
    #[must_use]
    pub fn slept_millis(&self) -> u64 {
        self.slept_millis.get()
    }

    /// `prepare` 调用次数。
    #[must_use]
    pub fn prepare_calls(&self) -> u32 {
        self.prepare_calls.get()
    }
}

impl LockStore for FakeLockStore {
    fn now_unix(&self) -> u64 {
        self.clock.get()
    }

    fn now_iso(&self) -> String {
        crate::guard_store::iso8601_utc(self.now_unix())
    }

    fn current_pid(&self) -> u32 {
        self.pid.get()
    }

    fn prepare(&self) -> Result<(), String> {
        self.prepare_calls.set(self.prepare_calls.get() + 1);
        Ok(())
    }

    fn read(&self, target: &str) -> Result<Option<String>, String> {
        if self.fail_next_read.replace(false) {
            return Err(format!("模拟读取 {} 失败", normalize_target(target)));
        }
        Ok(self.content_of(target))
    }

    fn create(&self, target: &str, content: &str) -> Result<(), String> {
        if self.fail_next_create.replace(false) {
            return Err("模拟：锁已被别人抢先创建".to_string());
        }
        let key = normalize_target(target);
        let mut files = self.files.borrow_mut();
        if files.contains_key(&key) {
            return Err(format!("模拟：{key} 的锁已存在"));
        }
        files.insert(key, content.to_string());
        Ok(())
    }

    fn remove(&self, target: &str) -> Result<(), String> {
        self.files.borrow_mut().remove(&normalize_target(target));
        Ok(())
    }

    fn remove_by_file_name(&self, lock_file_name: &str) -> Result<(), String> {
        // 反查：list() 的键是 slug，这里按 slug 匹配后删掉对应的目标
        let mut files = self.files.borrow_mut();
        if let Some(key) = files
            .keys()
            .find(|target| slug_for(target) == lock_file_name)
            .cloned()
        {
            files.remove(&key);
        }
        Ok(())
    }

    fn append_log(&self, line: &str) -> Result<(), String> {
        self.log.borrow_mut().push(line.to_string());
        Ok(())
    }

    fn list(&self) -> Result<BTreeMap<String, String>, String> {
        Ok(self
            .files
            .borrow()
            .iter()
            .map(|(target, content)| (slug_for(target), content.clone()))
            .collect())
    }

    fn sleep(&self, millis: u64) {
        self.slept_millis.set(self.slept_millis.get() + millis);
        // 不变量 2：睡眠推进时钟，否则超时分支永远走不到
        self.clock.set(
            self.clock
                .get()
                .saturating_add(self.advance_per_sleep_secs.get()),
        );
    }
}
