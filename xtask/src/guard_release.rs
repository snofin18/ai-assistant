//! # `xtask guard` 的 `release` / `status` / `reap` 三个操作
//!
//! 职责：锁的**收尾与观测** —— 释放自己持有的锁、列出当前所有锁、清理陈旧锁。
//! `acquire`（唯一会阻塞的路径）在 `guard_runner.rs`。
//!
//! ## 三个操作各自的语义要点
//! - `release`：owner 必须匹配，否则**拒绝**（退出码 1，ADR-0028 D8）。释放一个本来就不存在的锁
//!   是**幂等成功**（目的状态"没有这把锁"已经成立），但必须打印 `NOT_LOCKED` 而不是静默。
//! - `status`：空锁目录打印 `NONE`（ADR-0028 验证方式 1）。**不可解析的锁也要列出来**并标注 ——
//!   假装它不存在，等于把一个需要人工处理的故障藏起来。
//! - `reap`：只清理**锁龄 ≥ `--stale-after`** 的锁，外加记录损坏的锁；每清理一把都打印原持有者，
//!   让"我清掉了谁的锁"可追溯。
//!
//! ## 边界（不做什么）
//! - 不做判定：锁龄与陈旧阈值由 `guard.rs` 的纯函数给出。
//! - 不碰文件系统：全部经 [`LockStore`]。
//! - `reap` 不判断进程是否还活着（ADR-0028 D6 明确否决 PID 存活探测）。
//!
//! ## 不变量
//! 1. **删除别人的锁必须有显式授权**：`release` 靠 owner 匹配或 `--force`；`reap` 靠陈旧阈值。
//!    没有任何一条路径会"顺手"删掉一把新鲜的、属于别人的锁。
//! 2. 输出确定：`status` / `reap` 都按锁文件名排序（`LockStore::list` 返回 `BTreeMap`）。
//! 3. 每个操作都至少打印一行 `-- guard-result:` / `-- guard-status:` / `-- guard-reap:`，
//!    **绝不静默成功**（铁律 1）。
//!
//! 相关：`docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/src/guard_runner.rs`

use std::io::Write;

use crate::guard::{LockRecord, lock_age_secs, parse_lock_record, sort_targets};
use crate::guard_model::{GuardFailure, GuardRequest, LockRead, read_lock, write_line};
use crate::guard_store::LockStore;
use crate::{EXIT_FINDINGS, EXIT_OK};

/// 一把锁的状态快照（`status` / `reap` 用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockSnapshot {
    /// 锁文件名（`guard::slug_for` 的产物）。
    pub file_name: String,
    /// 解析出的记录；解析失败时是原因。
    pub record: Result<LockRecord, String>,
    /// 锁龄（秒）；记录不可解析时为 `None`。
    pub age_secs: Option<u64>,
    /// 是否已达到陈旧阈值。
    pub is_stale: bool,
    /// 记录是否不可解析（`reap` 对这类锁单独处理）。
    pub is_corrupt: bool,
}

/// `release`：释放自己持有的锁；owner 不匹配且未 `--force` → 退出码 1。
///
/// # Errors
/// 缺 `--owner`（且未 `--force`）、缺目标、或文件系统失败时返回 [`GuardFailure`]。
pub fn run_release(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    if request.owner.is_empty() && !request.force {
        return Err(GuardFailure::Usage(
            "release 必须给 `--owner`（要证明你有权释放这把锁），或用 `--force` 人工强制释放"
                .to_string(),
        ));
    }
    if request.targets.is_empty() {
        return Err(GuardFailure::Usage(
            "release 至少要给一个目标路径".to_string(),
        ));
    }
    let mut refused_any = false;
    for target in sort_targets(&request.targets) {
        if release_one(request, store, output, &target)? {
            refused_any = true;
        }
    }
    // ADR-0028 D8：owner 不匹配且未 --force → 退出码 1（"有阻塞级发现项"的语义在这里成立：
    // 你以为释放了，其实没释放，接下来的写回会覆盖别人的内容）
    Ok(if refused_any { EXIT_FINDINGS } else { EXIT_OK })
}

/// 释放单个目标；返回 `true` 表示**被拒绝**（owner 不匹配）。
fn release_one(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
    target: &str,
) -> Result<bool, GuardFailure> {
    match read_lock(store, target)? {
        LockRead::Absent => {
            // 目的状态已经成立 → 幂等成功，但必须说出来（不变量 3）
            write_line(
                output,
                &format!("-- guard-result: NOT_LOCKED target={target}"),
            )?;
            Ok(false)
        }
        LockRead::Present(record) => {
            if record.owner != request.owner && !request.force {
                write_line(
                    output,
                    &format!(
                        "-- guard-result: REFUSED target={target} held_by={} held_task={} \
                         reason=OWNER_MISMATCH（`--force` 可强制释放，但那会丢掉别人的写回机会）",
                        record.owner, record.task
                    ),
                )?;
                return Ok(true);
            }
            store.remove(target)?;
            write_line(
                output,
                &format!(
                    "-- guard-result: RELEASED target={target} previous_owner={}",
                    record.owner
                ),
            )?;
            Ok(false)
        }
        LockRead::Corrupt(reason) => {
            if !request.force {
                return Err(GuardFailure::Corrupt(reason));
            }
            store.remove(target)?;
            write_line(
                output,
                &format!(
                    "-- guard-result: RELEASED target={target} note=锁记录损坏，--force 直接删除"
                ),
            )?;
            Ok(false)
        }
    }
}

/// `status`：列出所有锁（空目录打印 `NONE`）。
///
/// # Errors
/// 锁目录不可读或写输出失败时返回 [`GuardFailure`]。
pub fn run_status(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    let snapshots = collect_snapshots(store, request.stale_after_secs)?;
    if snapshots.is_empty() {
        write_line(
            output,
            "-- guard-status: NONE（锁目录下没有任何锁；这不代表没人正在改文件，只代表没人用 guard）",
        )?;
        return Ok(EXIT_OK);
    }
    write!(output, "{}", render_status(&snapshots))
        .map_err(|error| GuardFailure::Io(error.to_string()))?;
    Ok(EXIT_OK)
}

/// `reap`：清理所有陈旧锁与损坏锁，逐个打印被清理者的信息（可追溯）。
///
/// # Errors
/// 锁目录不可读、删除失败或写输出失败时返回 [`GuardFailure`]。
pub fn run_reap(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    let snapshots = collect_snapshots(store, request.stale_after_secs)?;
    let stale: Vec<&LockSnapshot> = snapshots
        .iter()
        .filter(|snapshot| snapshot.is_stale || snapshot.is_corrupt)
        .collect();
    if stale.is_empty() {
        write_line(
            output,
            &format!(
                "-- guard-reap: NONE（{} 把锁都不是陈旧的，阈值 {} 秒）",
                snapshots.len(),
                request.stale_after_secs
            ),
        )?;
        return Ok(EXIT_OK);
    }
    // 先记下数量：下面 `for snapshot in stale` 会把它移动掉（借用检查器不接受先用后移）
    let stale_count = stale.len();
    for snapshot in stale {
        reap_one(store, output, snapshot)?;
    }
    write_line(
        output,
        &format!(
            "-- guard-reap: 共清理 {} 把锁（保留 {} 把）",
            stale_count,
            snapshots.len() - stale_count
        ),
    )?;
    Ok(EXIT_OK)
}

/// 清理单把锁并打印被清理者的信息（不静默）。
fn reap_one(
    store: &dyn LockStore,
    output: &mut dyn Write,
    snapshot: &LockSnapshot,
) -> Result<(), GuardFailure> {
    let line = match &snapshot.record {
        Ok(record) => format!(
            "-- guard-reap: REMOVED {} owner={} age_secs={} task={} target={}",
            snapshot.file_name,
            record.owner,
            snapshot.age_secs.unwrap_or(0),
            record.task,
            record.target
        ),
        // 损坏的锁没有 target 可用，只能按文件名删（这就是 LockStore::remove_by_file_name 存在的原因）
        Err(reason) => format!(
            "-- guard-reap: REMOVED_CORRUPT {} reason={reason}",
            snapshot.file_name
        ),
    };
    write_line(output, &line)?;
    store.append_log(&line.replace("-- guard-reap: ", "REAP "))?;
    store
        .remove_by_file_name(&snapshot.file_name)
        .map_err(GuardFailure::Io)
}

/// 列出锁目录并给每把锁算出锁龄与是否陈旧。
fn collect_snapshots(
    store: &dyn LockStore,
    stale_after_secs: u64,
) -> Result<Vec<LockSnapshot>, GuardFailure> {
    let now = store.now_unix();
    let files = store.list()?;
    Ok(files
        .into_iter()
        .map(|(file_name, content)| snapshot_of(&file_name, &content, now, stale_after_secs))
        .collect())
}

/// 由锁文件内容构造一个快照。
fn snapshot_of(
    file_name: &str,
    content: &str,
    now_unix: u64,
    stale_after_secs: u64,
) -> LockSnapshot {
    match parse_lock_record(content) {
        Ok(record) => {
            let age = lock_age_secs(&record, now_unix);
            LockSnapshot {
                is_stale: age >= stale_after_secs,
                age_secs: Some(age),
                record: Ok(record),
                is_corrupt: false,
                file_name: file_name.to_string(),
            }
        }
        Err(reason) => LockSnapshot {
            record: Err(reason),
            age_secs: None,
            // 损坏锁**不**自动判为陈旧：`status` 必须让人看见"这里有个坏锁"，
            // 而 `reap` 删它是显式动作（会打印 REMOVED_CORRUPT 并写日志）
            is_stale: false,
            is_corrupt: true,
            file_name: file_name.to_string(),
        },
    }
}

/// 渲染 `status` 的输出（纯函数，便于白盒断言）。
#[must_use]
pub fn render_status(snapshots: &[LockSnapshot]) -> String {
    let mut text = format!("-- guard-status: {} 把锁\n", snapshots.len());
    for snapshot in snapshots {
        text.push_str(&render_snapshot(snapshot));
    }
    text
}

/// 渲染单个快照为一行。
fn render_snapshot(snapshot: &LockSnapshot) -> String {
    match &snapshot.record {
        Ok(record) => format!(
            "  {} owner={} age_secs={} stale={} task={} target={} intent={}\n",
            snapshot.file_name,
            record.owner,
            snapshot.age_secs.unwrap_or(0),
            snapshot.is_stale,
            record.task,
            record.target,
            record.intent
        ),
        Err(reason) => format!("  {} CORRUPT {reason}\n", snapshot.file_name),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::guard_model::build_request;
    // `run` 是分派入口，住在 guard_runner；本文件的测试要跑完整链路（含 release/reap 分支）
    use crate::guard_runner::run;
    use crate::guard_testkit::FakeLockStore;
    use std::collections::BTreeMap;

    /// 造一个请求；`stale_after` 便于构造陈旧锁。
    fn request(
        operation: &str,
        targets: &[&str],
        owner: &str,
        stale_after: u64,
        force: bool,
    ) -> GuardRequest {
        let mut options = BTreeMap::new();
        if !owner.is_empty() {
            options.insert("owner".to_string(), owner.to_string());
        }
        options.insert("stale-after".to_string(), stale_after.to_string());
        build_request(
            Some(operation),
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

    // --- release ---

    #[test]
    fn test_release_own_lock_succeeds_and_removes_it() {
        let store = FakeLockStore::at(1000);
        store.place_lock("MEMORY.md", "codex-a", 900);
        let (code, text) = run_capture(
            &request("release", &["MEMORY.md"], "codex-a", 900, false),
            &store,
        );
        assert_eq!(code, EXIT_OK);
        assert!(text.contains("-- guard-result: RELEASED target=MEMORY.md previous_owner=codex-a"));
        assert!(!store.is_locked("MEMORY.md"));
    }

    #[test]
    fn test_release_someone_elses_lock_is_refused_with_exit_one() {
        let store = FakeLockStore::at(1000);
        store.place_lock("MEMORY.md", "codex-b", 999);
        let (code, text) = run_capture(
            &request("release", &["MEMORY.md"], "codex-a", 900, false),
            &store,
        );
        assert_eq!(code, EXIT_FINDINGS, "ADR-0028 D8：owner 不匹配 → 退出码 1");
        assert!(
            text.contains("REFUSED") && text.contains("held_by=codex-b"),
            "实际：{text}"
        );
        assert!(store.is_locked("MEMORY.md"), "不变量 1：不得删别人的新鲜锁");
    }

    #[test]
    fn test_release_with_force_removes_someone_elses_lock() {
        let store = FakeLockStore::at(1000);
        store.place_lock("MEMORY.md", "codex-b", 999);
        let (code, text) = run_capture(&request("release", &["MEMORY.md"], "", 900, true), &store);
        assert_eq!(code, EXIT_OK);
        assert!(text.contains("RELEASED"), "实际：{text}");
        assert!(!store.is_locked("MEMORY.md"));
    }

    #[test]
    fn test_release_absent_lock_is_idempotent_but_not_silent() {
        let store = FakeLockStore::at(1000);
        let (code, text) = run_capture(
            &request("release", &["MEMORY.md"], "codex-a", 900, false),
            &store,
        );
        assert_eq!(code, EXIT_OK, "目的状态已成立 → 幂等成功");
        assert!(text.contains("NOT_LOCKED"), "不变量 3：必须说出来，不静默");
    }

    #[test]
    fn test_release_without_owner_or_force_is_usage_error() {
        let store = FakeLockStore::at(1000);
        let mut sink: Vec<u8> = Vec::new();
        let failure = run(
            &request("release", &["MEMORY.md"], "", 900, false),
            &store,
            &mut sink,
        )
        .expect_err("必须报错");
        assert!(
            matches!(failure, GuardFailure::Usage(_)),
            "实际：{failure:?}"
        );
    }

    #[test]
    fn test_release_corrupt_lock_without_force_is_corrupt_failure() {
        let store = FakeLockStore::at(1000);
        store.place_corrupt_lock("MEMORY.md");
        let mut sink: Vec<u8> = Vec::new();
        let failure = run(
            &request("release", &["MEMORY.md"], "codex-a", 900, false),
            &store,
            &mut sink,
        )
        .expect_err("必须报错");
        assert!(
            matches!(failure, GuardFailure::Corrupt(_)),
            "实际：{failure:?}"
        );
    }

    // --- status ---

    #[test]
    fn test_status_on_empty_lock_directory_prints_none() {
        let store = FakeLockStore::at(1000);
        let (code, text) = run_capture(&request("status", &[], "", 900, false), &store);
        assert_eq!(code, EXIT_OK, "ADR-0028 验证方式 1");
        assert!(text.contains("-- guard-status: NONE"), "实际：{text}");
    }

    #[test]
    fn test_status_lists_locks_with_age_and_staleness() {
        let store = FakeLockStore::at(5000);
        store.place_lock("MEMORY.md", "codex-b", 1000); // 锁龄 4000
        store.place_lock("LEDGER.md", "codex-c", 4990); // 锁龄 10
        let (_, text) = run_capture(&request("status", &[], "", 900, false), &store);
        assert!(text.contains("-- guard-status: 2 把锁"), "实际：{text}");
        assert!(
            text.contains("age_secs=4000") && text.contains("stale=true"),
            "实际：{text}"
        );
        assert!(
            text.contains("age_secs=10") && text.contains("stale=false"),
            "实际：{text}"
        );
    }

    #[test]
    fn test_status_shows_corrupt_lock_instead_of_hiding_it() {
        let store = FakeLockStore::at(1000);
        store.place_corrupt_lock("MEMORY.md");
        let (_, text) = run_capture(&request("status", &[], "", 900, false), &store);
        assert!(text.contains("CORRUPT"), "坏锁必须可见，实际：{text}");
    }

    #[test]
    fn test_status_output_is_deterministic() {
        let store = FakeLockStore::at(5000);
        store.place_lock("MEMORY.md", "codex-b", 1000);
        store.place_lock("LEDGER.md", "codex-c", 2000);
        let first = run_capture(&request("status", &[], "", 900, false), &store).1;
        let second = run_capture(&request("status", &[], "", 900, false), &store).1;
        assert_eq!(first, second, "不变量 2：输出确定");
    }

    // --- reap ---

    #[test]
    fn test_reap_removes_only_stale_locks_and_logs_them() {
        let store = FakeLockStore::at(5000);
        store.place_lock("MEMORY.md", "codex-b", 1000); // 锁龄 4000 > 900 → 清理
        store.place_lock("LEDGER.md", "codex-c", 4990); // 锁龄 10 → 保留
        let (code, text) = run_capture(&request("reap", &[], "", 900, false), &store);
        assert_eq!(code, EXIT_OK);
        assert!(
            text.contains("REMOVED") && text.contains("owner=codex-b"),
            "实际：{text}"
        );
        assert!(!store.is_locked("MEMORY.md"));
        assert!(store.is_locked("LEDGER.md"), "不变量 1：新鲜锁不得被清理");
        assert!(
            store
                .log_lines()
                .iter()
                .any(|line| line.starts_with("REAP REMOVED")),
            "清理要可追溯，实际：{:?}",
            store.log_lines()
        );
    }

    #[test]
    fn test_reap_removes_corrupt_lock_by_file_name() {
        let store = FakeLockStore::at(1000);
        store.place_corrupt_lock("MEMORY.md");
        let (code, text) = run_capture(&request("reap", &[], "", 900, false), &store);
        assert_eq!(code, EXIT_OK);
        assert!(text.contains("REMOVED_CORRUPT"), "实际：{text}");
        assert_eq!(store.lock_count(), 0);
    }

    #[test]
    fn test_reap_without_stale_locks_reports_none() {
        let store = FakeLockStore::at(1000);
        store.place_lock("MEMORY.md", "codex-b", 999);
        let (code, text) = run_capture(&request("reap", &[], "", 900, false), &store);
        assert_eq!(code, EXIT_OK);
        assert!(text.contains("-- guard-reap: NONE"), "实际：{text}");
        assert!(store.is_locked("MEMORY.md"));
    }

    // --- 快照构造 ---

    #[test]
    fn test_snapshot_of_marks_exact_threshold_as_stale() {
        // 负向边界：判据是「锁龄 >= 阈值」
        let at_threshold = snapshot_of("a.lock", &lock_text(1000), 1900, 900);
        assert!(at_threshold.is_stale);
        let just_below = snapshot_of("a.lock", &lock_text(1000), 1899, 900);
        assert!(!just_below.is_stale);
    }

    /// 造一段锁记录文本（获取时刻可指定）。
    fn lock_text(acquired_at_unix: u64) -> String {
        format!(
            "target=MEMORY.md\nowner=codex-b\npid=999\ntask=TASK-099\nacquired_at_unix={acquired_at_unix}\nacquired_at_iso=unix:{acquired_at_unix}\nintent=测试\n"
        )
    }
}
