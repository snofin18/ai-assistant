//! `guard_runner.rs` 的单元测试：guard `acquire` 的等待 / 放弃 / 接管 / 回滚路径（ADR-0028）的白盒测试。
//!
//! **为什么单独一个文件**：`guard_runner.rs` 连同测试会超过单文件行数阈值
//! （AGENTS.md §5.3 的 600 行硬上限 / gov §5.4 的 > 600 警告）。用 `#[path]` 把 `mod tests`
//! 外置后两侧都回到阈值内，而测试**仍然是本模块的私有单元测试** —— `use super::*` 照旧能
//! 访问私有项，这一点与 `tests/` 目录下的集成测试有本质区别，不能混为一谈。
//! 声明处在 `guard_runner.rs` 末尾：`#[cfg(test)] #[path = "guard_runner_tests.rs"] mod tests;`。
//!
//! ## 组织方式
//! 按被测函数分组，每组前有一行 `// --- 函数名 ---` 分隔注释；命名遵循
//! `test_<被测单元>_<条件>_<期望>`（AGENTS.md §5.1）。

// 测试里允许 unwrap/expect/panic：断言失败就该立刻炸出来，包装成 Result 只会掩盖问题
// （AGENTS.md §5.5「tests/ 内可 allow」）。
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

use crate::guard_model::build_request;
use crate::guard_testkit::FakeLockStore;
use std::collections::BTreeMap;

/// 造一个 `acquire` 请求（owner 固定为 `codex-a`，超时 2 秒）。
fn acquire_request(targets: &[&str], force: bool) -> GuardRequest {
    let mut options = BTreeMap::new();
    options.insert("owner".to_string(), "codex-a".to_string());
    options.insert("task".to_string(), "TASK-030".to_string());
    options.insert("intent".to_string(), "追加 LEDGER 行".to_string());
    options.insert("timeout".to_string(), "2".to_string());
    build_request(
        Some("acquire"),
        &targets
            .iter()
            .map(|target| (*target).to_string())
            .collect::<Vec<String>>(),
        &options,
        force,
    )
    .expect("参数应合法")
}

/// 跑一次并返回 (退出码, 输出文本)。
fn run_capture(request: &GuardRequest, store: &FakeLockStore) -> (u8, String) {
    let mut sink: Vec<u8> = Vec::new();
    let code = run(request, store, &mut sink).expect("不应返回 GuardFailure");
    (code, String::from_utf8(sink).expect("应为 UTF-8"))
}

// --- 正向 ---

#[test]
fn test_acquire_on_empty_store_creates_lock_and_exits_zero() {
    let store = FakeLockStore::at(1000);
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_OK);
    assert!(text.contains("-- guard-result: ACQUIRED owner=codex-a targets=MEMORY.md"));
    assert!(store.is_locked("MEMORY.md"));
    let content = store.content_of("MEMORY.md").expect("应有锁记录");
    assert!(
        content.contains("owner=codex-a"),
        "锁记录要能被别人诊断：{content}"
    );
    assert!(content.contains("task=TASK-030"));
    assert!(content.contains("acquired_at_unix=1000"));
    assert!(store.prepare_calls() >= 1, "acquire 必须先确保锁目录存在");
}

#[test]
fn test_acquire_by_same_owner_reuses_instead_of_blocking() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-a", 900);
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_OK);
    assert!(text.contains("ACQUIRED"), "自己不该被自己挡住：{text}");
    assert_eq!(store.slept_millis(), 0, "复用不需要等待");
}

#[test]
fn test_acquire_multiple_targets_sorts_acquisition_order() {
    let store = FakeLockStore::at(1000);
    let (code, text) = run_capture(&acquire_request(&["B.md", "A.md"], false), &store);
    assert_eq!(code, EXIT_OK);
    assert!(
        text.contains("targets=A.md,B.md"),
        "D7：获取顺序必须与参数顺序无关，实际：{text}"
    );
    assert_eq!(store.lock_count(), 2);
}

// --- 超时放弃（不变量 1：四件事一件不少）---

#[test]
fn test_acquire_timeout_abandons_with_exit_code_five_and_full_report() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-b", 990);
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_LOCK_TIMEOUT, "D5 ①：专用退出码 5");
    assert!(
        text.contains("-- guard-result: ABANDONED target=MEMORY.md held_by=codex-b"),
        "D5 ②：机器可读结果行，实际：{text}"
    );
    assert!(
        store
            .log_lines()
            .iter()
            .any(|line| line.starts_with("ABANDONED owner=codex-a")),
        "D5 ③：放弃日志，实际：{:?}",
        store.log_lines()
    );
    assert!(text.contains("LEDGER.md"), "D5 ④：必须提醒台账义务");
    assert!(store.slept_millis() > 0, "应真的等待过，而不是立刻放弃");
}

#[test]
fn test_acquire_zero_timeout_abandons_without_sleeping() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-b", 990);
    let mut request = acquire_request(&["MEMORY.md"], false);
    request.timeout_secs = 0;
    let (code, _) = run_capture(&request, &store);
    assert_eq!(code, EXIT_LOCK_TIMEOUT);
    assert_eq!(
        store.slept_millis(),
        0,
        "超时 0 秒就该立刻放弃，不浪费 CI 时间"
    );
}

#[test]
fn test_acquire_reports_waiting_only_once_per_holder() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-b", 990);
    let (_, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    let waiting_lines = text
        .lines()
        .filter(|line| line.contains("-- guard-waiting:"))
        .count();
    assert_eq!(
        waiting_lines, 1,
        "不变量 5：同一个持有者只报一次，实际 {waiting_lines} 行"
    );
}

// --- 陈旧接管与强制接管（不变量 3）---

#[test]
fn test_acquire_stale_lock_is_taken_over_with_previous_record_printed() {
    let store = FakeLockStore::at(5000);
    store.place_lock("MEMORY.md", "codex-b", 1000); // 锁龄 4000 秒 > 默认 900
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_OK);
    assert!(text.contains("TAKEOVER reason=STALE"), "实际：{text}");
    assert!(text.contains("previous_owner=codex-b"), "必须说明接管了谁");
    assert!(
        text.contains("被接管者的完整锁记录"),
        "不变量 3：必须打印完整记录，实际：{text}"
    );
    assert!(
        store
            .log_lines()
            .iter()
            .any(|line| line.starts_with("TAKEOVER reason=STALE")),
        "接管也要写日志，实际：{:?}",
        store.log_lines()
    );
    let content = store.content_of("MEMORY.md").expect("应有新锁");
    assert!(content.contains("owner=codex-a"), "锁必须已换手：{content}");
}

#[test]
fn test_acquire_force_takes_over_a_fresh_lock() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-b", 999); // 锁龄 1 秒，绝不陈旧
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], true), &store);
    assert_eq!(code, EXIT_OK);
    assert!(text.contains("TAKEOVER reason=FORCED"), "实际：{text}");
}

// --- 时钟异常（ADR-0028 D6 / 验证方式 4）---

#[test]
fn test_acquire_future_lock_is_clock_anomaly_and_never_taken_over() {
    let store = FakeLockStore::at(1000);
    store.place_lock("MEMORY.md", "codex-b", 999_999); // 获取时刻在遥远的未来
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_LOCK_TIMEOUT, "时钟异常时判为等待，不是接管");
    assert!(
        text.contains("clock_anomaly=true"),
        "必须标注时钟异常，否则人类会以为对方在正常持锁：{text}"
    );
    assert!(
        !text.contains("TAKEOVER"),
        "一次时钟回拨不该导致所有锁被集体接管：{text}"
    );
}

// --- 多文件回滚（不变量 2）---

#[test]
fn test_acquire_failure_on_second_target_rolls_back_the_first() {
    let store = FakeLockStore::at(1000);
    store.place_lock("B.md", "codex-b", 999); // A 能拿到，B 拿不到
    let (code, text) = run_capture(&acquire_request(&["A.md", "B.md"], false), &store);
    assert_eq!(code, EXIT_LOCK_TIMEOUT);
    assert!(!store.is_locked("A.md"), "不变量 2：已获取的锁必须回滚");
    assert!(store.is_locked("B.md"), "别人的锁不能被动");
    assert!(text.contains("已回滚本次获取的 1 把锁"), "实际：{text}");
}

#[test]
fn test_rollback_skips_lock_that_changed_hands() {
    let store = FakeLockStore::at(1000);
    store.place_lock("B.md", "codex-b", 999);
    let (code, text) = run_capture(&acquire_request(&["A.md", "B.md"], false), &store);
    assert_eq!(code, EXIT_LOCK_TIMEOUT);
    // 回滚只删自己的锁；A 是自己的所以被删掉，不会误删 B
    assert!(!text.contains("回滚跳过"), "本例中 A 仍是自己的锁：{text}");
}

// --- 锁记录损坏（不变量 4）---

#[test]
fn test_acquire_corrupt_lock_without_force_is_io_failure_not_grant() {
    let store = FakeLockStore::at(1000);
    store.place_corrupt_lock("MEMORY.md");
    let mut sink: Vec<u8> = Vec::new();
    let failure = run(&acquire_request(&["MEMORY.md"], false), &store, &mut sink)
        .expect_err("损坏的锁必须显式失败");
    assert!(
        matches!(failure, GuardFailure::Corrupt(_)),
        "必须是 Corrupt 而不是 Io/Usage：{failure:?}"
    );
    assert_eq!(failure.exit_code(), crate::EXIT_IO);
    assert!(
        store.is_locked("MEMORY.md"),
        "损坏的锁文件不该被静默删除（否则等于假装它不存在）"
    );
    assert!(store.log_lines().is_empty(), "没放弃也没接管，不该写日志");
}

#[test]
fn test_acquire_corrupt_lock_with_force_replaces_it() {
    let store = FakeLockStore::at(1000);
    store.place_corrupt_lock("MEMORY.md");
    let (code, _) = run_capture(&acquire_request(&["MEMORY.md"], true), &store);
    assert_eq!(code, EXIT_OK, "--force 是人工接管的显式意图");
    let content = store.content_of("MEMORY.md").expect("应有新锁");
    assert!(content.contains("owner=codex-a"), "实际：{content}");
}

// --- create 竞争（不变量 6）---

#[test]
fn test_acquire_retries_when_create_loses_the_race() {
    let store = FakeLockStore::at(1000);
    store.fail_next_create_once();
    let (code, _) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_OK, "被抢先不是错误，应重试（不变量 6）");
    assert!(store.is_locked("MEMORY.md"));
    assert!(store.slept_millis() > 0, "重试前应退避一个轮询间隔");
}

// --- 用法错误 ---

#[test]
fn test_acquire_without_owner_is_usage_error() {
    let store = FakeLockStore::at(1000);
    let request = build_request(
        Some("acquire"),
        &["MEMORY.md".to_string()],
        &BTreeMap::new(),
        false,
    )
    .expect("build_request 不校验 owner，由 run_acquire 校验");
    let mut sink: Vec<u8> = Vec::new();
    let failure = run(&request, &store, &mut sink).expect_err("缺 owner 必须失败");
    assert!(
        matches!(failure, GuardFailure::Usage(_)),
        "实际：{failure:?}"
    );
    assert!(failure.to_string().contains("--owner"), "实际：{failure}");
}

// --- 时钟异常（ADR-0028 D6：宁可多等，不可误接管）---

#[test]
fn test_acquire_clock_rolled_back_is_anomaly_and_never_takes_over() {
    // 场景：锁是"正常"创建的（锁龄 1000 秒 > 陈旧阈值 900），随后系统时钟被拨回。
    // 如果只看锁龄，这把锁会被判成陈旧并被接管 —— 而真正的持有者可能还在写。
    // D6 要求：获取时刻落在未来时按「等待」处理并显式标注，绝不接管。
    let store = FakeLockStore::at(5000);
    store.place_lock("MEMORY.md", "codex-b", 4000);
    store.set_clock(1000); // 时钟回拨 4000 秒：4000 现在成了"未来"
    store.set_advance_per_sleep(1); // 替身时钟每次轮询推进 1 秒，超时分支可在 CI 里瞬时复现
    let (code, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    assert_eq!(code, EXIT_LOCK_TIMEOUT, "等不到就放弃，退出码 5");
    assert!(
        text.contains("clock_anomaly=true"),
        "必须显式标注时钟异常，否则人只会以为对方在磨蹭：{text}"
    );
    assert!(!text.contains("TAKEOVER"), "绝不能接管：{text}");
    let content = store.content_of("MEMORY.md").expect("锁应原样保留");
    assert!(
        content.contains("owner=codex-b"),
        "持有者必须仍是 codex-b，锁记录不得被改写：{content}"
    );
}

#[test]
fn test_acquire_abandonment_message_carries_clock_anomaly_flag() {
    // 放弃时那行机器可读结果也要带上 clock_anomaly：
    // 运行盘点只看这一行，缺了它就会把"时钟问题"误报成"对方长时间占锁"
    let store = FakeLockStore::at(5000);
    store.place_lock("MEMORY.md", "codex-b", 4000);
    store.set_clock(1000);
    store.set_advance_per_sleep(10); // 一次轮询就超过 2 秒超时，缩短等待轮数
    let (_, text) = run_capture(&acquire_request(&["MEMORY.md"], false), &store);
    let abandoned = text
        .lines()
        .find(|line| line.contains("-- guard-result: ABANDONED"))
        .unwrap_or_default();
    assert!(
        abandoned.contains("clock_anomaly=true"),
        "放弃行必须带时钟异常标记，实际：{abandoned}"
    );
    assert!(
        abandoned.contains("waited_ms=10000"),
        "等待时长按替身时钟计（1 次轮询 × 10 秒），实际：{abandoned}"
    );
}

#[test]
fn test_acquire_without_targets_is_usage_error() {
    let store = FakeLockStore::at(1000);
    let request = acquire_request(&[], false);
    let mut sink: Vec<u8> = Vec::new();
    assert!(matches!(
        run(&request, &store, &mut sink),
        Err(GuardFailure::Usage(_))
    ));
}
