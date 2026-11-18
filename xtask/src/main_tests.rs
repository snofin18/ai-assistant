//! White-box tests for the xtask binary entry and command dispatch.

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
fn test_not_implemented_failure_keeps_distinct_code() {
    let entry = deferred::DeferredCommand {
        command: "future-command",
        ci_gate: "gov §5.1 #future",
        owning_card: "TASK-999",
        reason: "future",
    };
    let failure = Failure::NotImplemented(deferred::not_implemented_message(&entry));
    assert_eq!(
        failure.exit_code(),
        EXIT_NOT_IMPLEMENTED,
        "未实现必须区别于成功（铁律 1）"
    );
    let s = failure.to_string();
    assert!(
        s.contains("TASK-999"),
        "deferred failure must reference owning card or PL: {s}"
    );
}

#[test]
fn test_deferred_commands_are_empty_after_replay_completion() {
    assert!(
        deferred::DEFERRED_COMMANDS.is_empty(),
        "TASK-236 完成后 replay 未实现项必须从登记表移除"
    );
    assert!(deferred::find_command("replay-skeleton").is_none());
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
fn test_execute_adapter_validate_requires_one_operand() {
    let failure = expect_failure(&["adapter-validate", "--report-only"]);
    assert_eq!(failure.exit_code(), EXIT_USAGE);
    assert!(failure.to_string().contains("<adapter-dir>"));
}

#[test]
fn test_execute_adapter_validate_requires_report_only() {
    let failure = expect_failure(&["adapter-validate", "adapters/com.microsoft.notepad"]);
    assert_eq!(failure.exit_code(), EXIT_USAGE);
    assert!(failure.to_string().contains("--report-only"));
}

#[test]
fn test_execute_list_deferred_alone_succeeds() {
    let mut output: Vec<u8> = Vec::new();
    let code = execute(&args(&["--list-deferred"]), &mut output).expect("不应失败");
    assert_eq!(code, EXIT_OK);
    let text = String::from_utf8(output).expect("应为 UTF-8");
    assert!(
        text.contains("未实现的子命令：0 项"),
        "TASK-236 后 replay 未实现项必须清零：{text}"
    );
    assert!(
        !text.contains("replay-skeleton"),
        "replay 完整版不能再出现在未实现清单里：{text}"
    );
    assert!(
        text.contains("未实现 0 项"),
        "TASK-234 后卫生规则必须全部实现：{text}"
    );
    assert!(
        !text.contains("单函数行数"),
        "TASK-085 已实现的规则不应继续出现在未实现清单里"
    );
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
fn test_execute_hygiene_with_repo_override_without_whitelist_is_error() {
    // 用最小临时仓库隔离测试：提供派生计数事实源，但不提供 ADR-0069 白名单。
    let root =
        std::env::temp_dir().join(format!("xtask-hygiene-no-whitelist-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("清理旧临时仓库");
    }
    std::fs::create_dir_all(root.join("docs")).expect("创建 docs");
    std::fs::create_dir_all(root.join(".github/workflows")).expect("创建 workflow 目录");
    std::fs::write(
        root.join("docs/governance-ai-agent-execution.md"),
        concat!(
            "### 5.1 CI\n\n",
            "| # | 检查 |\n",
            "|---|---|\n",
            "| 1 | fmt |\n\n",
            "### 5.2 下一节\n\n",
            "### 5.4 卫生\n\n",
            "| 检查 | 阈值 |\n",
            "|---|---|\n",
            "| 规则 | warning |\n",
        ),
    )
    .expect("写治理事实源");
    std::fs::write(root.join(".github/workflows/ci.yml"), "# gov-gate: 1\n").expect("写 CI 事实源");
    let mut output: Vec<u8> = Vec::new();
    let code = execute(
        &args(&["hygiene", "--repo", root.to_str().expect("路径应为 UTF-8")]),
        &mut output,
    )
    .expect("不应因 IO 失败");
    std::fs::remove_dir_all(&root).expect("清理临时仓库");
    let text = String::from_utf8(output).expect("应为 UTF-8");
    assert_eq!(code, EXIT_FINDINGS, "缺少顶层目录白名单必须 fail-closed");
    assert!(
        text.contains("xtask/no-source-files"),
        "0 个文件必须显式告警：{text}"
    );
    assert!(
        text.contains("hygiene/top-level-whitelist-unparsable"),
        "缺少白名单必须显式失败：{text}"
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
