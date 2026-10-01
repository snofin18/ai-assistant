//! `hygiene.rs` 的私有单元测试（TASK-085 外置）：仓库卫生规则。

use super::*;

/// 构造一条普通行注释，便于规则级单测。
fn line_comment(line: usize, text: &str) -> Comment {
    Comment {
        line,
        kind: CommentKind::Line,
        text: text.to_string(),
    }
}

/// 构造一条文档注释（`///`），用于验证文档注释被排除在代码块判定之外。
fn doc_comment(line: usize, text: &str) -> Comment {
    Comment {
        line,
        kind: CommentKind::LineDoc,
        text: text.to_string(),
    }
}

/// 取出发现项的规则标识符序列，便于一次性断言"命中了哪些规则、什么顺序"。
fn rules_of(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|finding| finding.rule).collect()
}

// --- 规则 1：文件行数 ---

#[test]
fn test_file_length_at_warn_threshold_yields_nothing() {
    assert!(
        check_file_length("a.rs", FILE_LINES_WARN).is_none(),
        "等于阈值不应告警"
    );
}

#[test]
fn test_file_length_over_warn_threshold_is_warning() {
    let finding = check_file_length("a.rs", FILE_LINES_WARN + 1).expect("应有发现项");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.rule, "hygiene/file-too-long");
}

#[test]
fn test_file_length_over_error_threshold_is_error() {
    let finding = check_file_length("a.rs", FILE_LINES_ERROR + 1).expect("应有发现项");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(
        finding.line,
        FILE_LINES_ERROR + 1,
        "文件级发现项把行数放在 line 字段，便于在 CI 输出里直接看到规模"
    );
}

// --- 规则 2：注释标签 ---

#[test]
fn test_banned_tag_is_error() {
    let findings = check_comment_tags("a.rs", &[line_comment(7, " 这里先 FIXME 一下")]);
    assert_eq!(rules_of(&findings), vec!["hygiene/banned-comment-tag"]);
    assert_eq!(findings.first().map(|finding| finding.line), Some(7));
}

#[test]
fn test_bare_todo_without_card_is_error() {
    let findings = check_comment_tags("a.rs", &[line_comment(3, " TODO 以后再说")]);
    assert_eq!(rules_of(&findings), vec!["hygiene/missing-card-reference"]);
}

#[test]
fn test_todo_with_task_card_passes() {
    let findings = check_comment_tags("a.rs", &[line_comment(3, " TODO(TASK-015): 补齐规则")]);
    assert!(findings.is_empty(), "带卡号的待办是合法的");
}

#[test]
fn test_todo_with_adr_reference_passes() {
    let findings = check_comment_tags("a.rs", &[line_comment(3, " TODO(ADR-0025): 等裁决")]);
    assert!(findings.is_empty());
}

#[test]
fn test_todo_with_placeholder_card_is_rejected() {
    let findings = check_comment_tags("a.rs", &[line_comment(3, " TODO(TASK-0NN): 占位")]);
    assert_eq!(
        rules_of(&findings),
        vec!["hygiene/missing-card-reference"],
        "占位卡号不指向真实卡片，不能放行"
    );
}

#[test]
fn test_stub_without_card_is_rejected() {
    let findings = check_comment_tags("a.rs", &[line_comment(9, " STUB 先返回空")]);
    assert_eq!(rules_of(&findings), vec!["hygiene/missing-card-reference"]);
}

#[test]
fn test_identifier_containing_tag_word_is_not_flagged() {
    let findings = check_comment_tags(
        "a.rs",
        &[line_comment(1, " 常量 TODOLIST 与 MY_HACKY 不是标签")],
    );
    assert!(
        findings.is_empty(),
        "必须做词边界判断，否则正常标识符会被误伤"
    );
}

#[test]
fn test_tag_inside_string_literal_is_not_flagged() {
    // 字符串内容会被 rustscan 抹平，因此这行源码不应产生任何发现项
    let source = "const BANNED: &str = \"FIXME\";\n";
    assert!(check_rust_source("a.rs", source).is_empty());
}

// --- 规则 3：被注释掉的代码 ---

#[test]
fn test_five_consecutive_code_comments_is_error() {
    let comments: Vec<Comment> = (1..=5)
        .map(|i| line_comment(i, &format!(" let x{i} = {i};")))
        .collect();
    let findings = check_commented_out_code("a.rs", &comments);
    assert_eq!(rules_of(&findings), vec!["hygiene/commented-out-code"]);
    assert_eq!(findings.first().map(|finding| finding.line), Some(1));
}

#[test]
fn test_four_consecutive_code_comments_passes() {
    let comments: Vec<Comment> = (1..=4)
        .map(|i| line_comment(i, &format!(" let x{i} = {i};")))
        .collect();
    assert!(
        check_commented_out_code("a.rs", &comments).is_empty(),
        "未达阈值不应失败"
    );
}

#[test]
fn test_prose_comment_block_passes() {
    let comments = vec![
        line_comment(1, " 这一段解释为什么策略引擎必须是唯一放行点："),
        line_comment(2, " 因为分散判断会让权限语义在多处漂移，"),
        line_comment(3, " 审计时也无法证明某次放行的依据。"),
        line_comment(4, " 所以 Host 与 UI 都不持有权限逻辑。"),
        line_comment(5, " 详见架构 v2 第 12 章。"),
    ];
    assert!(
        check_commented_out_code("a.rs", &comments).is_empty(),
        "散文注释不是代码"
    );
}

#[test]
fn test_doc_comments_are_excluded_from_code_run() {
    let comments: Vec<Comment> = (1..=6)
        .map(|i| doc_comment(i, &format!(" let x{i} = {i};")))
        .collect();
    assert!(
        check_commented_out_code("a.rs", &comments).is_empty(),
        "文档注释不参与判定"
    );
}

#[test]
fn test_blank_comment_line_does_not_break_run() {
    let comments = vec![
        line_comment(1, " let a = 1;"),
        line_comment(2, ""),
        line_comment(3, " let b = 2;"),
        line_comment(4, " let c = 3;"),
        line_comment(5, " let d = 4;"),
    ];
    let findings = check_commented_out_code("a.rs", &comments);
    assert_eq!(
        rules_of(&findings),
        vec!["hygiene/commented-out-code"],
        "空的 // 行是中性的"
    );
}

#[test]
fn test_all_blank_comment_run_passes() {
    let comments: Vec<Comment> = (1..=6).map(|i| line_comment(i, "")).collect();
    assert!(
        check_commented_out_code("a.rs", &comments).is_empty(),
        "全是空行不算代码块"
    );
}

#[test]
fn test_gap_in_line_numbers_starts_new_run() {
    let comments = vec![
        line_comment(1, " let a = 1;"),
        line_comment(2, " let b = 2;"),
        line_comment(3, " let c = 3;"),
        line_comment(10, " let d = 4;"),
        line_comment(11, " let e = 5;"),
    ];
    assert!(
        check_commented_out_code("a.rs", &comments).is_empty(),
        "不连续的注释分段计算"
    );
}

#[test]
fn test_two_separate_code_runs_both_reported() {
    let mut comments: Vec<Comment> = (1..=5)
        .map(|i| line_comment(i, &format!(" let a{i} = {i};")))
        .collect();
    comments.extend((20..=25).map(|i| line_comment(i, &format!(" let b{i} = {i};"))));
    let findings = check_commented_out_code("a.rs", &comments);
    assert_eq!(findings.len(), 2, "两段独立代码块应各报一次");
    assert_eq!(findings.first().map(|finding| finding.line), Some(1));
    assert_eq!(findings.get(1).map(|finding| finding.line), Some(20));
}

// --- 启发式辅助函数 ---

#[test]
fn test_looks_like_code_positive_cases() {
    for text in [
        " let x = 1;",
        " if flag {",
        " return Ok(());",
        "});",
        "pub fn f() {",
    ] {
        assert!(looks_like_code(text), "应判为代码：{text:?}");
    }
}

#[test]
fn test_looks_like_code_negative_cases() {
    for text in [
        "",
        "   ",
        " 这是一句中文说明",
        " 详见 gov 第 5.4 节",
        " 1) 先读文档",
    ] {
        assert!(!looks_like_code(text), "不应判为代码：{text:?}");
    }
}

#[test]
fn test_looks_like_code_ignores_double_colon_in_prose() {
    assert!(
        !looks_like_code(" 见 clippy::pedantic 这一组"),
        "散文里的 :: 不该触发"
    );
    assert!(!looks_like_code(" 相关：gov §5.4"), "中文引用不该触发");
}

#[test]
fn test_has_card_reference_boundaries() {
    assert!(has_card_reference("x TASK-001 y"));
    assert!(has_card_reference("ADR-0025"));
    assert!(!has_card_reference("TASK-12"), "少于 3 位数字不算卡号");
    assert!(!has_card_reference("ADR-012"), "少于 4 位数字不算 ADR 号");
    assert!(!has_card_reference("无引用"));
}

// --- 入口函数：确定性 ---

#[test]
fn test_check_rust_source_is_deterministic() {
    let source = "// TODO 没有卡号\n".repeat(3);
    let first = check_rust_source("a.rs", &source);
    let second = check_rust_source("a.rs", &source);
    assert_eq!(first, second, "不变量 1：纯函数");
    assert_eq!(first.len(), 3, "三行裸待办应各报一次");
}

#[test]
fn test_check_rust_source_on_real_module_header_has_no_findings() {
    // 反向自证：本文件自己的模块头（全是 //! 文档注释）不应被判成"注释掉的代码"
    let source = include_str!("hygiene.rs");
    let findings = check_rust_source("xtask/src/hygiene.rs", source);
    assert!(
        findings
            .iter()
            .all(|finding| finding.rule != "hygiene/commented-out-code"),
        "本文件不应触发注释代码规则，实际：{findings:?}"
    );
}

// --- TASK-085：函数与测试属性规则 ---

fn function_span(name: &str, start_line: usize, end_line: usize) -> FunctionSpan {
    FunctionSpan {
        name: name.to_string(),
        start_line,
        end_line,
        parameter_count: 0,
        cyclomatic_complexity: 1,
        is_test: false,
        ignore_attribute: None,
        body: "{}".to_string(),
    }
}

#[test]
fn test_function_length_over_threshold_is_warning() {
    let function = function_span("long_function", 1, FUNCTION_LINES_WARN + 2);
    let findings = check_function_length("a.rs", &[function]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "hygiene/function-too-long");
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_too_many_parameters_is_warning() {
    let mut function = function_span("many_parameters", 1, 2);
    function.parameter_count = PARAMETER_COUNT_WARN + 1;
    let findings = check_parameter_count("a.rs", &[function]);
    assert_eq!(findings[0].rule, "hygiene/too-many-parameters");
}

#[test]
fn test_high_cyclomatic_complexity_is_warning() {
    let mut function = function_span("branchy", 1, 20);
    function.cyclomatic_complexity = CYCLOMATIC_COMPLEXITY_WARN + 1;
    let findings = check_cyclomatic_complexity("a.rs", &[function]);
    assert_eq!(findings[0].rule, "hygiene/cyclomatic-complexity");
}

#[test]
fn test_empty_stub_without_marker_is_warning() {
    let function = function_span("empty_stub", 1, 2);
    let findings = check_bare_stubs("a.rs", &[], &[function]);
    assert_eq!(findings[0].rule, "hygiene/bare-stub");
}

#[test]
fn test_empty_stub_with_task_marker_passes() {
    let function = function_span("empty_stub", 1, 2);
    let comments = vec![line_comment(1, " STUB(TASK-085): temporary")];
    let findings = check_bare_stubs("a.rs", &comments, &[function]);
    assert!(findings.is_empty());
}

#[test]
fn test_skipped_test_without_reason_or_card_is_warning() {
    let mut function = function_span("skipped", 1, 2);
    function.is_test = true;
    function.ignore_attribute = Some("#[ignore]".to_string());
    let findings = check_skipped_tests("a.rs", "", &[function]);
    assert_eq!(findings[0].rule, "hygiene/skipped-test-without-reason");
}

#[test]
fn test_skipped_test_with_reason_and_card_passes() {
    let mut function = function_span("skipped", 1, 2);
    function.is_test = true;
    function.ignore_attribute = Some("#[ignore = \"TASK-085: windows-only\"]".to_string());
    let findings = check_skipped_tests("a.rs", "", &[function]);
    assert!(findings.is_empty());
}

// --- TASK-086：文件级换行规则 ---

#[test]
fn test_crlf_line_endings_are_error() {
    let findings = check_text_file_bytes("a.md", b"first\r\nsecond\n");
    assert_eq!(
        rules_of(&findings),
        vec!["hygiene/crlf-line-endings"],
        "CRLF 必须命中 Error 级规则"
    );
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Error)
    );
}

#[test]
fn test_lf_only_file_has_no_crlf_finding() {
    assert!(check_text_file_bytes("a.md", b"first\nsecond\n").is_empty());
}

#[test]
fn test_missing_final_newline_is_warning() {
    let findings = check_text_file_bytes("a.md", b"first");
    assert_eq!(rules_of(&findings), vec!["hygiene/missing-final-newline"]);
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Warning)
    );
}

#[test]
fn test_trailing_blank_line_is_warning() {
    let findings = check_text_file_bytes("a.md", b"first\n\n");
    assert_eq!(rules_of(&findings), vec!["hygiene/missing-final-newline"]);
}

#[test]
fn test_empty_text_file_is_warning() {
    let findings = check_text_file_bytes("a.md", b"");
    assert_eq!(rules_of(&findings), vec!["hygiene/missing-final-newline"]);
}
