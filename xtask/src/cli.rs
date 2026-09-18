//! # 命令行解析（xtask 的语法层）
//!
//! 职责：把 `argv` 解析成 `Invocation`，以及提供统一的用法说明文本。
//!
//! ## 边界（不做什么）
//! - 不做语义判断：例如"`--list-deferred` 能否单独使用"由 `main::execute` 决定。
//! - 不执行任何子命令、不碰文件系统。
//! - 不打印任何东西：解析失败只返回 `Err(消息)`，呈现由 `main` 负责。
//!
//! ## 不变量
//! 1. 解析是纯函数：同一 `argv` 必得同一 `Invocation`。
//! 2. 任何无法识别的输入都必须变成 `Err`，**不允许**被静默忽略
//!    （否则拼错命令名会得到一次"看起来成功"的空运行）。
//! 3. 一次调用最多一个子命令；出现第二个位置参数即报错。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.1

use std::path::PathBuf;

/// 用法说明（`--help` 与用法错误时打印）。
///
/// 子命令后面的 `[未实现 · TASK-NNN]` 标注必须与 `deferred.rs` 的登记表一致；
/// 两处不同步会误导读者，属于文档缺陷。
pub const USAGE: &str = r"xtask — 仓库护栏与开发任务工具（只读扫描）

用法：
  cargo run -p xtask -- <子命令> [选项]

子命令：
  hygiene            仓库卫生检查（gov §5.4；当前实现 3/13 项，见输出中的 deferred-rules 行）
  verify-schemas     [未实现 · TASK-011/015] Tool/Adapter/审计事件 schema 校验
  codegen            [未实现 · TASK-011]     由 schema 生成 Rust/TS 类型
  replay             [未实现 · TASK-034]     用录制的树快照做离线回放回归
  check-comments     [未实现 · 待补卡]       命名与注释规范检查（naming §10）
  check-ledger       [未实现 · 待补卡]       台账与记忆同步检查
  card-check         [未实现 · 待补卡]       任务卡格式完整性检查

选项：
  --list-deferred    打印未实现的子命令与未实现的卫生规则清单（含归属卡号）
  --repo <路径>      指定仓库根（默认由编译期的 CARGO_MANIFEST_DIR 推导）
  -h, --help, help   打印本说明

退出码：
  0 通过 | 1 有阻塞级发现项 | 2 用法错误 | 3 子命令未实现 | 4 IO/内部错误
";

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
        }
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
        match argument.as_str() {
            "-h" | "--help" | "help" => invocation.help = true,
            "--list-deferred" => invocation.list_deferred = true,
            "--repo" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| "--repo 需要一个路径参数".to_string())?;
                invocation.repo = Some(PathBuf::from(value));
            }
            other => {
                if let Some(value) = other.strip_prefix("--repo=") {
                    invocation.repo = Some(PathBuf::from(value));
                } else if other.starts_with('-') {
                    return Err(format!("未知选项 `{other}`"));
                } else if invocation.command.is_none() {
                    invocation.command = Some(other.to_string());
                } else {
                    return Err(format!("多余的参数 `{other}`（一次只能执行一个子命令）"));
                }
            }
        }
        index += 1;
    }
    Ok(invocation)
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
}
