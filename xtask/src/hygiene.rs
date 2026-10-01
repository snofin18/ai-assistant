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
//! - 本卡（TASK-001）只实现 gov §5.4 的 13 项中的 3 项；其余 10 项在 `deferred.rs`
//!   登记为「未实现 + 归属卡号」，归属 **TASK-085 / TASK-086**（另有 2 项未拆卡，见
//!   `docs/PARKING_LOT.md` PL-060；归属修正见 PL-059）。未实现的规则**不会被静默跳过**：
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
use crate::rustscan::{Comment, CommentKind, FunctionSpan, scan, scan_functions};

/// 单文件行数**警告**阈值（gov §5.4：> 600 行警告）。
pub const FILE_LINES_WARN: usize = 600;

/// 单文件行数**失败**阈值（gov §5.4：> 900 行失败，要求拆分）。
pub const FILE_LINES_ERROR: usize = 900;

/// 连续多少行 `//` 注释且形似代码时，判定为「被注释掉的代码块」（gov §5.4：> 5 行连续）。
pub const COMMENTED_CODE_RUN_ERROR: usize = 5;

/// 单函数行数**警告**阈值（gov §5.4：> 80 行警告）。
pub const FUNCTION_LINES_WARN: usize = 80;

/// 函数顶层参数个数**警告**阈值（gov §5.4：> 6 个警告）。
pub const PARAMETER_COUNT_WARN: usize = 6;

/// 圈复杂度**警告**阈值（gov §5.4：> 15 警告）。
pub const CYCLOMATIC_COMPLEXITY_WARN: usize = 15;

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
    let functions = scan_functions(source);
    let mut findings = Vec::new();

    // 规则 1（文件级）：行数上限
    if let Some(finding) = check_file_length(relative_path, scanned.line_count) {
        findings.push(finding);
    }
    // 规则 2：注释标签（禁用标签 / 缺卡号）
    findings.extend(check_comment_tags(relative_path, &scanned.comments));
    // 规则 3：被注释掉的代码块
    findings.extend(check_commented_out_code(relative_path, &scanned.comments));
    // 规则 4~8（TASK-085）：函数与测试属性结构
    findings.extend(check_function_rules(
        relative_path,
        source,
        &scanned.comments,
        &functions,
    ));

    findings
}

/// 返回源码中可执行函数的个数，供 `run_hygiene` 做「扫到 0 个函数」的显式告警。
#[must_use]
pub fn count_functions(source: &str) -> usize {
    scan_functions(source).len()
}

/// 应用 TASK-085 的五条函数结构规则。
#[must_use]
pub fn check_function_rules(
    relative_path: &str,
    source: &str,
    comments: &[Comment],
    functions: &[FunctionSpan],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(check_function_length(relative_path, functions));
    findings.extend(check_parameter_count(relative_path, functions));
    findings.extend(check_cyclomatic_complexity(relative_path, functions));
    findings.extend(check_bare_stubs(relative_path, comments, functions));
    findings.extend(check_skipped_tests(relative_path, source, functions));
    findings
}

/// 规则「单函数行数」：函数跨越行数 > 80 时警告。
#[must_use]
pub fn check_function_length(relative_path: &str, functions: &[FunctionSpan]) -> Vec<Finding> {
    functions
        .iter()
        .filter_map(|function| {
            let line_count = function.end_line.saturating_sub(function.start_line) + 1;
            if line_count <= FUNCTION_LINES_WARN {
                return None;
            }
            Some(Finding::new(
                "hygiene/function-too-long",
                Severity::Warning,
                relative_path,
                function.start_line,
                format!(
                    "函数 {} 跨 {} 行，超过建议上限 {}；考虑拆小或抽 helper（gov §5.4）",
                    function.name, line_count, FUNCTION_LINES_WARN
                ),
            ))
        })
        .collect()
}

/// 规则「函数参数个数」：顶层参数 > 6 时警告。
#[must_use]
pub fn check_parameter_count(relative_path: &str, functions: &[FunctionSpan]) -> Vec<Finding> {
    functions
        .iter()
        .filter_map(|function| {
            if function.parameter_count <= PARAMETER_COUNT_WARN {
                return None;
            }
            Some(Finding::new(
                "hygiene/too-many-parameters",
                Severity::Warning,
                relative_path,
                function.start_line,
                format!(
                    "函数 {} 有 {} 个参数，超过建议上限 {}；考虑结构体参数（gov §5.4）",
                    function.name, function.parameter_count, PARAMETER_COUNT_WARN
                ),
            ))
        })
        .collect()
}

/// 规则「圈复杂度」：分支估计 > 15 时警告。
#[must_use]
pub fn check_cyclomatic_complexity(
    relative_path: &str,
    functions: &[FunctionSpan],
) -> Vec<Finding> {
    functions
        .iter()
        .filter_map(|function| {
            if function.cyclomatic_complexity <= CYCLOMATIC_COMPLEXITY_WARN {
                return None;
            }
            Some(Finding::new(
                "hygiene/cyclomatic-complexity",
                Severity::Warning,
                relative_path,
                function.start_line,
                format!(
                    "函数 {} 估算圈复杂度 {}，超过建议上限 {}；考虑拆分分支（gov §5.4）",
                    function.name, function.cyclomatic_complexity, CYCLOMATIC_COMPLEXITY_WARN
                ),
            ))
        })
        .collect()
}

/// 规则「空实现 stub」：空体或直接 `Ok(())` 必须带 `STUB` + 卡号（TASK-085）。
#[must_use]
pub fn check_bare_stubs(
    relative_path: &str,
    comments: &[Comment],
    functions: &[FunctionSpan],
) -> Vec<Finding> {
    functions
        .iter()
        .filter(|function| is_bare_stub(&function.body))
        .filter(|function| !has_stub_marker(comments, function))
        .map(|function| {
            Finding::new(
                "hygiene/bare-stub",
                Severity::Warning,
                relative_path,
                function.start_line,
                format!(
                    "函数 {} 是空实现或直接返回 Ok(())，但没有 `STUB` + 卡号标记（gov §5.4 / naming §8）",
                    function.name
                ),
            )
        })
        .collect()
}

/// 规则「跳过测试」：`#[ignore]` / `.skip` 必须同时有非空原因与卡号（TASK-085）。
#[must_use]
pub fn check_skipped_tests(
    relative_path: &str,
    _source: &str,
    functions: &[FunctionSpan],
) -> Vec<Finding> {
    functions
        .iter()
        .filter(|function| function.is_test)
        .filter_map(|function| {
            let attribute = function.ignore_attribute.as_deref()?;
            let has_reason = has_nonempty_quoted_text(attribute);
            if has_reason && has_card_reference(attribute) {
                return None;
            }
            Some(Finding::new(
                "hygiene/skipped-test-without-reason",
                Severity::Warning,
                relative_path,
                function.start_line,
                format!(
                    "测试函数 {} 被跳过，但缺少非空原因或 TASK-NNN / ADR-NNNN 引用（gov §5.4）",
                    function.name
                ),
            ))
        })
        .collect()
}

/// 函数体是否为空实现或直接返回 `Ok(())`。
fn is_bare_stub(body: &str) -> bool {
    let normalized: String = body
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '{' | '}' | ';'))
        .collect();
    normalized.is_empty() || normalized == "Ok(())" || normalized == "returnOk(())"
}

/// 函数前后相邻范围内是否存在 `STUB` + 卡号注释（TASK-085）。
fn has_stub_marker(comments: &[Comment], function: &FunctionSpan) -> bool {
    let start = function.start_line.saturating_sub(3);
    comments.iter().any(|comment| {
        comment.line >= start
            && comment.line <= function.end_line
            && contains_token(&comment.text, "STUB")
            && has_card_reference(&comment.text)
    })
}

/// 属性中是否含至少一个非空双引号字符串。
fn has_nonempty_quoted_text(attribute: &str) -> bool {
    let mut in_quotes = false;
    let mut has_text = false;
    for character in attribute.chars() {
        if character == '"' {
            if in_quotes && has_text {
                return true;
            }
            in_quotes = !in_quotes;
            has_text = false;
        } else if in_quotes && !character.is_whitespace() {
            has_text = true;
        }
    }
    false
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
#[path = "hygiene_tests.rs"]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests;
