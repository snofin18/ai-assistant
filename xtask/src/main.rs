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
//! 单文件行数的写作规范与 CI 门禁阈值以 ADR-0033 为准，本注释不复制阈值。为了让「被测模块」保持可读，
//! 私有单测按既有约定用 `#[path]` 属性外置到同名 `*_tests.rs`；当前外置清单以
//! `xtask/src/**` 里的 `#[path = "*_tests.rs"]` 声明为唯一事实源，本注释不手抄数量或名单。
//! 外置不改变可见性语义 —— 那些文件里的 `mod tests` 仍是父模块的**私有子模块**，因此可以照常访问
//! 父模块的私有项，`use super::*;` 即可。
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

mod adapter_catalog;
mod adapter_catalog_check;
mod adapter_validate;
mod adr_index;
mod adr_registry;
mod arch;
mod automation_rounds;
mod automation_session;
mod card_check;
mod cli;
mod codegen;
mod comments;
mod deferred;
mod doccheck;
mod docscan;
mod duplicate_code;
mod exemptions;
mod guard;
mod guard_model;
mod guard_release;
mod guard_runner;
mod guard_store;
mod hygiene;
mod hygiene_io;
mod ledger_check;
mod memory_counts;
mod memory_table;
mod migration_registry;
mod refscan;
mod render;
mod replay;
mod replay_sequence;
mod report;
mod repowalk;
mod rustscan;
mod serde_json_lite;
mod top_level_dirs;
mod verify_schemas;
mod write_channel;

// guard 的测试替身：只在测试构建里存在，产品构建不会编进来
#[cfg(test)]
mod guard_testkit;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use cli::{Invocation, USAGE, parse_args};
use report::{Finding, Report, Severity};
use repowalk::{
    WalkError, collect_hygiene_text_files, collect_rust_files, relative_display_path,
    resolve_repo_root,
};

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
/// 而是**正常的协作结果**。调用方（脚本、自动化包装器、agent）需要能区分它，
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
    /// 治理事实源解析或集合一致性失败；属于阻塞级发现，退出码 1。
    GovernanceCounts(String),
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
            Self::GovernanceCounts(message) => write!(formatter, "治理计数错误：{message}"),
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
            Self::GovernanceCounts(_) => EXIT_FINDINGS,
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
        let derived_counts = load_governance_counts(&invocation)?;
        write_line(output, &deferred::describe_deferred_commands())?;
        let rules = deferred::describe_deferred_rules(derived_counts.hygiene_rules)
            .map_err(Failure::GovernanceCounts)?;
        write_line(output, &rules)?;
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
    if command == "check-ledger" {
        return run_check_ledger(&invocation, output);
    }
    if command == "automation-rounds" {
        return run_automation_rounds(&invocation, output);
    }
    if command == "automation-reset" {
        let root = resolve_repo_root(invocation.repo.as_deref())
            .map_err(|error| Failure::from_walk(&error))?;
        return automation_rounds::record_human_prompt(&root).map_err(Failure::Io);
    }
    if command == "check-migrations" {
        return run_check_migrations(&invocation, output);
    }
    if command == "check-comments" {
        return run_check_comments(&invocation, output);
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
    if command == "write" {
        return run_write(&invocation, output);
    }
    if command == "verify-schemas" {
        return run_verify_schemas(&invocation, output);
    }
    if command == "codegen" {
        return run_codegen(&invocation, output);
    }
    if command == "adapter-validate" {
        return run_adapter_validate(&invocation, output);
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

/// 从事实源派生治理计数；解析或集合不一致按阻塞级发现处理。
fn load_governance_counts(
    invocation: &Invocation,
) -> Result<deferred::DerivedGovernanceCounts, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    doccheck::derive_governance_counts(&root).map_err(Failure::GovernanceCounts)
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

/// 执行 `write`：把 stdin 内容经 guard 锁与独占占用探测写入一个仓库内相对路径。
///
/// 默认超时比 guard 更短（ADR-0066 D7）：写入通道存在的目的就是让 PowerShell
/// 会话尽快拿回控制权，而不是再次变成“看不见的无限等待”。
fn run_write(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    let store = guard_store::FileLockStore::new(root.clone());
    write_channel::run_from_invocation(
        invocation,
        &root,
        &store,
        &write_channel::FileSystemTargetAccess,
        output,
    )
    .map_err(Failure::Guard)
}

/// 执行架构护栏检查（分层 + 零三方依赖）。
fn run_arch(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    arch::run(&root, output).map_err(Failure::Io)
}

/// 执行 JSON schema 校验（protocol/*.json：存在性 + JSON 合法性 + version + 13 类 `ErrorCode`）。
fn run_verify_schemas(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    // 校验发现项用 Ok(1) 表达（阻塞级），必须原样透传，不能一律当成成功。
    match verify_schemas::run(&root, output) {
        Ok(exit_code) => Ok(exit_code),
        Err(error) => Err(Failure::Io(error)),
    }
}

/// 由 protocol/*.json 生成 Rust 类型；--check 只检测 drift，不写文件。
fn run_codegen(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    let check_only = invocation.has_flag("check");
    // 同上：`--check` 有 drift 时返回 1，必须透传给 CI。
    match codegen::run(&root, check_only, output) {
        Ok(exit_code) => Ok(exit_code),
        Err(error) => Err(Failure::Io(error)),
    }
}

/// Execute the read-only Adapter bundle validator.
fn run_adapter_validate(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    if invocation.operands.len() != 1 {
        return Err(Failure::Usage(
            "adapter-validate requires exactly one <adapter-dir>".to_string(),
        ));
    }
    if !invocation.has_flag("report-only") {
        return Err(Failure::Usage(
            "adapter-validate requires explicit --report-only".to_string(),
        ));
    }
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    adapter_validate::run(
        &root,
        invocation
            .operands
            .first()
            .map(String::as_str)
            .unwrap_or_default(),
        true,
        output,
    )
    .map_err(Failure::Io)
}

/// 执行树快照回放：`replay <fixture>` 保留旧路径，`replay --suite core` 跑序列与树 diff。
fn run_replay(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    if let Some(suite) = invocation.options.get("suite") {
        if !invocation.operands.is_empty() {
            return Err(Failure::Usage(
                "replay 不能同时指定 fixture 路径与 `--suite`".to_string(),
            ));
        }
        let root = resolve_repo_root(invocation.repo.as_deref())
            .map_err(|error| Failure::from_walk(&error))?;
        return replay::run_suite(&root, suite, output).map_err(Failure::Io);
    }
    let snapshot = invocation
        .operands
        .first()
        .ok_or_else(|| Failure::Usage("replay 需要快照路径".to_string()))?;
    let path = std::path::Path::new(snapshot);
    replay::run_path(path, output).map_err(Failure::Io)
}

/// 执行命名与注释规范检查（`docs/spec/naming.md` §10 的 8 条规则）。
///
/// 形状与 `run_hygiene` 一致：遍历 Rust 源文件 → 纯函数判定 → 排序 → 渲染。
/// 规则 ①② 复用 `hygiene` 的判据（见 `comments.rs` 模块头），其余 6 条在 `comments` 内实现。
fn run_check_comments(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    let files = collect_rust_files(&root).map_err(|error| Failure::from_walk(&error))?;

    let mut report = Report::new("check-comments");
    report.scanned_files = files.len();

    let mut findings: Vec<Finding> = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file)
            .map_err(|error| Failure::from_io(&format!("读取 {}", file.display()), &error))?;
        let relative = relative_display_path(&root, file);
        findings.extend(comments::check_rust_source(&relative, &source));
    }
    // 排序保证输出确定性（report.rs 不变量 3 要求调用方排好序再插入）
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    report.extend(findings);

    // 不变量：0 个文件时 PASSED 是假信号，必须显式说出来
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

    let summary = report
        .summary_line()
        .map_err(|error| Failure::Internal(format!("生成报告摘要失败：{error}")))?;
    write_line(output, &format!("-- machine-summary: {summary}"))?;
    // naming §10 的要求：未知规则不得静默忽略 —— 主动声明 8 条规则各自的实现位置。
    write_line(output, &comments::rule_coverage_note())?;
    report
        .render(output)
        .map_err(|error| Failure::from_io("渲染报告", &error))?;

    Ok(if report.is_failure() {
        EXIT_FINDINGS
    } else {
        EXIT_OK
    })
}

/// 执行仓库卫生检查。
fn run_hygiene(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    let derived_counts =
        doccheck::derive_governance_counts(&root).map_err(Failure::GovernanceCounts)?;
    let rust_files = collect_rust_files(&root).map_err(|error| Failure::from_walk(&error))?;
    let text_files =
        collect_hygiene_text_files(&root).map_err(|error| Failure::from_walk(&error))?;

    let mut report = Report::new("hygiene");
    report.scanned_files = rust_files.len();

    let rust_data = hygiene_io::collect_rust_hygiene_data(&root, &rust_files)?;
    let mut findings = rust_data.findings;
    let duplicate_sources: Vec<duplicate_code::DuplicateSource<'_>> = rust_data
        .sources
        .iter()
        .map(|(relative_path, source)| duplicate_code::DuplicateSource {
            relative_path,
            source,
        })
        .collect();
    findings.extend(duplicate_code::check_duplicate_code(&duplicate_sources));
    findings.extend(hygiene_io::collect_text_hygiene_findings(&text_files)?);
    findings.extend(hygiene_io::collect_dependency_hygiene_findings(
        &root,
        &text_files,
    )?);
    findings.extend(hygiene_io::collect_top_level_directory_hygiene_findings(
        &root,
    )?);
    // 排序保证输出确定性（report.rs 不变量 3 要求调用方排好序再插入）
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    report.extend(findings);

    hygiene_io::append_zero_scan_warnings(
        &mut report,
        &root,
        rust_files.len(),
        text_files.len(),
        rust_data.function_count,
    );

    // 机器可读的一行摘要放在最前，便于 CI/脚本 grep；随后是给人看的完整报告
    let summary = report
        .summary_line()
        .map_err(|error| Failure::Internal(format!("生成报告摘要失败：{error}")))?;
    write_line(output, &format!("-- machine-summary: {summary}"))?;
    // 主动声明覆盖范围，避免 PASSED 被误读成"全部规则都过了"
    let progress_note = deferred::hygiene_progress_note(derived_counts.hygiene_rules)
        .map_err(Failure::GovernanceCounts)?;
    write_line(output, &progress_note)?;
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

/// 执行台账新鲜度检查（ADR-0039 D3 的两条规则：`PLAN.md` 日期 + `README.md` 状态行）。
fn run_check_ledger(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    ledger_check::run(&root, output).map_err(Failure::Io)
}

/// 执行 `automation-rounds --run-date YYYY-MM-DD`。
fn run_automation_rounds(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let run_date = invocation.options.get("run-date").ok_or_else(|| {
        Failure::Usage("automation-rounds 缺少 `--run-date YYYY-MM-DD`".to_owned())
    })?;
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    automation_rounds::run(&root, run_date, output).map_err(Failure::Io)
}

/// 执行迁移登记表一致性检查（PL-047 的三条判据；SSOT = `docs/storage-design.md` §3.4）。
fn run_check_migrations(invocation: &Invocation, output: &mut dyn Write) -> Result<u8, Failure> {
    let root = resolve_repo_root(invocation.repo.as_deref())
        .map_err(|error| Failure::from_walk(&error))?;
    migration_registry::run(&root, output).map_err(Failure::Io)
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "main_tests.rs"]
mod tests;
