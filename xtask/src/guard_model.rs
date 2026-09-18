//! # guard 的请求模型与共享小工具（`xtask guard` 的类型层）
//!
//! 职责：定义 guard 的四个操作、一次调用的参数、失败原因、以及「读一把锁」的三态结果；
//! 再提供两个被 `guard_runner` / `guard_release` 共用的输出小工具。
//!
//! ## 为什么单独成文件
//! `acquire` 路径（等待/放弃/回滚）与 `release`/`status`/`reap` 路径都需要同一套类型。
//! 把类型放在任何一个执行文件里，另一个就得反向依赖它 —— 而模块间的依赖方向应该指向"更稳定的一侧"，
//! 类型比执行逻辑稳定得多。
//!
//! ## 边界（不做什么）
//! - 不执行任何操作：分派在 `guard_runner.rs::run`。
//! - 不做判定：`guard.rs::decide_acquire` 说了算。
//! - 不给必填参数兜默认值：缺 `--owner`、缺目标、操作名不认识，一律 [`GuardFailure::Usage`]
//!   （铁律 1：不给默认值冒充结果）。
//!
//! ## 不变量
//! 1. `build_request` 是纯函数：同样的参数必得同样的请求（目标已排序去重，见 `guard::sort_targets`）。
//! 2. 锁记录的三态（无 / 有 / 损坏）必须能被调用方**穷尽**匹配 —— 用 `Result<Option<_>>`
//!    表达三态一定会漏掉"损坏"这一种，而那正是最危险的一种（两个进程同时以为拿到了锁）。
//! 3. 退出码集中在此处的 [`GuardFailure::exit_code`]，不散落在各执行分支里。
//!
//! 相关：`docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/src/guard_runner.rs`

use std::collections::BTreeMap;
use std::io::Write;

use crate::guard::{LockRecord, parse_lock_record};
use crate::guard_store::LockStore;
use crate::{EXIT_IO, EXIT_USAGE};

/// guard 的四个操作（ADR-0028 D1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// 获取一把或多把锁（按路径排序，任一失败即回滚）。
    Acquire,
    /// 释放一把或多把锁（owner 必须匹配，除非 `--force`）。
    Release,
    /// 列出当前所有锁及其锁龄与是否陈旧（空目录打印 `NONE`）。
    Status,
    /// 清理所有陈旧锁（锁龄 ≥ `--stale-after`）。
    Reap,
}

impl Operation {
    /// 从操作名解析；不认识时返回 `None`（由 [`build_request`] 报用法错误）。
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "acquire" => Some(Self::Acquire),
            "release" => Some(Self::Release),
            "status" => Some(Self::Status),
            "reap" => Some(Self::Reap),
            _ => None,
        }
    }
}

/// 一次 guard 调用的全部已解析参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardRequest {
    /// 要执行的操作。
    pub operation: Operation,
    /// 目标路径（已归一化、去重、排序 —— 死锁避免，ADR-0028 D7）。
    pub targets: Vec<String>,
    /// 持有者标识（`acquire` / `release` 必填，且必须**会话级唯一**）。
    pub owner: String,
    /// 关联任务卡号（可空）。
    pub task: String,
    /// 这次改写想干什么（可空，但强烈建议填 —— 它是别人超时放弃时唯一的线索）。
    pub intent: String,
    /// 等待超时（秒）。
    pub timeout_secs: u64,
    /// 陈旧阈值（秒）。
    pub stale_after_secs: u64,
    /// 人工强制接管 / 强制释放。
    pub force: bool,
}

/// guard 执行失败的原因（决定退出码与呈现方式）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardFailure {
    /// 参数缺失或不合法 → 退出码 2。
    Usage(String),
    /// 文件系统或时钟错误 → 退出码 4。
    Io(String),
    /// 锁记录损坏且未给 `--force` → 退出码 4（不变量 2）。
    Corrupt(String),
}

impl GuardFailure {
    /// 对应的退出码（不变量 3：退出码只有这一处映射）。
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::Io(_) | Self::Corrupt(_) => EXIT_IO,
        }
    }
}

/// 把 `LockStore` 的字符串错误统一收敛成 [`GuardFailure::Io`]。
///
/// 为什么用 `From<String>` 而不是在每个调用点写 `.map_err(GuardFailure::Io)`：
/// `LockStore` 是 guard 唯一的 IO 边界（见 `guard_store.rs` 模块头的不变量 2），
/// 它的所有错误在语义上都是"文件系统/时钟出问题了"，没有第二种可能。
/// 有了这个 impl，`?` 就不会把 IO 故障悄悄降级成别的失败类别（铁律 1：无静默失败）。
/// **注意**：用法错误（`Usage`）与锁记录损坏（`Corrupt`）必须由调用方显式构造，
/// 绝不能指望字符串自动转换 —— 那两类不是 IO 故障，退出码也不同。
impl From<String> for GuardFailure {
    fn from(message: String) -> Self {
        Self::Io(message)
    }
}

impl std::fmt::Display for GuardFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "guard 用法错误：{message}"),
            Self::Io(message) => write!(formatter, "guard IO 错误：{message}"),
            Self::Corrupt(message) => write!(formatter, "guard 锁记录损坏：{message}"),
        }
    }
}

/// 读一把锁的三态结果（不变量 2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockRead {
    /// 没有锁文件。
    Absent,
    /// 有锁且记录合法。
    Present(LockRecord),
    /// 有锁文件但记录不可解析（半截写入 / 被人手改坏）。
    Corrupt(String),
}

/// 读并解析某个目标的锁。
///
/// # Errors
/// 仅在**读文件本身**失败时返回 [`GuardFailure::Io`]；记录损坏走 [`LockRead::Corrupt`]，
/// 因为那是"锁的状态"而不是"工具的故障"，调用方需要按 `--force` 分别处置。
pub fn read_lock(store: &dyn LockStore, target: &str) -> Result<LockRead, GuardFailure> {
    let Some(content) = store.read(target)? else {
        return Ok(LockRead::Absent);
    };
    Ok(match parse_lock_record(&content) {
        Ok(record) => LockRead::Present(record),
        Err(reason) => LockRead::Corrupt(format!(
            "{target} 的锁记录不可解析：{reason}。处置：人工确认没有进程正在写它之后，\
             用 `guard acquire --force` 接管，或 `guard reap` 清理"
        )),
    })
}

/// 组装一次 guard 调用的请求。
///
/// `operands` 是操作名之后的位置参数（`acquire`/`release` 的目标路径；`status`/`reap` 忽略它）。
/// `options` 的键不含前导 `--`。
///
/// # Errors
/// 缺操作名、操作名不认识、或数值选项不是非负整数时返回 [`GuardFailure::Usage`]。
pub fn build_request(
    operation_name: Option<&str>,
    operands: &[String],
    options: &BTreeMap<String, String>,
    force: bool,
) -> Result<GuardRequest, GuardFailure> {
    let Some(name) = operation_name else {
        return Err(GuardFailure::Usage(
            "缺少操作名（acquire / release / status / reap）".to_string(),
        ));
    };
    let Some(operation) = Operation::parse(name) else {
        return Err(GuardFailure::Usage(format!(
            "不认识的操作 `{name}`（可用：acquire / release / status / reap）"
        )));
    };
    let lookup_seconds = |key: &str, fallback: u64| -> Result<u64, GuardFailure> {
        // 用 let-else 而不是 match：「选项缺失」是正常路径（走默认值），「给了但非法」是 Usage
        // 错误 —— 两者处置完全不同，不该混在同一个 match 表达式里让人来回对照分支。
        let Some(text) = options.get(key) else {
            return Ok(fallback);
        };
        text.parse::<u64>().map_err(|error| {
            GuardFailure::Usage(format!(
                "`--{key}` 需要一个非负整数（秒），实际是 `{text}`：{error}"
            ))
        })
    };
    Ok(GuardRequest {
        operation,
        targets: crate::guard::sort_targets(operands),
        owner: options.get("owner").cloned().unwrap_or_default(),
        task: options.get("task").cloned().unwrap_or_default(),
        intent: options.get("intent").cloned().unwrap_or_default(),
        timeout_secs: lookup_seconds("timeout", crate::guard::DEFAULT_TIMEOUT_SECS)?,
        stale_after_secs: lookup_seconds("stale-after", crate::guard::DEFAULT_STALE_AFTER_SECS)?,
        force,
    })
}

/// 往 sink 写一行（统一的 IO 错误包装，避免每处都写一遍 `map_err`）。
///
/// 用 `pub` 而不是 `pub(crate)`：本模块是 `main.rs` 里的私有 `mod`，`pub` 的实际可见范围
/// 已经等于 crate 内（clippy `redundant_pub_crate`）。
///
/// # Errors
/// 写入 sink 失败时返回 [`GuardFailure::Io`]，原因文本原样带出。
pub fn write_line(output: &mut dyn Write, text: &str) -> Result<(), GuardFailure> {
    writeln!(output, "{text}").map_err(|error| GuardFailure::Io(error.to_string()))
}

/// 把多行文本整体缩进两格，让"被接管者的记录"在输出里是一个视觉块。
///
/// 用 `pub` 的理由同 [`write_line`]。
#[must_use]
pub fn indent_block(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::guard::{DEFAULT_STALE_AFTER_SECS, DEFAULT_TIMEOUT_SECS};

    /// 造一份选项表。
    fn options(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn test_operation_parse_accepts_four_names_only() {
        for name in ["acquire", "release", "status", "reap"] {
            assert!(Operation::parse(name).is_some(), "{name} 应被识别");
        }
        for name in ["", "Acquire", "acquir", "lock", "unlock"] {
            assert_eq!(Operation::parse(name), None, "{name} 不该被识别");
        }
    }

    #[test]
    fn test_build_request_fills_defaults_from_adr_0028() {
        let request = build_request(Some("status"), &[], &options(&[]), false).expect("应成功");
        assert_eq!(request.operation, Operation::Status);
        assert_eq!(
            request.timeout_secs, DEFAULT_TIMEOUT_SECS,
            "默认 30 秒（D5）"
        );
        assert_eq!(
            request.stale_after_secs, DEFAULT_STALE_AFTER_SECS,
            "默认 900 秒（D6）"
        );
        assert!(request.owner.is_empty(), "不给 owner 兜默认值（铁律 1）");
    }

    #[test]
    fn test_build_request_sorts_and_dedups_targets() {
        let request = build_request(
            Some("acquire"),
            &["B.md".to_string(), "A.md".to_string(), "B.md".to_string()],
            &options(&[]),
            false,
        )
        .expect("应成功");
        assert_eq!(request.targets, vec!["A.md", "B.md"], "D7：排序 + 去重");
    }

    #[test]
    fn test_build_request_missing_operation_is_usage_error() {
        let failure = build_request(None, &[], &options(&[]), false).expect_err("必须报错");
        assert_eq!(failure.exit_code(), EXIT_USAGE);
        assert!(
            failure.to_string().contains("缺少操作名"),
            "实际：{failure}"
        );
    }

    #[test]
    fn test_build_request_unknown_operation_is_usage_error_naming_it() {
        let failure =
            build_request(Some("frobnicate"), &[], &options(&[]), false).expect_err("必须报错");
        assert!(
            failure.to_string().contains("frobnicate"),
            "应回显操作名：{failure}"
        );
    }

    #[test]
    fn test_build_request_non_numeric_timeout_is_usage_error() {
        let failure = build_request(
            Some("acquire"),
            &[],
            &options(&[("timeout", "soon")]),
            false,
        )
        .expect_err("必须报错");
        assert!(failure.to_string().contains("--timeout"), "实际：{failure}");
    }

    #[test]
    fn test_build_request_reads_owner_task_intent_and_force() {
        let request = build_request(
            Some("acquire"),
            &["MEMORY.md".to_string()],
            &options(&[
                ("owner", "codex-1a2b3c4d"),
                ("task", "TASK-030"),
                ("intent", "追加 LEDGER 行"),
                ("stale-after", "60"),
            ]),
            true,
        )
        .expect("应成功");
        assert_eq!(request.owner, "codex-1a2b3c4d");
        assert_eq!(request.task, "TASK-030");
        assert_eq!(request.intent, "追加 LEDGER 行");
        assert_eq!(request.stale_after_secs, 60);
        assert!(request.force);
    }

    #[test]
    fn test_guard_failure_exit_codes_are_distinct_per_category() {
        assert_eq!(GuardFailure::Usage("x".into()).exit_code(), EXIT_USAGE);
        assert_eq!(GuardFailure::Io("x".into()).exit_code(), EXIT_IO);
        assert_eq!(GuardFailure::Corrupt("x".into()).exit_code(), EXIT_IO);
    }

    #[test]
    fn test_guard_failure_display_prefixes_category() {
        assert_eq!(
            GuardFailure::Io("读不到".into()).to_string(),
            "guard IO 错误：读不到"
        );
        assert_eq!(
            GuardFailure::Corrupt("坏了".into()).to_string(),
            "guard 锁记录损坏：坏了"
        );
    }

    #[test]
    fn test_read_lock_absent_when_store_has_nothing() {
        let store = crate::guard_testkit::FakeLockStore::at(1000);
        assert_eq!(
            read_lock(&store, "MEMORY.md").expect("读本身应成功"),
            LockRead::Absent
        );
    }

    #[test]
    fn test_read_lock_present_when_record_is_valid() {
        let store = crate::guard_testkit::FakeLockStore::at(1000);
        store.place_lock("MEMORY.md", "codex-b", 900);
        let read = read_lock(&store, "MEMORY.md").expect("读本身应成功");
        match read {
            LockRead::Present(record) => assert_eq!(record.owner, "codex-b"),
            other => panic!("应为 Present，实际 {other:?}"),
        }
    }

    #[test]
    fn test_read_lock_corrupt_is_distinguished_from_absent() {
        // 不变量 2：这是最危险的一种情况，必须能被穷尽匹配到
        let store = crate::guard_testkit::FakeLockStore::at(1000);
        store.place_corrupt_lock("MEMORY.md");
        match read_lock(&store, "MEMORY.md").expect("读本身应成功") {
            LockRead::Corrupt(reason) => {
                assert!(reason.contains("--force"), "应给出处置：{reason}");
            }
            other => panic!("应为 Corrupt，实际 {other:?}"),
        }
    }

    #[test]
    fn test_read_lock_io_error_is_not_swallowed_into_absent() {
        let store = crate::guard_testkit::FakeLockStore::at(1000);
        store.fail_next_read_once();
        let failure = read_lock(&store, "MEMORY.md").expect_err("IO 错误必须上抛");
        assert_eq!(failure.exit_code(), EXIT_IO);
    }

    #[test]
    fn test_indent_block_indents_every_line() {
        assert_eq!(indent_block("a\nb\n"), "  a\n  b");
    }

    #[test]
    fn test_write_line_appends_newline() {
        let mut sink: Vec<u8> = Vec::new();
        write_line(&mut sink, "hello").expect("写内存不会失败");
        assert_eq!(String::from_utf8(sink).expect("应为 UTF-8"), "hello\n");
    }
}
