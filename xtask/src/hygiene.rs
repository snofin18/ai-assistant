//! # 仓库卫生检查规则（gov §5.4 的机器实现）
//!
//! 职责：对**单个 Rust 源文件的文本**应用卫生规则，产出 `Finding`。
//! 这些规则针对的是「AI 协作项目特有的退化信号」——不是功能缺陷，而是过程退化：
//! 文件越长，下一个 agent 越难完整读懂；没有卡号的待办等于永久债务；
//! 被注释掉的代码是"舍不得删"的典型症状。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO：输入是 `(相对路径, 源码文本)`，遍历与读写在 `main.rs`。
//! - 不做词法分析：注释识别委托给 `rustscan::scan`（否则字符串里的 `//` 会被误判为注释）。
//! - 本卡（TASK-001）只实现 gov §5.4 的 11 项中的 3 项；其余 8 项在 `deferred.rs`
//!   登记为「未实现 + 归属卡号」，由 TASK-015 补齐。未实现的规则**不会被静默跳过**：
//!   `xtask hygiene --list-deferred` 会把它们全部打印出来。
//!
//! ## 不变量
//! 1. 判定是**纯函数**：同样的 `(path, source)` 必得同样的 `Vec<Finding>`。
//! 2. 输出顺序确定：先文件级规则，再按注释出现顺序。
//! 3. 规则标识符（`Finding::rule`）是稳定契约，改动需 ADR —— CI 与任务卡都按它引用。
//! 4. 阈值只在本文件以 `pub const` 定义；调阈值等于改契约，需 ADR。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.4、`docs/spec/naming.md` §8

use crate::report::{Finding, Severity};
use crate::rustscan::{Comment, CommentKind, scan};

/// 单文件行数**警告**阈值（gov §5.4：> 600 行警告）。
pub const FILE_LINES_WARN: usize = 600;

/// 单文件行数**失败**阈值（gov §5.4：> 900 行失败，要求拆分）。
pub const FILE_LINES_ERROR: usize = 900;

/// 连续多少行 `//` 注释且形似代码时，判定为「被注释掉的代码块」（gov §5.4：> 5 行连续）。
pub const COMMENTED_CODE_RUN_ERROR: usize = 5;

/// `naming.md` §8 明令禁止的注释标签。
///
/// 为什么直接失败而不是警告：这三个标签的共同特征是**不携带任何可追溯信息**
/// （没有卡号、没有负责人），一旦进主干就永久留在代码里。
/// 项目要求「临时方案必须写成带卡号的待办」，正是为了让债务可被追踪。
const BANNED_COMMENT_TAGS: [&str; 3] = ["FIXME", "HACK", "XXX"];

/// 必须携带卡号引用的标签（`naming.md` §8）。
const CARD_REQUIRED_TAGS: [&str; 2] = ["TODO", "STUB"];

/// 形似代码的行首特征（启发式，只用于「被注释掉的代码」判定）。
///
/// 刻意**不包含** `::`、`->` 这类在中文技术散文里也会出现的符号，以降低误报。
const CODE_LINE_PREFIXES: [&str; 27] = [
    "let ", "let(", "if ", "if(", "for ", "while ", "fn ", "return", "use ", "pub ", "mod ",
    "impl ", "struct ", "enum ", "trait ", "type ", "match ", "else", "} else", "#[", "assert",
    "unsafe", "const ", "static ", "Some(", "Ok(", "Err(",
];

/// 形似代码的行内特征（启发式）。
const CODE_LINE_MARKERS: [&str; 5] = [" = ", "();", " == ", " != ", " => "];

/// 形似代码的行尾特征（启发式）。
const CODE_LINE_ENDINGS: [char; 5] = [';', '{', '}', ')', ','];

/// 对一个 Rust 源文件应用全部**已实现**的卫生规则。
///
/// `relative_path` 必须是相对仓库根、以 `/` 分隔的路径（由 `main.rs` 归一化），
/// 这样 CI 在 Windows 与 Linux 上输出一字不差。
#[must_use]
pub fn check_rust_source(relative_path: &str, source: &str) -> Vec<Finding> {
    let scanned = scan(source);
    let mut findings = Vec::new();

    // 规则 1（文件级）：行数上限
    if let Some(finding) = check_file_length(relative_path, scanned.line_count) {
        findings.push(finding);
    }
    // 规则 2：注释标签（禁用标签 / 缺卡号）
    findings.extend(check_comment_tags(relative_path, &scanned.comments));
    // 规则 3：被注释掉的代码块
    findings.extend(check_commented_out_code(relative_path, &scanned.comments));

    findings
}

/// 规则「单文件行数」：超过 `FILE_LINES_ERROR` 失败，超过 `FILE_LINES_WARN` 警告。
///
/// 为什么要拆文件：下一个会话的 agent 只能看到有限的上下文窗口。
/// 一个 2000 行的文件意味着 agent 每次都在「局部视图」下改代码 —— 这是漂移的温床。
#[must_use]
pub fn check_file_length(relative_path: &str, line_count: usize) -> Option<Finding> {
    if line_count > FILE_LINES_ERROR {
        Some(Finding::new(
            "hygiene/file-too-long",
            Severity::Error,
            relative_path,
            line_count,
            format!(
                "文件 {line_count} 行，超过硬上限 {FILE_LINES_ERROR} 行；必须按职责拆分（gov §5.4）"
            ),
        ))
    } else if line_count > FILE_LINES_WARN {
        Some(Finding::new(
            "hygiene/file-too-long",
            Severity::Warning,
            relative_path,
            line_count,
            format!(
                "文件 {line_count} 行，超过建议上限 {FILE_LINES_WARN} 行；考虑拆分（gov §5.4）"
            ),
        ))
    } else {
        None
    }
}

/// 规则「注释标签」：
/// - 命中 `BANNED_COMMENT_TAGS`（`naming.md` §8 禁用）→ **失败**，规则 `hygiene/banned-comment-tag`
/// - 命中 `CARD_REQUIRED_TAGS` 但同一条注释里没有 `TASK-NNN` / `ADR-NNNN` → **失败**，
///   规则 `hygiene/missing-card-reference`
///
/// 只扫描注释，不扫描代码：字符串字面量里的同名文本已被 `rustscan` 抹平，
/// 所以本文件把禁用标签写进 `const` 数组也不会触发自己的规则。
#[must_use]
pub fn check_comment_tags(relative_path: &str, comments: &[Comment]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for comment in comments {
        for tag in BANNED_COMMENT_TAGS {
            if contains_token(&comment.text, tag) {
                findings.push(Finding::new(
                    "hygiene/banned-comment-tag",
                    Severity::Error,
                    relative_path,
                    comment.line,
                    format!("使用了禁用标签 {tag}；改为带卡号的待办并写清内容（naming.md §8）"),
                ));
            }
        }
        if has_card_reference(&comment.text) {
            continue;
        }
        for tag in CARD_REQUIRED_TAGS {
            if contains_token(&comment.text, tag) {
                findings.push(Finding::new(
                    "hygiene/missing-card-reference",
                    Severity::Error,
                    relative_path,
                    comment.line,
                    format!(
                        "{tag} 缺少卡号引用；写成 `{tag}(TASK-NNN):` 或 `{tag}(ADR-NNNN):`（naming.md §8）"
                    ),
                ));
            }
        }
    }
    findings
}

/// 规则「被注释掉的代码块」：连续 ≥ `COMMENTED_CODE_RUN_ERROR` 行 `//` 注释，
/// 且其中每一行非空正文都「形似代码」→ **失败**。
///
/// 为什么失败而不是警告：被注释掉的代码 git 已经保存过，留在文件里只有两个后果 ——
/// ① 让读者以为它还有效；② 让下一个 agent 复制它。gov §6.1.2 明确「删除，用 git 找回」。
///
/// 只统计 `CommentKind::Line`：文档注释（`///`、`//!`）是说明书，不是「注释掉的代码」。
/// 空的 `//` 行视为**中性**：既不打断连续段，也不参与「形似代码」判定 ——
/// 真实被注释掉的代码块里常夹着空的 `//` 分隔行。
#[must_use]
pub fn check_commented_out_code(relative_path: &str, comments: &[Comment]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let line_comments: Vec<&Comment> = comments
        .iter()
        .filter(|comment| comment.kind == CommentKind::Line)
        .collect();

    let mut index = 0usize;
    while index < line_comments.len() {
        // 从 index 出发，沿「行号严格 +1」向后收集一段连续注释。
        // 用 get() 逐元素取而不是切片，避免下标越界 panic（clippy::indexing_slicing 为 deny）。
        let mut run: Vec<&Comment> = Vec::new();
        if let Some(first) = line_comments.get(index) {
            run.push(*first);
        }
        while let (Some(current), Some(next)) =
            (line_comments.get(index), line_comments.get(index + 1))
        {
            if current.line + 1 != next.line {
                break;
            }
            run.push(*next);
            index += 1;
        }
        if let Some(finding) = evaluate_comment_run(relative_path, &run) {
            findings.push(finding);
        }
        index += 1;
    }
    findings
}

/// 判定一段连续行注释是否构成「被注释掉的代码块」。
fn evaluate_comment_run(relative_path: &str, run: &[&Comment]) -> Option<Finding> {
    if run.len() < COMMENTED_CODE_RUN_ERROR {
        return None;
    }
    let non_blank: Vec<&Comment> = run
        .iter()
        .filter(|comment| !comment.text.trim().is_empty())
        .copied()
        .collect();
    if non_blank.is_empty()
        || !non_blank
            .iter()
            .all(|comment| looks_like_code(&comment.text))
    {
        return None;
    }
    let first_line = run.first().map_or(0, |comment| comment.line);
    Some(Finding::new(
        "hygiene/commented-out-code",
        Severity::Error,
        relative_path,
        first_line,
        format!(
            "第 {first_line} 行起有 {} 行连续注释形似被注释掉的代码；请删除，需要时用 git 找回（gov §6.1.2）",
            run.len()
        ),
    ))
}

/// 一行注释正文是否「形似代码」（启发式）。
///
/// 三条判据任一命中即为真：行尾是代码常见收尾符、行首是代码常见关键字、
/// 行内含赋值/调用等强特征。刻意偏保守：**漏报可以接受，误报更危险** ——
/// 误报会把人训练成「看到红就加 allow」，那会让整条护栏失效。
#[must_use]
pub fn looks_like_code(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.ends_with(CODE_LINE_ENDINGS) {
        return true;
    }
    if CODE_LINE_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return true;
    }
    CODE_LINE_MARKERS
        .iter()
        .any(|marker| trimmed.contains(marker))
}

/// 文本中是否出现 `TASK-<≥3 位数字>` 或 `ADR-<≥4 位数字>` 引用。
///
/// 不接受 `TASK-0NN` 这类占位写法：占位符不指向任何真实卡片，
/// 让占位符通过检查等于允许「看起来合规的债务」。
#[must_use]
pub fn has_card_reference(text: &str) -> bool {
    has_digits_after(text, "TASK-", 3) || has_digits_after(text, "ADR-", 4)
}

/// `text` 中是否存在 `prefix` 且其后紧跟至少 `min_digits` 位 ASCII 数字。
fn has_digits_after(text: &str, prefix: &str, min_digits: usize) -> bool {
    text.match_indices(prefix).any(|(offset, matched)| {
        let tail = text.get(offset + matched.len()..).unwrap_or_default();
        tail.chars()
            .take(min_digits)
            .filter(char::is_ascii_digit)
            .count()
            == min_digits
    })
}

/// `text` 中是否出现独立单词 `token`（前后都不是标识符字符）。
///
/// 做词边界判断是为了避免把 `TODOLIST`、`MY_HACKY_MACRO` 这类正常标识符误判为标签。
fn contains_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(offset, matched)| {
        let is_boundary_before = text
            .get(..offset)
            .and_then(|head| head.chars().next_back())
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let is_boundary_after = text
            .get(offset + matched.len()..)
            .and_then(|tail| tail.chars().next())
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        is_boundary_before && is_boundary_after
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
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
        let findings = check_comment_tags("a.rs", &[line_comment(3, " TODO(ADR-0007): 等裁决")]);
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
        assert!(has_card_reference("ADR-0012"));
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
}
