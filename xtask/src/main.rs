//! # xtask 入口：子命令分派与结果呈现
//!
//! 职责：把 `cli` 解析出的调用意图分派到对应子命令，调用规则模块，渲染报告，决定退出码。
//!
//! ## 模块分层（本 crate 内部）
//! | 模块 | 职责 | 是否碰文件系统 |
//! |---|---|---|
//! | `cli` | 解析 argv、用法文本 | 否 |
//! | `repowalk` | 定位仓库根、遍历源文件、相对路径归一化 | **是** |
//! | `rustscan` | 把源码拆成「注释」与「降噪代码」两个视图 | 否 |
//! | `hygiene` | gov §5.4 的卫生规则判定 | 否 |
//! | `deferred` | 未实现项登记表 | 否 |
//! | `report` | 发现项模型与渲染 | 否 |
//! | `memory_table` | `MEMORY.md` 规模表解析 + 行数/条目数判据（ADR-0030） | 否 |
//! | `memory_counts` | 记忆规模一致性的 8 条规则（ADR-0030 D1/D2） | 否 |
//! | `adr_registry` | ADR 登记表 / ADR 文件状态行 / 待建号解析（ADR-0030） | 否 |
//! | `adr_registry_tests` | `adr_registry` 解析原语的私有单测（`#[path]` 外置，见下「文件长度」说明） | 否 |
//! | `adr_index` | ADR 编号一致性的 11 条规则（ADR-0030 D3） | 否 |
//! | `adr_index_tests` | `adr_index` 各规则的私有单测（`#[path]` 外置，见下「文件长度」说明） | 否 |
//! | `doccheck` | `memory-counts` 与 `adr-index` 的 IO 边界 | **是**（只读） |
//! | `guard` | 锁记录编解码、slug 派生、`decide_acquire` 判定（ADR-0028） | 否 |
//! | `guard_tests` | `guard` 判定逻辑的私有单测（`#[path]` 外置，见下「文件长度」说明） | 否 |
//! | `guard_store` | `LockStore` trait + 文件系统实现 + ISO-8601 时钟 | **是**（`target/locks/`） |
//! | `guard_model` | guard 的请求/失败/三态读锁模型 | 否 |
//! | `guard_runner` | guard 分派 + `acquire`（等待/放弃/接管/回滚） | 经 `LockStore` |
//! | `guard_runner_tests` | `guard_runner` 各分支的私有单测（`#[path]` 外置，见下「文件长度」说明） | 否 |
//! | `guard_release` | guard 的 `release` / `status` / `reap` | 经 `LockStore` |
//! | `guard_testkit` | 内存锁存储替身（仅 `cfg(test)`） | 否 |
//! | `main`（本文件） | 分派、读文件内容、呈现、退出码 | 是（只读） |
//!
//! 规则判定一律是纯函数，IO 集中在 `repowalk` 与本文件 —— 这是白盒测试能覆盖每条规则
//! 而不需要造临时目录的前提。
//!
//! ## 文件长度与测试外置
//! gov §5.4 对单文件行数有硬上限，而白盒测试本身也很占行数。为了让「被测模块」保持可读，
//! 四个模块的私有单测用 `#[path]` 属性外置到同名 `*_tests.rs` 文件：
//! `adr_index` / `adr_registry` / `guard` / `guard_runner`。外置不改变可见性语义 —— 那些文件里的 `mod tests`
//! 仍是父模块的**私有子模块**，因此可以照常访问父模块的私有项，`use super::*;` 即可。
//! 新增规则时：判定逻辑写进被测模块，单测写进对应 `*_tests.rs`；只有当被测模块本身没有
//! 外置文件时，才把 `#[cfg(test)] mod tests` 直接写在模块尾部。
//!
//! ## 边界（不做什么）
//! - 不做规则判定：本文件不知道"多少行算太长"，那在 `hygiene.rs`。
//! - 只读**仓库内容**：不创建、修改或删除任何**受版本控制**的文件（写文件属于 codegen，
//!   那是 TASK-011 的事）。唯一例外是 `target/locks/` 下的临时锁文件与放弃日志（ADR-0028 D8），
//!   它们不入库、由 `guard` 自己管理生命周期。
//! - 不访问网络。
//!
//! ## 退出码约定（CI 与脚本依赖，改动需 ADR）
//! - `0` 检查通过（可能有 Warning）
//! - `1` 存在 Error 级发现项 → 阻塞合并
//! - `2` 用法错误（未知子命令 / 未知选项 / 多余参数 / 缺子命令）
//! - `3` 子命令已登记但尚未实现（见 `deferred.rs`）—— **故意不是 0**，铁律 1
//! - `4` IO 或内部错误（读不到文件、仓库根不存在等）
//! - `5` 锁获取超时（放弃）—— `guard acquire` 专用（ADR-0028 D5/D8）
//!
//! ## 不变量
//! 1. 输出确定性：同一仓库状态两次运行输出逐字节相同。
//! 2. 无静默失败：任何一步出错都会反映为非 0 退出码；唯一例外是 stderr 本身写不进去，
//!    此时退出码仍是非 0。
//! 3. 只读仓库内容：不创建、修改或删除任何**受版本控制**的文件；唯一例外见上面的「边界」
//!    （`target/locks/` 下的锁文件与放弃日志，ADR-0028 D8）。
//! 4. 「扫到 0 个文件」必须显式告警：否则空仓库会得到一个毫无意义的 PASSED。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.1/§5.4、`docs/spec/naming.md` §10

mod adr_index;
mod adr_registry;
mod arch;
mod card_check;
mod cli;
mod deferred;
mod doccheck;
mod docscan;
mod exemptions;
mod guard;
mod guard_model;
mod guard_release;
mod guard_runner;
mod guard_store;
mod hygiene;
mod memory_counts;
mod memory_table;
mod refscan;
mod replay;
mod report;
mod repowalk;
mod rustscan;

// guard 的测试替身：只在测试构建里存在，产品构建不会编进来
#[cfg(test)]
mod guard_testkit;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use cli::{Invocation, USAGE, parse_args};
use report::{Finding, Report, Severity};
use repowalk::{WalkError, collect_rust_files, relative_display_path, resolve_repo_root};

/// 检查通过（可能仍有 Warning）。
pub const EXIT_OK: u8 = 0;
/// 存在 Error 级发现项。
pub const EXIT_FINDINGS: u8 = 1;
/// 用法错误。
pub const EXIT_USAGE: u8 = 2;
/// 子命令尚未实现。
pub const EXIT_NOT_IMPLEMENTED: u8 = 3;
/// IO 或内部错误。
pub const EXIT_IO: u8 = 4;
/// 锁获取超时（放弃）—— `guard acquire` 专用（ADR-0028 D5/D8）。
///
/// 为什么单独一个码：「没拿到锁」既不是"检查发现了问题"（1），也不是"工具坏了"（4），
/// 而是**正常的协作结果**。调用方（脚本、夜间包装器、agent）需要能区分它，
/// 才能决定是重试、换目标、还是履行台账义务。
pub const EXIT_LOCK_TIMEOUT: u8 = 5;

/// 运行失败的原因（决定退出码与 `main` 的呈现方式）。
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// 命令行用法错误；`main` 会额外打印 `USAGE`。
    Usage(String),
    /// 子命令已登记但尚未实现；消息由 `deferred::not_implemented_message` 生成。
    NotImplemented(String),
    /// 文件系统错误（含仓库根不可用）。
    Io(String),
    /// 工具自身缺陷（例如报告摘要格式化失败）。
    Internal(String),
    /// `guard` 子命令的失败。
    ///
    /// 为什么不当场翻译成 `Usage`/`Io`：`GuardFailure` 自己就是退出码的唯一映射处
    /// （`guard_model.rs` 不变量 3）。在这里再写一遍 `match` 就有了两份真相，
    /// 将来加一个新失败类别必然只改一处，于是"锁记录损坏"会被静默归到别的退出码。
    /// 直接委托，编译期就保证两边一致。
    Guard(guard_model::GuardFailure),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // NotImplemented 的消息本身已是完整多行说明，不再加标签前缀
            Self::NotImplemented(message) => write!(formatter, "{message}"),
            Self::Usage(message) => write!(formatter, "用法错误：{message}"),
            Self::Io(message) => write!(formatter, "IO 错误：{message}"),
            Self::Internal(message) => write!(formatter, "内部错误：{message}"),
            // GuardFailure 的 Display 自带"guard …"前缀，这里不再叠加标签
            Self::Guard(failure) => write!(formatter, "{failure}"),
        }
    }
}

impl Failure {
    /// 该失败对应的退出码。
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::NotImplemented(_) => EXIT_NOT_IMPLEMENTED,
            Self::Io(_) | Self::Internal(_) => EXIT_IO,
            Self::Guard(failure) => failure.exit_code(),
        }
    }

    /// 把遍历错误包装成 `Failure::Io`。
    fn from_walk(error: &WalkError) -> Self {
        Self::Io(error.to_string())
    }

    /// 把 `std::io::Error` 连同上下文包装成 `Failure::Io`。
    ///
    /// 为什么带上下文：CI 里只看到 "Access is denied" 无法定位是哪个文件；
    /// 护栏工具的可诊断性本身就是护栏的一部分。
    fn from_io(context: &str, error: &std::io::Error) -> Self {
        Self::Io(format!("{context}：{error}"))
    }
}

/// 进程入口。
///
/// 输出通道刻意用 `stdout().lock()` / `stderr().lock()` 显式取得，
/// 而不是 `println!` / `eprintln!`：后者被 `clippy::print_stdout` / `print_stderr`
/// 设为 deny（产品代码必须走 tracing）。xtask 是**输出通道本身**，
/// 用显式 sink 既满足了护栏，也让 `execute` 能被注入 `Vec<u8>` 做白盒测试。
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut output = std::io::stdout().lock();
    let mut errors = std::io::stderr().lock();
    match execute(&arguments, &mut output) {
        Ok(code) => ExitCode::from(code),
        Err(failure) => present_failure(&failure, &mut errors),
    }
}

/// 把失败呈现给人，并给出退出码。
///
/// 这是全工具唯一「可能无处上报」的地方：如果 stderr 自己写不进去，就没有第二个通道了。
/// 此时退出码仍然是非 0，因此不构成静默成功（不变量 2）。
fn present_failure(failure: &Failure, errors: &mut dyn Write) -> ExitCode {
    writeln!(errors, "xtask: {failure}").ok();
    // guard 的用法错误同样要给出完整 USAGE：只说"--owner 必填"而不展示可选值，
    // 等于让人去翻源码（可诊断性是护栏的一部分）
    if matches!(
        failure,
        Failure::Usage(_) | Failure::Guard(guard_model::GuardFailure::Usage(_))
    ) {
        writeln!(errors, "{USAGE}").ok();
    }
    ExitCode::from(failure.exit_code())
}

/// 真正的执行逻辑（`main` 的薄封装），输出可注入。
///
/// 白盒测试用 `Vec<u8>` 当 sink，直接断言文本，不需要捕获子进程 stdout。
///
/// # 错误
/// - `Failure::Usage`：参数解析失败、缺子命令、未知子命令
/// - `Failure::NotImplemented`：子命令已登记但尚未实现
/// - `Failure::Io`：读写文件系统失败
/// - `Failure::Internal`：报告摘要格式化失败等工具自身缺陷
/// - `Failure::Guard`：`guard` 子命令失败（退出码委托给 `GuardFailure::exit_code`）
fn execute(arguments: &[String], output: &mut dyn Write) -> Result<u8, Failure> {
    let invocation = parse_args(arguments).map_err(Failure::Usage)?;
    if invocation.help {
        write_line(output, USAGE)?;
        return Ok(EXIT_OK);
    }
    if invocation.list_deferred {
        write_line(output, &deferred::describe_deferred_commands())?;
        write_line(output, &deferred::describe_deferred_rules())?;
        // --list-deferred 可以单独使用（只问"还缺什么"），也可以与子命令同用
        if invocation.command.is_none() {
            return Ok(EXIT_OK);
        }
    }
    let Some(command) = invocation.command.as_deref() else {
        return Err(Failure::Usage("缺少子命令".to_string()));
    };
    if command == "hygiene" {
        return run_hygiene(&invocation, output);
    }
    if command == "memory-counts" {
        return run_doc_consistency(&invocation, output, doccheck::run_memory_counts);
    }
    if command == "adr-index" {
        return run_doc_consistency(&invocation, output, doccheck::run_adr_index);
    }
    if command == "refscan" {
        return run_refscan(&invocation, output);
    }
    if command == "docscan" {
        return run_docscan(&invocation, output);
    }
    if command == "card-check" {
        return run_card_check(&invocation, output);
    }
    if command == "arch" {
        return run_arch(&invocation, output);
    }
    if command == "replay" {
        return run_replay(&invocation, output);
    }
    if command == "guard" {
        return run_guard(&invocation, output);
    }
    if let Some(entry) = deferred::find_command(command) {
        return Err(Failure::NotImplemented(deferred::not_implemented_message(
            entry,
        )));
    }
    Err(Failure::Usage(format!("未知子命令 `{command}`")))
}

/// 往 sink 写一行文本（统一的错误包装，避免每处都写一遍 `map_err`）。
fn write_line(output: &mut dyn Write, text: &str) -> Result<(), Failure> {
    writeln!(output, "{text}").map_err(|error| Failure::from_io("写 stdout", &error))
}

/// 执行文档一致性检查（`memory-counts` / `adr-index` 共用同一层壳）。
///
/// 两个子命令的形状完全相同（定位仓库根 → 调 IO 边界函数 → 呈现），故用一个函数指针参数化，
/// 避免复制三遍同样的样板。判定逻辑仍然在各自的纯函数模块里。
fn run_doc_consistency(
    invocation: &Invocation,
    output: &mut dyn Write,
    runner: fn(&Path, &mut dyn Write) -> Result<u8, String>,
) -> Result<u8, Failure> {
    // `from_walk` 收引用：`resolve_repo_root` 交出的是所有权，借一下再转换，避免为它多写一个 by-value 变体
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    runner(&root, output).map_err(Failure::Io)
}

/// 执行 `guard`：组装请求 → 造文件系统锁存储 → 交给 `guard_runner` 分派。
///
/// 操作数约定：`operands[0]` 是操作名（acquire/release/status/reap），其余是目标路径。
fn run_guard(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    // `from_walk` 收引用：`resolve_repo_root` 交出的是所有权，借一下再转换，避免为它多写一个 by-value 变体
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    let request = guard_model::build_request(
        invocation.operands.first().map(String::as_str),
        invocation.operands.get(1..).unwrap_or(&[]),
        &invocation.options,
        invocation.has_flag("force"),
    )
    .map_err(Failure::Guard)?;
    let store = guard_store::FileLockStore::new(root);
    guard_runner::run(&request, &store, output).map_err(Failure::Guard)
}

/// 执行架构护栏检查（分层 + 零三方依赖）。
fn run_arch(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    arch::run(&root, output).map_err(Failure::Io)
}

/// 执行树快照回放（replay skeleton = dry-run）。
fn run_replay(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let snapshot = invocation
        .operands
        .first()
        .ok_or_else(|| Failure::Usage("replay 需要快照路径".to_string()))?;
    let path = std::path::Path::new(snapshot);
    replay::run(path, output).map_err(Failure::Io)
}

/// 执行仓库卫生检查。
fn run_hygiene(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    let files = collect_rust_files(&root).map_err(|error| Failure::from_walk(&error))?;

    let mut report = Report::new("hygiene");
    report.scanned_files = files.len();

    let mut findings: Vec<Finding> = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file)
            .map_err(|error| Failure::from_io(&format!("读取 {}", file.display()), &error))?;
        let relative = relative_display_path(&root, file);
        findings.extend(hygiene::check_rust_source(&relative, &source));
    }
    // 排序保证输出确定性（report.rs 不变量 3 要求调用方排好序再插入）
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    report.extend(findings);

    // 不变量 4：0 个文件时 PASSED 是假信号，必须显式说出来
    if files.is_empty() {
        report.push(Finding::new(
            "xtask/no-source-files",
            Severity::Warning,
            "xtask",
            0,
            format!(
                "在 {} 下没有找到任何 .rs 文件（扫描根：{}）。PASSED 只代表没有代码可查，不代表代码合规。",
                root.display(),
                repowalk::SCANNED_SOURCE_ROOTS.join(", ")
            ),
        ));
    }

    // 机器可读的一行摘要放在最前，便于 CI/脚本 grep；随后是给人看的完整报告
    let summary = report
        .summary_line()
        .map_err(|error| Failure::Internal(format!("生成报告摘要失败：{error}")))?;
    write_line(output, &format!("-- machine-summary: {summary}"))?;
    // 主动声明覆盖范围，避免 PASSED 被误读成"全部 13 项都过了"
    write_line(output, &deferred::hygiene_progress_note())?;
    report
        .render(output)
        .map_err(|error| Failure::from_io("渲染报告", &error))?;

    Ok(if report.is_failure() {
        EXIT_FINDINGS
    } else {
        EXIT_OK
    })
}

#[allow(clippy::items_after_statements)]
fn run_refscan(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    refscan::run(&root, output).map_err(Failure::Io)
}

#[allow(clippy::items_after_statements)]
fn run_docscan(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    docscan::run(&root, output).map_err(Failure::Io)
}

#[allow(clippy::items_after_statements)]
fn run_card_check(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    card_check::run(&root, output).map_err(Failure::Io)
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// 把参数切片构造成 `Vec<String>`，让用例读起来接近真实命令行。
    fn args(pieces: &[&str]) -> Vec<String> {
        pieces.iter().map(|piece| (*piece).to_string()).collect()
    }

    /// 执行并断言"必定失败"，返回失败值供进一步断言。
    fn expect_failure(pieces: &[&str]) -> Failure {
        let mut output: Vec<u8> = Vec::new();
        match execute(&args(pieces), &mut output) {
            Ok(code) => panic!("期望失败，实际返回退出码 {code}"),
            Err(failure) => failure,
        }
    }

    // --- 分派行为（不触碰文件系统的分支）---

    #[test]
    fn test_execute_help_prints_usage_and_succeeds() {
        let mut output: Vec<u8> = Vec::new();
        let code = execute(&args(&["--help"]), &mut output).expect("不应失败");
        assert_eq!(code, EXIT_OK);
        let text = String::from_utf8(output).expect("应为 UTF-8");
        assert!(text.contains("退出码"), "帮助文本应包含退出码约定");
    }

    #[test]
    fn test_execute_without_command_is_usage_failure() {
        let failure = expect_failure(&[]);
        assert_eq!(failure, Failure::Usage("缺少子命令".to_string()));
        assert_eq!(failure.exit_code(), EXIT_USAGE);
    }

    #[test]
    fn test_execute_deferred_command_fails_with_distinct_code() {
        let failure = expect_failure(&["codegen"]);
        assert_eq!(
            failure.exit_code(),
            EXIT_NOT_IMPLEMENTED,
            "未实现必须区别于成功（铁律 1）"
        );
        assert!(
            failure.to_string().contains("TASK-011"),
            "必须指出归属卡号：{failure}"
        );
    }

    #[test]
    fn test_execute_every_deferred_command_fails_with_code_three() {
        for entry in deferred::DEFERRED_COMMANDS {
            let failure = expect_failure(&[entry.command]);
            assert_eq!(
                failure.exit_code(),
                EXIT_NOT_IMPLEMENTED,
                "子命令 {} 未实现却返回了别的退出码",
                entry.command
            );
        }
    }

    #[test]
    fn test_execute_unknown_command_is_usage_failure_not_not_implemented() {
        let failure = expect_failure(&["frobnicate"]);
        assert_eq!(
            failure.exit_code(),
            EXIT_USAGE,
            "拼错命令应是用法错误，而不是「未实现」"
        );
        assert!(failure.to_string().contains("frobnicate"));
    }

    #[test]
    fn test_execute_list_deferred_alone_succeeds() {
        let mut output: Vec<u8> = Vec::new();
        let code = execute(&args(&["--list-deferred"]), &mut output).expect("不应失败");
        assert_eq!(code, EXIT_OK);
        let text = String::from_utf8(output).expect("应为 UTF-8");
        assert!(text.contains("check-comments"), "清单应包含未实现子命令");
        assert!(text.contains("单函数行数"), "清单应包含未实现的卫生规则");
    }

    #[test]
    fn test_execute_hygiene_with_missing_repo_reports_io_failure() {
        let failure = expect_failure(&["hygiene", "--repo", "Z:/definitely-not-a-directory-xyz"]);
        assert_eq!(failure.exit_code(), EXIT_IO);
        assert!(
            failure.to_string().contains("仓库根不可用"),
            "实际：{failure}"
        );
    }

    // --- hygiene 端到端（对本仓库自身运行）---

    #[test]
    fn test_execute_hygiene_on_own_repo_produces_report() {
        let mut output: Vec<u8> = Vec::new();
        let code = execute(&args(&["hygiene"]), &mut output).expect("不应因 IO 失败");
        let text = String::from_utf8(output).expect("应为 UTF-8");
        assert!(text.contains("== xtask hygiene =="), "应有报告标题：{text}");
        assert!(text.contains("-- machine-summary:"), "应有机器可读摘要行");
        assert!(
            text.contains("-- deferred-rules:"),
            "应声明覆盖范围，避免 PASSED 被误读"
        );
        assert!(
            code == EXIT_OK || code == EXIT_FINDINGS,
            "hygiene 只应返回 0 或 1，实际 {code}"
        );
    }

    #[test]
    fn test_execute_hygiene_output_is_deterministic() {
        let mut first: Vec<u8> = Vec::new();
        let mut second: Vec<u8> = Vec::new();
        execute(&args(&["hygiene"]), &mut first).expect("第一次不应失败");
        execute(&args(&["hygiene"]), &mut second).expect("第二次不应失败");
        assert_eq!(first, second, "不变量 1：两次运行输出必须逐字节相同");
    }

    #[test]
    fn test_execute_hygiene_with_repo_override_scans_that_directory() {
        // 指向 xtask/src 之外的目录：应当扫到 0 个文件并显式告警（不变量 4）
        let root = resolve_repo_root(None).expect("默认仓库根应可用");
        let docs = root.join("docs");
        let mut output: Vec<u8> = Vec::new();
        let code = execute(
            &args(&["hygiene", "--repo", docs.to_str().expect("路径应为 UTF-8")]),
            &mut output,
        )
        .expect("不应因 IO 失败");
        let text = String::from_utf8(output).expect("应为 UTF-8");
        assert_eq!(code, EXIT_OK, "docs 下没有 .rs，不该有 Error 级发现项");
        assert!(
            text.contains("xtask/no-source-files"),
            "0 个文件必须显式告警：{text}"
        );
        assert!(text.contains("scanned_files=0"));
    }

    // --- present_failure ---

    #[test]
    fn test_present_failure_writes_message_and_usage_for_usage_errors() {
        let mut errors: Vec<u8> = Vec::new();
        let code = present_failure(&Failure::Usage("缺少子命令".to_string()), &mut errors);
        assert_eq!(code, ExitCode::from(EXIT_USAGE));
        let text = String::from_utf8(errors).expect("应为 UTF-8");
        assert!(text.contains("xtask: 用法错误：缺少子命令"));
        assert!(text.contains("退出码"), "用法错误应附带完整用法说明");
    }

    #[test]
    fn test_present_failure_does_not_duplicate_usage_for_other_failures() {
        let mut errors: Vec<u8> = Vec::new();
        let _ = present_failure(&Failure::Io("读不到文件".to_string()), &mut errors);
        let text = String::from_utf8(errors).expect("应为 UTF-8");
        assert!(text.contains("IO 错误"));
        assert!(!text.contains("退出码"), "非用法错误不应刷一屏帮助文本");
    }

    // --- Failure ---

    #[test]
    fn test_failure_display_includes_category_label() {
        assert_eq!(Failure::Usage("x".to_string()).to_string(), "用法错误：x");
        assert_eq!(Failure::Io("x".to_string()).to_string(), "IO 错误：x");
        assert_eq!(
            Failure::Internal("x".to_string()).to_string(),
            "内部错误：x"
        );
    }

    #[test]
    fn test_failure_display_keeps_not_implemented_message_verbatim() {
        let failure = Failure::NotImplemented("子命令 `replay` 尚未实现。".to_string());
        assert_eq!(
            failure.to_string(),
            "子命令 `replay` 尚未实现。",
            "该消息已是完整说明，不加前缀"
        );
        assert_eq!(failure.exit_code(), EXIT_NOT_IMPLEMENTED);
    }

    #[test]
    fn test_failure_guard_delegates_exit_code_instead_of_remapping() {
        // guard_model 不变量 3：退出码只有那一处映射。
        // 这条测试是"main 没有偷偷再判一遍"的证据 —— 把委托改回重复 match 就会红。
        let usage = Failure::Guard(guard_model::GuardFailure::Usage("--owner 必填".to_string()));
        assert_eq!(usage.exit_code(), EXIT_USAGE);
        assert_eq!(
            usage.to_string(),
            "guard 用法错误：--owner 必填",
            "GuardFailure 自带前缀，Failure 不再叠加标签"
        );

        let io = Failure::Guard(guard_model::GuardFailure::Io("锁目录不可写".to_string()));
        assert_eq!(io.exit_code(), EXIT_IO);

        let corrupt = Failure::Guard(guard_model::GuardFailure::Corrupt(
            "记录不可解析".to_string(),
        ));
        assert_eq!(
            corrupt.exit_code(),
            EXIT_IO,
            "损坏的锁是 IO 级故障，不是用法错误"
        );
    }

    #[test]
    fn test_present_failure_prints_usage_for_guard_usage_error() {
        // 只说"--owner 必填"而不展示全部选项，等于让人去翻源码
        let failure = Failure::Guard(guard_model::GuardFailure::Usage("缺少操作名".to_string()));
        let mut errors: Vec<u8> = Vec::new();
        let code = present_failure(&failure, &mut errors);
        assert_eq!(code, ExitCode::from(EXIT_USAGE));
        let text = String::from_utf8(errors).expect("应为 UTF-8");
        assert!(
            text.contains("退出码"),
            "guard 的用法错误也要附完整 USAGE：{text}"
        );
    }

    #[test]
    fn test_present_failure_does_not_print_usage_for_guard_io_error() {
        let failure = Failure::Guard(guard_model::GuardFailure::Io("锁目录不可写".to_string()));
        let mut errors: Vec<u8> = Vec::new();
        let _ = present_failure(&failure, &mut errors);
        let text = String::from_utf8(errors).expect("应为 UTF-8");
        assert!(
            !text.contains("退出码"),
            "IO 故障不该刷一屏帮助文本：{text}"
        );
    }

    #[test]
    fn test_failure_from_walk_keeps_context() {
        let failure = Failure::from_walk(&WalkError::RootUnderivable);
        assert!(
            failure.to_string().contains("CARGO_MANIFEST_DIR"),
            "实际：{failure}"
        );
    }
}
