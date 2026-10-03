//! `write_channel.rs` 的单元测试（ADR-0066）。
//!
//! 内存替身覆盖成功、占用超时、guard 超时、平台不支持、路径与输入边界；
//! Windows 专项用真实文件句柄验证“写句柄只共享读取”和“独占持有者触发超时且锁已释放”。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

use crate::guard_model::build_request;
use crate::guard_testkit::FakeLockStore;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::io::Cursor;

#[cfg(windows)]
use crate::guard_store::FileLockStore;

fn free_outcome() -> ProbeOutcome {
    ProbeOutcome {
        is_busy: false,
        reason: String::new(),
    }
}

fn busy_outcome(reason: &str) -> ProbeOutcome {
    ProbeOutcome {
        is_busy: true,
        reason: reason.to_string(),
    }
}

/// 内存目标替身：按脚本返回探测结果并记录写入。
#[derive(Debug)]
struct FakeTargetAccess {
    probes: RefCell<VecDeque<ProbeOutcome>>,
    writes: RefCell<Vec<(PathBuf, Vec<u8>)>>,
    probe_calls: Cell<usize>,
    probe_error: Option<String>,
}

impl FakeTargetAccess {
    fn new(probes: Vec<ProbeOutcome>) -> Self {
        Self {
            probes: RefCell::new(probes.into()),
            writes: RefCell::new(Vec::new()),
            probe_calls: Cell::new(0),
            probe_error: None,
        }
    }

    fn failing(reason: &str) -> Self {
        Self {
            probes: RefCell::new(VecDeque::new()),
            writes: RefCell::new(Vec::new()),
            probe_calls: Cell::new(0),
            probe_error: Some(reason.to_string()),
        }
    }

    fn writes(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.writes.borrow().clone()
    }

    fn probe_calls(&self) -> usize {
        self.probe_calls.get()
    }
}

impl TargetAccess for FakeTargetAccess {
    fn probe_exclusive(&self, _path: &Path) -> Result<ProbeOutcome, String> {
        self.probe_calls.set(self.probe_calls.get() + 1);
        if let Some(reason) = &self.probe_error {
            return Err(reason.clone());
        }
        Ok(self
            .probes
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(free_outcome))
    }

    fn write_target(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        self.writes
            .borrow_mut()
            .push((path.to_path_buf(), bytes.to_vec()));
        Ok(())
    }
}

/// 造一个 `write` 请求（底层仍是 guard acquire，进入 write 后只使用 targets/owner/timeout）。
fn write_request(target: &str, timeout_secs: u64) -> GuardRequest {
    let mut options = BTreeMap::new();
    options.insert("owner".to_string(), "codex-write".to_string());
    options.insert("task".to_string(), "TASK-224".to_string());
    options.insert("intent".to_string(), "测试唯一写通道".to_string());
    options.insert("timeout".to_string(), timeout_secs.to_string());
    build_request(Some("acquire"), &[target.to_string()], &options, false).expect("请求应合法")
}

/// 跑一次 `write_channel::run`，返回 (结果, stdout, access)。
fn run_with(
    request: &GuardRequest,
    store: &FakeLockStore,
    access: FakeTargetAccess,
    content: &[u8],
) -> (Result<u8, GuardFailure>, String, FakeTargetAccess) {
    let mut source = Cursor::new(content.to_vec());
    let mut sink: Vec<u8> = Vec::new();
    let result = run(
        request,
        Path::new("/repo"),
        store,
        &access,
        &mut source,
        &mut sink,
    );
    (
        result,
        String::from_utf8(sink).expect("输出应为 UTF-8"),
        access,
    )
}

#[test]
fn test_write_success_acquires_probes_writes_and_releases() {
    let store = FakeLockStore::at(1000);
    let request = write_request("MEMORY.md", 5);
    let (result, text, access) = run_with(
        &request,
        &store,
        FakeTargetAccess::new(vec![free_outcome()]),
        b"hello",
    );
    assert_eq!(result.expect("成功路径不应失败"), EXIT_OK);
    assert!(text.contains("-- guard-result: ACQUIRED"), "实际：{text}");
    assert!(text.contains("-- guard-result: RELEASED"), "实际：{text}");
    assert!(
        text.contains("-- write-result: WRITTEN target=MEMORY.md bytes=5"),
        "实际：{text}"
    );
    assert!(!store.is_locked("MEMORY.md"), "写后必须释放 guard 锁");
    assert_eq!(access.probe_calls(), 1);
    assert_eq!(access.writes().len(), 1);
    assert_eq!(
        access
            .writes()
            .first()
            .expect("应记录一次写入")
            .1
            .as_slice(),
        b"hello"
    );
}

#[test]
fn test_write_busy_target_times_out_with_exit_five_and_releases_lock() {
    let store = FakeLockStore::at(1000);
    let request = write_request("MEMORY.md", 1);
    let (result, text, access) = run_with(
        &request,
        &store,
        FakeTargetAccess::new(vec![
            busy_outcome("sharing_violation"),
            busy_outcome("sharing_violation"),
        ]),
        b"new",
    );
    assert_eq!(
        result.expect("超时是正常结果，不是工具故障"),
        EXIT_LOCK_TIMEOUT
    );
    assert!(text.contains("-- write-result: ABANDONED"), "实际：{text}");
    assert!(text.contains("LEDGER.md"), "必须提醒台账义务：{text}");
    assert_eq!(access.writes().len(), 0, "没有空闲时不得写入");
    assert!(!store.is_locked("MEMORY.md"), "超时后不得残留 guard 锁");
    assert!(
        store
            .log_lines()
            .iter()
            .any(|line| line.starts_with("WRITE_ABANDONED")),
        "必须写放弃日志：{:?}",
        store.log_lines()
    );
}

#[test]
fn test_write_guard_lock_timeout_does_not_probe_or_write() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-other", 999);
    let request = write_request("MEMORY.md", 0);
    let (result, text, access) = run_with(
        &request,
        &store,
        FakeTargetAccess::new(vec![free_outcome()]),
        b"new",
    );
    assert_eq!(result.expect("guard 超时也是正常结果"), EXIT_LOCK_TIMEOUT);
    assert_eq!(access.probe_calls(), 0, "没拿到 guard 锁前不得探测目标");
    assert_eq!(access.writes().len(), 0);
    assert!(store.is_locked("MEMORY.md"), "别人的锁必须保留");
    assert!(text.contains("guard-result: ABANDONED"), "实际：{text}");
}

#[test]
fn test_write_unsupported_probe_fails_closed_and_releases_lock() {
    let store = FakeLockStore::at(1000);
    let request = write_request("MEMORY.md", 5);
    let (result, text, access) = run_with(
        &request,
        &store,
        FakeTargetAccess::failing("not supported"),
        b"new",
    );
    let failure = result.expect_err("平台不支持必须失败");
    assert_eq!(failure.exit_code(), crate::EXIT_IO);
    assert!(failure.to_string().contains("not supported"));
    assert_eq!(access.writes().len(), 0);
    assert!(!store.is_locked("MEMORY.md"), "失败也要释放自己的锁");
    assert!(text.contains("RELEASED"), "实际：{text}");
}

#[test]
fn test_write_rejects_parent_directory_traversal() {
    let store = FakeLockStore::at(1000);
    let request = write_request("../outside.txt", 5);
    let (result, _text, access) = run_with(
        &request,
        &store,
        FakeTargetAccess::new(vec![free_outcome()]),
        b"new",
    );
    let failure = result.expect_err("路径穿越必须失败");
    assert_eq!(failure.exit_code(), crate::EXIT_USAGE);
    assert_eq!(access.probe_calls(), 0);
    assert_eq!(access.writes().len(), 0);
    assert_eq!(store.lock_count(), 0);
}

#[test]
fn test_write_rejects_stdin_over_hard_limit_before_acquiring_lock() {
    let store = FakeLockStore::at(1000);
    let request = write_request("MEMORY.md", 5);
    let over_limit = u64::try_from(MAX_WRITE_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut source = std::io::repeat(0).take(over_limit);
    let mut sink: Vec<u8> = Vec::new();
    let result = run(
        &request,
        Path::new("/repo"),
        &store,
        &FakeTargetAccess::new(vec![free_outcome()]),
        &mut source,
        &mut sink,
    );
    let failure = result.expect_err("超限必须失败");
    assert_eq!(failure.exit_code(), crate::EXIT_USAGE);
    assert_eq!(store.lock_count(), 0, "读输入失败不能先占锁");
}

#[cfg(windows)]
fn create_temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let root = std::env::temp_dir().join(format!(
        "xtask-write-channel-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("应能创建临时目录");
    root
}

#[cfg(windows)]
struct TempRoot(PathBuf);

#[cfg(windows)]
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
#[test]
fn test_windows_write_handle_allows_concurrent_read() {
    use std::io::Write as _;
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::mpsc;
    use std::time::Duration;

    let root = TempRoot(create_temp_root("read"));
    let target = root.0.join("target.txt");
    std::fs::write(&target, b"before").expect("应能创建目标");

    // 使用与生产写入相同的打开路径：只共享读取。
    let mut writer = open_target_for_write(&target).expect("writer handle 应能打开");
    writer.write_all(b"before").expect("writer 应能写入");
    let (sender, receiver) = mpsc::channel();
    let reader_path = target.clone();
    let reader = std::thread::spawn(move || {
        let result = std::fs::read(&reader_path);
        sender.send(result).ok();
    });

    let read_result = receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("读取者不应被写句柄阻塞");
    assert_eq!(read_result.expect("读取应成功"), b"before");
    drop(writer);
    reader.join().expect("读取线程应正常结束");

    // 显式验证测试使用与生产相同的 share mode。
    let _ = std::fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(&target)
        .expect("writer 已关闭，独占探测应成功");
}

#[cfg(windows)]
#[test]
fn test_windows_exclusive_holder_causes_timeout_and_releases_lock() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = TempRoot(create_temp_root("busy"));
    let target_path = root.0.join("target.txt");
    std::fs::write(&target_path, b"before").expect("应能创建目标");
    let exclusive = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&target_path)
        .expect("独占持有者应能打开");

    let store = FileLockStore::new(root.0.clone());
    let request = write_request("target.txt", 0);
    let mut source = Cursor::new(b"after".to_vec());
    let mut sink: Vec<u8> = Vec::new();
    let result = run(
        &request,
        &root.0,
        &store,
        &FileSystemTargetAccess,
        &mut source,
        &mut sink,
    );

    assert_eq!(result.expect("占用超时应返回正常退出码"), EXIT_LOCK_TIMEOUT);
    drop(exclusive);
    assert_eq!(
        std::fs::read(&target_path).expect("目标应仍可读"),
        b"before"
    );
    assert!(
        store.read("target.txt").expect("读锁状态应成功").is_none(),
        "超时后 guard 锁必须已释放"
    );
}
