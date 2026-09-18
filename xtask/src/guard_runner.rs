//! # `xtask guard` 的分派与 `acquire` 路径（ADR-0028 D5/D7 的落地）
//!
//! 职责：① 把请求分派到四个操作；② 实现最复杂的那条路径 —— `acquire` 的
//! 「原子创建 → 等待轮询 → 超时放弃 → 陈旧接管 → 多文件回滚」。
//! `release` / `status` / `reap` 在 `guard_release.rs`，类型与共享小工具在 `guard_model.rs`。
//!
//! ## 为什么 acquire 值得单独一份文件
//! 它是唯一**会阻塞**的操作，也是唯一需要「放弃通报」的操作：退出码 5、机器可读结果行、
//! 放弃日志、以及提醒调用方履行 `LEDGER.md` 台账义务，四件事必须同时发生（ADR-0028 D5）。
//! 少任何一件，「放弃」就退化成「静默失败」—— 而静默失败是本项目铁律 1 的头号敌人。
//!
//! ## 边界（不做什么）
//! - 不做判定：`guard.rs::decide_acquire` 说了算，本文件只执行判定结果。
//! - 不碰文件系统：全部经 [`LockStore`]，因此可用内存替身做白盒测试。
//! - 不给 `--owner` 兜默认值：没有 owner 就无法区分"自己已持锁"与"别人持锁"，
//!   也无法在别人超时放弃时说明是谁占着 —— 缺 owner 一律用法错误。
//!
//! ## 不变量
//! 1. **放弃必须通报**：四件事一件不少，且只有 [`finish_abandonment`] 这一处实现
//!    （避免"通报"被某个分支漏掉）。
//! 2. **多文件失败必须回滚**：本次已拿到的锁全部释放后再放弃；回滚时**只删自己的锁**
//!    （别人可能已接管，删错锁比留一把锁更糟）。
//! 3. **接管必须打印被接管者的完整锁记录并写放弃日志**，不静默。
//! 4. 锁记录损坏**不得**当成"没有锁"→ 退出码 4，除非显式 `--force`。
//! 5. 输出确定且不刷屏：同一个持有者只打印一次"在等谁"。
//! 6. `create` 失败**不是错误**：那是"刚好被别人抢先"，必须回到循环重新判定
//!    （这正是 `create_new` 原子性的意义 —— 竞争由文件系统裁决，不由我们猜）。
//!
//! 相关：`docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/src/guard.rs`

use std::io::Write;

use crate::guard::{
    Acquisition, LockRecord, POLL_INTERVAL_MILLIS, TakeOverReason, decide_acquire, lock_age_secs,
    normalize_target, render_lock_record, sort_targets,
};
use crate::guard_model::{
    GuardFailure, GuardRequest, LockRead, Operation, indent_block, read_lock, write_line,
};
use crate::guard_store::LockStore;
use crate::{EXIT_LOCK_TIMEOUT, EXIT_OK};

/// 执行一次 guard 调用，返回退出码。
///
/// # Errors
/// 用法错误、IO 失败、或锁记录损坏且未 `--force` 时返回 [`GuardFailure`]。
pub fn run(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    match request.operation {
        Operation::Acquire => run_acquire(request, store, output),
        Operation::Release => crate::guard_release::run_release(request, store, output),
        Operation::Status => crate::guard_release::run_status(request, store, output),
        Operation::Reap => crate::guard_release::run_reap(request, store, output),
    }
}

/// `acquire`：按序获取全部目标；任一失败 → 回滚已获取的 → 放弃并通报。
fn run_acquire(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    validate_acquire_request(request)?;
    store.prepare()?;
    let mut acquired: Vec<String> = Vec::new();
    // D7 的排序在 build_request 里做过，这里再排一次，保证任何调用路径都安全
    for target in sort_targets(&request.targets) {
        if let Err(abandonment) = acquire_one(request, store, output, &target) {
            return finish_abandonment(&acquired, request, store, output, abandonment);
        }
        acquired.push(target);
    }
    write_line(
        output,
        &format!(
            "-- guard-result: ACQUIRED owner={} targets={}",
            request.owner,
            acquired.join(",")
        ),
    )?;
    Ok(EXIT_OK)
}

/// 校验 `acquire` 的必填参数（缺 owner / 缺目标都是用法错误，不给默认值）。
fn validate_acquire_request(request: &GuardRequest) -> Result<(), GuardFailure> {
    if request.owner.is_empty() {
        return Err(GuardFailure::Usage(
            "acquire 必须给 `--owner`（会话级唯一，例如 `codex-1a2b3c4d` 或 `TASK-030`）。\
             没有 owner 就无法判断「是不是自己已经拿着锁」，也无法在别人超时放弃时说明是谁占着"
                .to_string(),
        ));
    }
    if request.targets.is_empty() {
        return Err(GuardFailure::Usage(
            "acquire 至少要给一个目标路径，例如 `xtask guard acquire MEMORY.md`".to_string(),
        ));
    }
    Ok(())
}

/// `acquire` 单个目标失败的两类原因。
enum Abandonment {
    /// 等待超时 → 退出码 5 + 通报。
    Timeout {
        /// 机器可读结果行的字段部分。
        message: String,
    },
    /// IO 错误或锁记录损坏 → 原样上抛（**不**能伪装成"放弃成功"）。
    Fatal(GuardFailure),
}

/// 放弃时的收尾：回滚 → 放弃日志 → 结果行 → 台账提醒 → 退出码 5（不变量 1）。
fn finish_abandonment(
    acquired: &[String],
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
    abandonment: Abandonment,
) -> Result<u8, GuardFailure> {
    // Fatal 分支：把错误原样交回 main.rs 呈现，退出码由 GuardFailure 决定（不是 5）
    let message = match abandonment {
        Abandonment::Fatal(failure) => return Err(failure),
        Abandonment::Timeout { message } => message,
    };
    rollback(acquired, request, store, output)?;
    // D5 ③：放弃日志（取证用，不入库）
    store.append_log(&format!("ABANDONED owner={} {message}", request.owner))?;
    // D5 ②：机器可读结果行（CI 与脚本可 grep）
    write_line(output, &format!("-- guard-result: ABANDONED {message}"))?;
    write_line(output, &format!("已回滚本次获取的 {} 把锁", acquired.len()))?;
    // D5 ④ 是协议义务（工具做不到），故在输出里明文提醒 —— 缺它"通报"就只是"日志"
    write_line(
        output,
        "通报义务（ADR-0028 D5 ④）：请在 LEDGER.md 追加一行，说明放弃了什么、为什么。",
    )?;
    // D5 ①：专用退出码，让"没拿到锁"与"工具坏了"可区分
    Ok(EXIT_LOCK_TIMEOUT)
}

/// 获取单个目标的锁：内含「判定 → 创建/接管/等待 → 超时放弃」的完整循环。
fn acquire_one(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
    target: &str,
) -> Result<(), Abandonment> {
    let started_at = store.now_unix();
    let timeout_millis = request.timeout_secs.saturating_mul(1000);
    // 不变量 5：已经打印过"在等谁"就不再重复，避免长等待把输出刷成噪声
    let mut reported_holder: Option<String> = None;
    loop {
        let decision = decide_for(request, store, target).map_err(Abandonment::Fatal)?;
        // 必须在 match 之前判定：`Wait { held }` 分支会把 `held` 从 `decision` 里移出，
        // 之后再 `matches!(decision, ..)` 就是"部分移动后使用"（E0382）
        let is_anomaly = matches!(decision, Acquisition::ClockAnomaly { .. });
        match decision {
            Acquisition::Grant => {
                if try_create(request, store, target) {
                    return Ok(());
                }
                // 不变量 6：被抢先了，回循环重判
            }
            Acquisition::Reuse => return Ok(()),
            Acquisition::TakeOver { held, reason } => {
                take_over(store, output, &held, reason, target).map_err(Abandonment::Fatal)?;
                if try_create(request, store, target) {
                    return Ok(());
                }
                // 接管后又被第三人抢先：接管不是独占权，回循环重判
            }
            Acquisition::Wait { held } | Acquisition::ClockAnomaly { held } => {
                let waited_millis = store
                    .now_unix()
                    .saturating_sub(started_at)
                    .saturating_mul(1000);
                if waited_millis >= timeout_millis {
                    return Err(Abandonment::Timeout {
                        message: abandonment_message(
                            target,
                            &held,
                            store.now_unix(),
                            waited_millis,
                            is_anomaly,
                        ),
                    });
                }
                report_waiting(
                    output,
                    target,
                    &held,
                    store.now_unix(),
                    is_anomaly,
                    &mut reported_holder,
                )
                .map_err(Abandonment::Fatal)?;
            }
        }
        store.sleep(POLL_INTERVAL_MILLIS);
    }
}

/// 读锁并交给 `decide_acquire` 判定；没有锁时直接返回 `Grant`。
///
/// 锁记录损坏时：给了 `--force` 就删掉重建（人工接管的语义），否则上抛 `Corrupt`（不变量 4）。
fn decide_for(
    request: &GuardRequest,
    store: &dyn LockStore,
    target: &str,
) -> Result<Acquisition, GuardFailure> {
    match read_lock(store, target)? {
        LockRead::Absent => Ok(Acquisition::Grant),
        LockRead::Present(record) => Ok(decide_acquire(
            Some(&record),
            store.now_unix(),
            request.stale_after_secs,
            &request.owner,
            request.force,
        )),
        LockRead::Corrupt(reason) => {
            if request.force {
                store.remove(target)?;
                Ok(Acquisition::Grant)
            } else {
                Err(GuardFailure::Corrupt(reason))
            }
        }
    }
}

/// 尝试原子创建锁；返回是否成功（`false` = 已被别人抢先，调用方应重试）。
///
/// 刻意**不**返回 `Result`：`LockStore::create` 的 IO 失败与"已被抢先"在本层是**同一种处置**
/// （回循环重判），区分开只会让两处调用点各多写一层 `?`（clippy `unnecessary_wraps`）。
fn try_create(request: &GuardRequest, store: &dyn LockStore, target: &str) -> bool {
    let record = LockRecord {
        target: normalize_target(target),
        owner: request.owner.clone(),
        pid: store.current_pid(),
        task: request.task.clone(),
        acquired_at_unix: store.now_unix(),
        acquired_at_iso: store.now_iso(),
        intent: request.intent.clone(),
    };
    store.create(target, &render_lock_record(&record)).is_ok()
}

/// 接管一把锁：打印被接管者的完整记录（不变量 3）、写放弃日志、删旧锁。
fn take_over(
    store: &dyn LockStore,
    output: &mut dyn Write,
    held: &LockRecord,
    reason: TakeOverReason,
    target: &str,
) -> Result<(), GuardFailure> {
    write_line(
        output,
        &format!(
            "-- guard-result: TAKEOVER reason={} target={} previous_owner={} previous_age_secs={}",
            reason.label(),
            target,
            held.owner,
            lock_age_secs(held, store.now_unix())
        ),
    )?;
    write_line(
        output,
        &format!(
            "被接管者的完整锁记录：\n{}",
            indent_block(&render_lock_record(held))
        ),
    )?;
    store.append_log(&format!(
        "TAKEOVER reason={} target={} previous_owner={} previous_pid={} previous_task={}",
        reason.label(),
        target,
        held.owner,
        held.pid,
        held.task
    ))?;
    store.remove(target).map_err(GuardFailure::Io)
}

/// 回滚本次已获取的锁（不变量 2）。
fn rollback(
    acquired: &[String],
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<(), GuardFailure> {
    for target in acquired {
        // 只释放**确实是自己**的锁：回滚期间别人可能已经接管，删错锁比留一把锁更糟
        let is_ours = matches!(read_lock(store, target)?, LockRead::Present(record) if record.owner == request.owner);
        if is_ours {
            store.remove(target)?;
        } else {
            write_line(
                output,
                &format!("回滚跳过 {target}：锁已不是自己的，不删别人的锁"),
            )?;
        }
    }
    Ok(())
}

/// 等待期间打印"在等谁"；同一个持有者只打印一次（不变量 5）。
fn report_waiting(
    output: &mut dyn Write,
    target: &str,
    held: &LockRecord,
    now_unix: u64,
    is_anomaly: bool,
    reported_holder: &mut Option<String>,
) -> Result<(), GuardFailure> {
    if reported_holder.as_deref() == Some(held.owner.as_str()) {
        return Ok(());
    }
    *reported_holder = Some(held.owner.clone());
    let annotation = if is_anomaly {
        " clock_anomaly=true（获取时刻在未来，按等待处理而不接管 —— ADR-0028 D6）"
    } else {
        ""
    };
    write_line(
        output,
        &format!(
            "-- guard-waiting: target={target} held_by={} held_task={} held_age_secs={}{}",
            held.owner,
            held.task,
            lock_age_secs(held, now_unix),
            annotation
        ),
    )
}

/// 拼出放弃时那行机器可读结果的字段部分（ADR-0028 D5 ②）。
fn abandonment_message(
    target: &str,
    held: &LockRecord,
    now_unix: u64,
    waited_millis: u64,
    is_anomaly: bool,
) -> String {
    format!(
        "target={target} held_by={} held_age_secs={} waited_ms={} clock_anomaly={}",
        held.owner,
        lock_age_secs(held, now_unix),
        waited_millis,
        is_anomaly
    )
}

#[cfg(test)]
// 测试体外置到 `guard_runner_tests.rs`（理由见该文件头）；`#[path]` 让它仍是本模块的私有单测。
#[path = "guard_runner_tests.rs"]
mod tests;
