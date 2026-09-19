//! # 命令行解析（xtask 的语法层）
//!
//! 职责：把 `argv` 解析成 `Invocation`，以及提供统一的用法说明文本。
//!
//! ## 边界（不做什么）
//! - 不做语义判断：例如"`--list-deferred` 能否单独使用"由 `main::execute` 决定，
//!   "`acquire` 必须有 `--owner`"由 `guard_runner` 决定。
//! - 不执行任何子命令、不碰文件系统。
//! - 不打印任何东西：解析失败只返回 `Err(消息)`，呈现由 `main` 负责。
//!
//! ## 不变量
//! 1. 解析是纯函数：同一 `argv` 必得同一 `Invocation`。
//! 2. 任何无法识别的输入都必须变成 `Err`，**不允许**被静默忽略
//!    （否则拼错命令名会得到一次"看起来成功"的空运行）。
//! 3. 一次调用最多一个子命令；出现第二个位置参数即报错 —— **除非**该子命令在
//!    [`COMMANDS_ACCEPTING_OPERANDS`] 里，此时后续位置参数是它的**操作数**
//!    （`guard acquire MEMORY.md LEDGER.md` 的两个路径不是"第二、第三个子命令"）。
//! 4. 带值选项与布尔开关的名字集中在两张常量表里；新增选项**必须**同时进表和 `USAGE`，
//!    否则会被当成"未知选项"报错（不变量 2）。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.1、`docs/adr/0028-file-rewrite-mutex-protocol.md`

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// 用法说明（`--help` 与用法错误时打印）。
///
/// 子命令后面的 `[未实现 · TASK-NNN]` 标注必须与 `deferred.rs` 的登记表一致；
/// 两处不同步会误导读者，属于文档缺陷。退出码说明必须覆盖**每一个**可能返回的码
/// （有测试盯着，见 `test_usage_text_documents_every_exit_code`）。
pub const USAGE: &str = r#"xtask — 仓库护栏与开发任务工具（只读扫描 + 文件锁）

用法：
  cargo run -p xtask -- <子命令> [选项]

子命令：
  hygiene            仓库卫生检查（gov §5.4；当前实现 3/13 项，见输出中的 deferred-rules 行）
  memory-counts      MEMORY.md 规模表 ↔ docs/memory/ 实测计数是否一致（ADR-0030 D1/D2）
  adr-index          ADR 编号登记表 ↔ docs/adr/*.md ↔ decisions.md 是否一致（ADR-0030 D3）
  refscan           ADR 编号一致性扩展（ADR-0032 + ADR-0026）：范围写法 / 裁引用 / .ps1 非 ASCII
  docscan           文档结构扫描（破表 / setext 风险 / 编码形状），不免
  card-check         任务卡格式完整性（ADR-0031 D6，**当前实现部分**）：9 节骨架齐全（Ready 豁免） + 状态=Done/Review + 记录区空 → Warning；**未实现**：状态行唯一 / 分界线唯一（计划归 TASK-060）
  guard <操作>       文件改写互斥锁（ADR-0028）；操作 = acquire | release | status | reap
  verify-schemas     [未实现 · TASK-011/015] Tool/Adapter/审计事件 schema 校验
  codegen            [未实现 · TASK-011]     由 schema 生成 Rust/TS 类型
  replay             [未实现 · TASK-034]     用录制的树快照做离线回放回归
  check-comments     [未实现 · 待补卡]       命名与注释规范检查（naming §10）
  check-ledger       [未实现 · 待补卡]       台账与记忆同步检查

guard 的选项（其它子命令不接受）：
  --owner <标识>     持有者；acquire/release **必填**，且必须会话级唯一
                     （例如 `codex-1a2b3c4d` 或 `TASK-030`）。不给默认值：没有 owner
                     就无法区分"自己已持锁"与"别人持锁"
  --task <卡号>      关联任务卡号（写进锁记录，供别人诊断）
  --intent <一句话>  这次改写想干什么（别人超时放弃时唯一的线索，强烈建议填）
  --timeout <秒>     等待超时，默认 30；超时即**放弃并通报**（退出码 5）
  --stale-after <秒> 陈旧阈值，默认 900；到期即可接管（接管会打印被接管者的完整锁记录）
  --force            人工强制接管 / 强制释放

通用选项：
  --list-deferred    打印未实现的子命令与未实现的卫生规则清单（含归属卡号）
  --repo <路径>      指定仓库根（默认由编译期的 CARGO_MANIFEST_DIR 推导）
  -h, --help, help   打印本说明

退出码：
  0 通过 | 1 有阻塞级发现项 | 2 用法错误 | 3 子命令未实现 | 4 IO/内部错误
  5 锁获取超时（放弃；见 ADR-0028 D5，放弃必须通报）
"#;

/// 允许携带位置参数（操作数）的子命令（不变量 3）。
///
/// 表外的子命令出现第二个位置参数仍然是用法错误 —— 这条行为有测试盯着
/// （`test_parse_args_second_command_is_usage_error`），**不得**为了图省事把这张表放开成"全部"。
const COMMANDS_ACCEPTING_OPERANDS: [&str; 1] = ["guard"];

/// 需要跟一个值的选项（不含前导 `--` 的名字会作为 `Invocation::options` 的键）。
const VALUE_OPTIONS: [&str; 5] = [
    "--owner",
    "--task",
    "--intent",
    "--timeout",
    "--stale-after",
];

/// 布尔开关（不含前导 `--` 的名字会进 `Invocation::flags`）。
const BOOLEAN_FLAGS: [&str; 1] = ["--force"];

/// 一次调用的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// 子命令名（缺省时为 `None`，由 `execute` 判定为用法错误）。
    pub command: Option<String>,
    /// `--repo` 指定的仓库根覆盖值。
    pub repo: Option<PathBuf>,
    /// 是否要求打印未实现清单。
    pub list_deferred: bool,
    /// 是否要求打印用法说明。
    pub help: bool,
    /// 子命令之后的位置参数（仅对 [`COMMANDS_ACCEPTING_OPERANDS`] 里的子命令合法）。
    pub operands: Vec<String>,
    /// 带值选项（键不含前导 `--`），例：`owner` → `codex-1a2b3c4d`。
    pub options: BTreeMap<String, String>,
    /// 布尔开关（元素不含前导 `--`），例：`force`。
    pub flags: BTreeSet<String>,
}

impl Invocation {
    /// 构造一个"什么都没要求"的调用（`--repo` 走默认推导时用得上）。
    #[must_use]
    pub const fn without_command() -> Self {
        Self {
            command: None,
            repo: None,
            list_deferred: false,
            help: false,
            operands: Vec::new(),
            options: BTreeMap::new(),
            flags: BTreeSet::new(),
        }
    }

    /// 某个布尔开关是否被传了。
    #[must_use]
    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
}

/// 解析命令行参数。
///
/// # 错误
/// 返回 `Err(消息)` 表示用法错误，消息面向人、包含出错的那个参数原文。
pub fn parse_args(arguments: &[String]) -> Result<Invocation, String> {
    let mut invocation = Invocation::without_command();
    let mut index = 0usize;
    while let Some(argument) = arguments.get(index) {
        let text = argument.as_str();
        match text {
            "-h" | "--help" | "help" => invocation.help = true,
            "--list-deferred" => invocation.list_deferred = true,
            // --repo 是**类型化**字段（main.rs 直接用它定位仓库根），故不走通用 options 表
            "--repo" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| "--repo 需要一个路径参数".to_string())?;
                invocation.repo = Some(PathBuf::from(value));
            }
            _ => {
                parse_other_argument(text, arguments, &mut index, &mut invocation)?;
            }
        }
        index += 1;
    }
    Ok(invocation)
}

/// 解析「不是 `-h`/`--help`/`--list-deferred`/`--repo`」的那个参数。
///
/// 判定顺序即优先级：内联带值（`--owner=x`）→ 分离带值（`--owner x`）→ 布尔开关 →
/// 未知选项 → 子命令 → 操作数 → 多余参数。
fn parse_other_argument(
    text: &str,
    arguments: &[String],
    index: &mut usize,
    invocation: &mut Invocation,
) -> Result<(), String> {
    if let Some(value) = text.strip_prefix("--repo=") {
        invocation.repo = Some(PathBuf::from(value));
        return Ok(());
    }
    if let Some((name, value)) = split_inline_value_option(text) {
        invocation.options.insert(name, value);
        return Ok(());
    }
    if VALUE_OPTIONS.contains(&text) {
        *index += 1;
        let value = arguments
            .get(*index)
            .ok_or_else(|| format!("{text} 需要一个值"))?;
        invocation
            .options
            .insert(trim_option_dashes(text), value.clone());
        return Ok(());
    }
    if let Some(flag) = strip_boolean_flag(text) {
        invocation.flags.insert(flag);
        return Ok(());
    }
    if text.starts_with('-') {
        return Err(format!("未知选项 `{text}`"));
    }
    if invocation.command.is_none() {
        invocation.command = Some(text.to_string());
        return Ok(());
    }
    if accepts_operands(invocation.command.as_deref()) {
        invocation.operands.push(text.to_string());
        return Ok(());
    }
    Err(format!("多余的参数 `{text}`（一次只能执行一个子命令）"))
}

/// 拆开 `--name=value` 形式；名字不在 [`VALUE_OPTIONS`] 里时返回 `None`。
fn split_inline_value_option(text: &str) -> Option<(String, String)> {
    let (name, value) = text.split_once('=')?;
    if !VALUE_OPTIONS.contains(&name) {
        return None;
    }
    Some((trim_option_dashes(name), value.to_string()))
}

/// `--force` → `force`；不是布尔开关时返回 `None`。
fn strip_boolean_flag(text: &str) -> Option<String> {
    BOOLEAN_FLAGS
        .contains(&text)
        .then(|| trim_option_dashes(text))
}

/// 去掉选项名的前导 `--`。
fn trim_option_dashes(name: &str) -> String {
    name.trim_start_matches('-').to_string()
}

/// 该子命令是否允许携带位置参数（不变量 3）。
fn accepts_operands(command: Option<&str>) -> bool {
    command.is_some_and(|name| COMMANDS_ACCEPTING_OPERANDS.contains(&name))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// 把参数切片构造成 `Vec<String>`，让用例读起来接近真实命令行。
    fn args(pieces: &[&str]) -> Vec<String> {
        pieces.iter().map(|piece| (*piece).to_string()).collect()
    }

    #[test]
    fn test_parse_args_plain_command() {
        let invocation = parse_args(&args(&["hygiene"])).expect("应解析成功");
        assert_eq!(invocation.command.as_deref(), Some("hygiene"));
        assert!(!invocation.help);
        assert!(!invocation.list_deferred);
        assert_eq!(invocation.repo, None);
    }

    #[test]
    fn test_parse_args_no_arguments_yields_empty_invocation() {
        let invocation = parse_args(&args(&[])).expect("空参数是合法的，由 execute 决定报错");
        assert_eq!(invocation, Invocation::without_command());
    }

    #[test]
    fn test_parse_args_help_variants() {
        for flag in ["-h", "--help", "help"] {
            let invocation = parse_args(&args(&[flag])).expect("应解析成功");
            assert!(invocation.help, "{flag} 应被识别为帮助");
        }
    }

    #[test]
    fn test_parse_args_list_deferred_flag() {
        let invocation = parse_args(&args(&["--list-deferred"])).expect("应解析成功");
        assert!(invocation.list_deferred);
        assert_eq!(invocation.command, None, "该选项可以单独使用");
    }

    #[test]
    fn test_parse_args_repo_with_separate_value() {
        let invocation = parse_args(&args(&["hygiene", "--repo", "D:/tmp/x"])).expect("应解析成功");
        assert_eq!(invocation.repo, Some(PathBuf::from("D:/tmp/x")));
    }

    #[test]
    fn test_parse_args_repo_with_equals_form() {
        let invocation = parse_args(&args(&["hygiene", "--repo=D:/tmp/x"])).expect("应解析成功");
        assert_eq!(invocation.repo, Some(PathBuf::from("D:/tmp/x")));
    }

    #[test]
    fn test_parse_args_repo_without_value_is_usage_error() {
        let error = parse_args(&args(&["hygiene", "--repo"])).expect_err("缺参数必须报错");
        assert!(
            error.contains("--repo"),
            "错误信息应指出是哪个选项：{error}"
        );
    }

    #[test]
    fn test_parse_args_unknown_option_is_usage_error() {
        let error = parse_args(&args(&["hygiene", "--shiny"])).expect_err("未知选项必须报错");
        assert!(error.contains("--shiny"), "错误信息应回显参数原文：{error}");
    }

    #[test]
    fn test_parse_args_second_command_is_usage_error() {
        let error = parse_args(&args(&["hygiene", "codegen"])).expect_err("一次只能一个子命令");
        assert!(
            error.contains("codegen"),
            "错误信息应指出多余的那个参数：{error}"
        );
    }

    #[test]
    fn test_parse_args_is_deterministic() {
        let input = args(&["hygiene", "--repo", "D:/x", "--list-deferred"]);
        assert_eq!(parse_args(&input), parse_args(&input), "不变量 1：纯函数");
    }

    #[test]
    fn test_usage_text_documents_every_exit_code() {
        // USAGE 是给人和 CI 看的契约；退出码说明缺失会让人无法解释非 0 退出
        for code in [
            "0 通过",
            "1 有阻塞级发现项",
            "2 用法错误",
            "3 子命令未实现",
            "4 IO/内部错误",
        ] {
            assert!(USAGE.contains(code), "用法说明缺少退出码说明：{code}");
        }
    }

    #[test]
    fn test_usage_text_marks_unimplemented_commands_with_owning_card() {
        for command in [
            "verify-schemas",
            "codegen",
            "replay",
            "check-comments",
            "check-ledger",
        ] {
            assert!(USAGE.contains(command), "用法说明遗漏子命令 {command}");
        }
        assert!(
            USAGE.contains("未实现"),
            "用法说明必须标明哪些子命令还没实现"
        );
    }

    // --- 新增（ADR-0028 / ADR-0030）：guard 操作数与选项、新子命令、退出码 5 ---

    #[test]
    fn test_usage_text_documents_lock_timeout_exit_code() {
        // ADR-0028 D8 新增退出码 5；USAGE 必须解释它，否则没人能读懂非 0 退出
        assert!(
            USAGE.contains("5 锁获取超时（放弃"),
            "用法说明缺少退出码 5 的说明"
        );
    }

    #[test]
    fn test_usage_text_documents_new_subcommands() {
        for command in ["guard", "memory-counts", "adr-index"] {
            assert!(USAGE.contains(command), "用法说明遗漏子命令 {command}");
        }
        for operation in ["acquire", "release", "status", "reap"] {
            assert!(
                USAGE.contains(operation),
                "用法说明遗漏 guard 操作 {operation}"
            );
        }
    }

    #[test]
    fn test_parse_args_guard_collects_operands_instead_of_erroring() {
        let invocation =
            parse_args(&args(&["guard", "acquire", "MEMORY.md", "LEDGER.md"])).expect("应解析成功");
        assert_eq!(invocation.command.as_deref(), Some("guard"));
        assert_eq!(
            invocation.operands,
            vec!["acquire", "MEMORY.md", "LEDGER.md"],
            "不变量 3：guard 之后的位置参数是操作数"
        );
    }

    #[test]
    fn test_parse_args_guard_value_options_in_both_forms() {
        let invocation = parse_args(&args(&[
            "guard",
            "acquire",
            "--owner",
            "codex-1a2b3c4d",
            "--task=TASK-030",
            "--intent",
            "追加 LEDGER 行",
            "--timeout",
            "5",
            "--stale-after=60",
            "--force",
            "MEMORY.md",
        ]))
        .expect("应解析成功");
        assert_eq!(
            invocation.options.get("owner").map(String::as_str),
            Some("codex-1a2b3c4d")
        );
        assert_eq!(
            invocation.options.get("task").map(String::as_str),
            Some("TASK-030")
        );
        assert_eq!(
            invocation.options.get("intent").map(String::as_str),
            Some("追加 LEDGER 行"),
            "含空格的值必须被当成一个整体"
        );
        assert_eq!(
            invocation.options.get("timeout").map(String::as_str),
            Some("5")
        );
        assert_eq!(
            invocation.options.get("stale-after").map(String::as_str),
            Some("60")
        );
        assert!(invocation.has_flag("force"));
        assert_eq!(invocation.operands, vec!["acquire", "MEMORY.md"]);
    }

    #[test]
    fn test_parse_args_value_option_without_value_is_usage_error() {
        let error = parse_args(&args(&["guard", "acquire", "--owner"])).expect_err("缺值必须报错");
        assert!(error.contains("--owner"), "应指出是哪个选项：{error}");
    }

    #[test]
    fn test_parse_args_unknown_inline_option_is_usage_error() {
        // 不变量 2：`--shiny=1` 不在 VALUE_OPTIONS 里，必须报"未知选项"而不是被当操作数
        let error = parse_args(&args(&["guard", "--shiny=1"])).expect_err("必须报错");
        assert!(error.contains("--shiny=1"), "实际：{error}");
    }

    #[test]
    fn test_parse_args_boolean_flag_with_value_is_usage_error() {
        let error = parse_args(&args(&["guard", "--force=yes"])).expect_err("必须报错");
        assert!(error.contains("--force=yes"), "实际：{error}");
    }

    #[test]
    fn test_parse_args_guard_options_are_not_available_to_other_commands_silently() {
        // `--owner` 是通用语法层认识的选项，但 hygiene 会忽略它 —— 语义校验在 execute，
        // 这里只验证语法层不会把它误当成操作数或子命令
        let invocation = parse_args(&args(&["hygiene", "--owner", "codex-x"])).expect("语法上合法");
        assert_eq!(invocation.command.as_deref(), Some("hygiene"));
        assert!(invocation.operands.is_empty());
        assert_eq!(
            invocation.options.get("owner").map(String::as_str),
            Some("codex-x")
        );
    }
}
