//! # 注释与命名规范检查（`docs/spec/naming.md` §10 的机器实现）
//!
//! 职责：对**单个 Rust 源文件的文本**应用 naming §10 的 8 条规则，产出 `Finding`。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO：输入是 `(相对路径, 源码文本)`，遍历与读写在 `main.rs`。
//! - 不做词法分析：注释/字面量识别委托给 `rustscan::scan`。
//! - 不判「该不该有这条规则」：规则集来自 naming §10，改动需 ADR。
//!
//! ## 与 `hygiene` 的分工（规则 ①②）
//! naming §10 的规则 ①（禁用标签 / 无卡号的待办标签）与 ②（被注释掉的代码块）**已经**由
//! `hygiene` 实现（gov §5.4 的同名规则，规则 id 为 `hygiene/banned-comment-tag`、
//! `hygiene/missing-card-reference`、`hygiene/commented-out-code`）。本模块**复用**那两个
//! 判定函数并原样转发它们的 `Finding`，而不是抄一份第二套判据 —— 两套判据迟早会漂移，
//! 而「同一份规则在两条门禁上给出不同答案」比缺一条规则更难排查。
//!
//! ## 不变量
//! 1. 判定是**纯函数**：同样的 `(path, source)` 必得同样的 `Vec<Finding>`。
//! 2. 输出顺序确定：按规则编号顺序，再按行号。
//! 3. 规则标识符（`Finding::rule`）是稳定契约，改动需 ADR。
//! 4. 规则覆盖必须可自证：`TOTAL_COMMENT_RULE_COUNT` 与各规则函数一一对应，
//!    且 `rule_coverage()` 会打印每条规则的实现位置 —— 不存在"静默跳过的规则"。
//!
//! 相关：`docs/spec/naming.md` §3/§4/§8/§10、`docs/governance-ai-agent-execution.md` §5.4

use crate::hygiene;
use crate::report::{Finding, Severity};
use crate::rustscan::{Comment, CommentKind, scan};

/// naming §10 的规则总数。
pub const TOTAL_COMMENT_RULE_COUNT: usize = 8;

/// 规则 ④：模块头缺失在下列 crate 内是 **Error**（naming §10 原文）。
const MODULE_HEADER_STRICT_PREFIXES: [&str; 5] = [
    "crates/core/",
    "crates/policy/",
    "crates/task-engine/",
    "crates/verify/",
    "crates/undo/",
];

/// 规则 ⑦ 的禁用同义词（naming §4 受控词汇表的右列，**只取与现有受控词不冲突的那些**）。
///
/// 为什么不用整张右列：右列里的 `Transport` / `Checkpoint` / `Permission` / `Phase` /
/// `Runner` 等词在本仓库里是**受控词本身的组成部分**（例如 `StepPhase`、
/// `CheckpointStore`），把它们当违规会制造大量误报，而误报会训练人忽略门禁。
/// 这里只保留「明确是某个受控词的禁用同义词、且不与任何受控词重叠」的词。
const BANNED_SYNONYM_STEMS: [&str; 14] = [
    "element",
    "object",
    "widget",
    "driver",
    "connector",
    "plugin",
    "extension",
    "endpoint",
    "deviation",
    "detour",
    "dirty",
    "flagged",
    "undoability",
    "locator",
];

/// 规则 ⑧ 的缩写白名单（naming §3 的 23 项）。
const ABBREVIATION_ALLOWLIST: [&str; 23] = [
    "id", "url", "uri", "ui", "os", "db", "ipc", "mcp", "uia", "ax", "atspi", "cdp", "dpi", "ocr",
    "ttl", "http", "json", "sql", "fs", "vm", "px", "ms", "us",
];

/// 规则 ⑧ 的**显式**缩写黑名单：含元音因而躲过「无元音」启发式的高频缩写。
///
/// 为什么不用「长度 ≤4 且不在白名单」的原始启发式：那样会命中 `read` / `list` / `name` /
/// `path` 这类完全正常的英文词，为了压住误报只能不断往豁免表里加词 —— 一张靠"看着顺眼"
/// 增长的豁免表本身就是不可审计的规则。改用两条**可判定的**判据：
/// ① 2~4 字母且**不含元音**（`tgt` / `cfg` / `ptr` / `wnd` / `str`）；
/// ② 出现在本黑名单里（`buf` / `val` / `idx` 这类含元音的常见缩写）。
/// 新增黑名单词需要理由（"它在别处是缩写"），而不是"它看起来短"。
const BANNED_ABBREVIATIONS: [&str; 21] = [
    "buf", "val", "idx", "num", "obj", "req", "res", "rsp", "tmp", "elem", "sel", "attr", "var",
    "cmd", "opt", "proc", "sess", "func", "misc", "impl2", "info",
];

/// 规则 ⑧ 的元音字母（`y` 不计入：`style` / `key` 里的 `y` 不改变判定，而 `sync` 需要被命中）。
const VOWELS: [char; 5] = ['a', 'e', 'i', 'o', 'u'];

/// 对一个 Rust 源文件应用 naming §10 的全部 8 条规则。
#[must_use]
pub fn check_rust_source(relative_path: &str, source: &str) -> Vec<Finding> {
    let scanned = scan(source);
    let mut findings = Vec::new();

    // ① 禁用标签 / 无卡号的待办标签 —— 复用 hygiene 的判据（见模块头「与 hygiene 的分工」）
    findings.extend(hygiene::check_comment_tags(
        relative_path,
        &scanned.comments,
    ));
    // ② 被注释掉的代码块 —— 同上
    findings.extend(hygiene::check_commented_out_code(
        relative_path,
        &scanned.comments,
    ));
    // ③ 公共 API 缺文档注释
    if is_public_library_surface(relative_path, source) {
        findings.extend(check_public_api_docs(relative_path, &scanned));
    }
    // ④ 模块头缺 `//!`
    if !is_generated(source) {
        findings.extend(check_module_header(relative_path, source, &scanned));
    }
    // ⑤ unsafe 缺 `// SAFETY:`
    findings.extend(check_unsafe_safety(relative_path, &scanned));
    // ⑥ PITFALL 标签格式
    findings.extend(check_pitfall_format(relative_path, &scanned.comments));
    // ⑦⑧ 命名类规则只针对**对外命名面**（库 crate 的 src，非生成物）：
    // 测试辅助函数与生成代码不是"给人读的 API 面"，对它们报命名违规只会制造噪声。
    if is_public_library_surface(relative_path, source) {
        findings.extend(check_controlled_vocabulary(relative_path, &scanned));
        findings.extend(check_abbreviations(relative_path, &scanned));
    }

    findings
}

/// 该文件是否属于「库 crate 的对外命名面」。
///
/// 判据：路径形如 `crates/<name>/src/**`（排除 `tests/` 与 `apps/`），且不是生成物。
/// 为什么这样划：naming §10 的 ③⑦⑧ 关心的是**公共 API 与对外命名**；
/// 测试辅助模块、二进制 crate（`xtask`）与 `protocol` 生成代码都没有"外部读者"，
/// 对它们套用同一判据会把 46 条文档缺失里的 28 条集中在 `tests/common/mod.rs`，
/// 让真正的 API 缺口淹没在噪声里。
fn is_public_library_surface(relative_path: &str, source: &str) -> bool {
    relative_path.starts_with("crates/")
        && relative_path.contains("/src/")
        && !relative_path.contains("/tests/")
        && !is_generated(source)
}

/// 生成物判定：头部声明「GENERATED — DO NOT EDIT」。
fn is_generated(source: &str) -> bool {
    source
        .lines()
        .take(3)
        .any(|line| line.contains("GENERATED") && line.contains("DO NOT EDIT"))
}

/// 返回每条规则的编号、规则 id 与实现位置，供 `--list-rules` 打印。
///
/// 这条自证是 naming §10 的要求：**未知规则不得静默忽略**。任何一条规则只要没有
/// 对应的实现，就必须在这里显式写出来，而不是让读者以为 8 条都在跑。
#[must_use]
pub fn rule_coverage() -> Vec<(u8, &'static str, &'static str)> {
    vec![
        (
            1,
            "hygiene/banned-comment-tag",
            "hygiene::check_comment_tags",
        ),
        (
            1,
            "hygiene/missing-card-reference",
            "hygiene::check_comment_tags",
        ),
        (
            2,
            "hygiene/commented-out-code",
            "hygiene::check_commented_out_code",
        ),
        (
            3,
            "comments/public-api-missing-doc",
            "comments::check_public_api_docs",
        ),
        (
            4,
            "comments/module-header-missing",
            "comments::check_module_header",
        ),
        (
            5,
            "comments/unsafe-without-safety",
            "comments::check_unsafe_safety",
        ),
        (
            6,
            "comments/pitfall-format",
            "comments::check_pitfall_format",
        ),
        (
            7,
            "comments/controlled-vocabulary",
            "comments::check_controlled_vocabulary",
        ),
        (8, "comments/abbreviation", "comments::check_abbreviations"),
    ]
}

/// 供 `check-comments` 在报告前打印的一行覆盖说明。
///
/// 为什么必须打印：naming §10 的要求是「未知规则不得静默忽略」。一条规则只要没实现，
/// 就必须在**每次运行的输出里**看得见，而不是藏在文档里等人去比对。
#[must_use]
pub fn rule_coverage_note() -> String {
    let entries = rule_coverage()
        .iter()
        .map(|(number, rule, location)| format!("{number}:{rule}@{location}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "-- rule-coverage: naming §10 共 {TOTAL_COMMENT_RULE_COUNT} 条，全部已实现（{entries}）"
    )
}

/// 一条被识别出来的声明。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    line: usize,
    keyword: &'static str,
    name: String,
}

/// 规则 ③：`pub fn` / `pub struct` / `pub enum` / `pub trait` 缺 `///` 文档注释。
///
/// 判定方式：从声明的上一行向上走，**只**穿过文档注释行与 `#[...]` 属性行；
/// 一旦遇到普通代码行或真正的空行就停止。这样"上一项有文档、这一项没有"不会被误判为合规。
#[must_use]
pub fn check_public_api_docs(relative_path: &str, scanned: &crate::rustscan::Scan) -> Vec<Finding> {
    let doc_lines = doc_comment_lines(&scanned.comments);
    let code_lines: Vec<&str> = scanned.code.lines().collect();
    let mut findings = Vec::new();
    for declaration in declarations(&scanned.code) {
        if !matches!(declaration.keyword, "fn" | "struct" | "enum" | "trait") {
            continue;
        }
        if has_preceding_doc(&code_lines, &doc_lines, declaration.line) {
            continue;
        }
        findings.push(Finding::new(
            "comments/public-api-missing-doc",
            Severity::Error,
            relative_path,
            declaration.line,
            format!(
                "`pub {} {}` 缺少文档注释：公共 API 必须写清语义、错误语义、副作用与幂等性（naming §10 ③）",
                declaration.keyword, declaration.name
            ),
        ));
    }
    findings
}

/// 规则 ④：文件开头没有 `//!` 模块文档。
#[must_use]
pub fn check_module_header(
    relative_path: &str,
    source: &str,
    scanned: &crate::rustscan::Scan,
) -> Vec<Finding> {
    let has_module_doc = scanned
        .comments
        .iter()
        .take(3)
        .any(|comment| comment.kind == CommentKind::ModuleDoc);
    if has_module_doc {
        return Vec::new();
    }
    // 空文件不是"缺模块头"，是"没有内容"：避免对空文件产生噪声
    if source.trim().is_empty() {
        return Vec::new();
    }
    let strict = MODULE_HEADER_STRICT_PREFIXES
        .iter()
        .any(|prefix| relative_path.starts_with(prefix));
    let severity = if strict {
        Severity::Error
    } else {
        Severity::Warning
    };
    vec![Finding::new(
        "comments/module-header-missing",
        severity,
        relative_path,
        1,
        format!(
            "缺少 `//!` 模块头（职责 / 边界 / 不变量）{}（naming §10 ④）",
            if strict {
                "：该 crate 属强制范围，缺头为阻塞级"
            } else {
                "：建议补齐"
            }
        ),
    )]
}

/// 规则 ⑤：`unsafe` 出现处缺 `// SAFETY:` 说明。
#[must_use]
pub fn check_unsafe_safety(relative_path: &str, scanned: &crate::rustscan::Scan) -> Vec<Finding> {
    let code_lines: Vec<&str> = scanned.code.lines().collect();
    let mut findings = Vec::new();
    for (index, line) in code_lines.iter().enumerate() {
        if !contains_unsafe_token(line) {
            continue;
        }
        let line_number = index + 1;
        // `unsafe` 可能出现在属性行（`#![allow(unsafe_code)]` 不含 `unsafe ` 关键字形式），
        // 也可能在文档注释里 —— 后者在 code 视图中已被抹平，不会命中。
        if has_safety_comment_above(scanned, line_number) {
            continue;
        }
        findings.push(Finding::new(
            "comments/unsafe-without-safety",
            Severity::Error,
            relative_path,
            line_number,
            "`unsafe` 块缺少紧邻上一行的 `// SAFETY:` 说明（naming §8 / §10 ⑤）".to_owned(),
        ));
    }
    findings
}

/// 规则 ⑥：`PITFALL` 标签必须写成 `PITFALL(app=<id>)` 或 `PITFALL(platform=<os>)`。
#[must_use]
pub fn check_pitfall_format(relative_path: &str, comments: &[Comment]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for comment in comments {
        if !comment.text.contains("PITFALL") {
            continue;
        }
        if comment.text.contains("PITFALL(app=") || comment.text.contains("PITFALL(platform=") {
            continue;
        }
        findings.push(Finding::new(
            "comments/pitfall-format",
            Severity::Warning,
            relative_path,
            comment.line,
            "`PITFALL` 标签必须写成 `PITFALL(app=<id>):` 或 `PITFALL(platform=<os>):`（naming §8 / §10 ⑥）"
                .to_owned(),
        ));
    }
    findings
}

/// 规则 ⑦：声明名中出现受控词汇表的禁用同义词。
#[must_use]
pub fn check_controlled_vocabulary(
    relative_path: &str,
    scanned: &crate::rustscan::Scan,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for declaration in declarations(&scanned.code) {
        for stem in split_stems(&declaration.name) {
            if !BANNED_SYNONYM_STEMS.contains(&stem.as_str()) {
                continue;
            }
            findings.push(Finding::new(
                "comments/controlled-vocabulary",
                Severity::Warning,
                relative_path,
                declaration.line,
                format!(
                    "`{}` 命中受控词汇表的禁用同义词 `{stem}`：同一概念全项目只用一个词（naming §4 / §10 ⑦）",
                    declaration.name
                ),
            ));
        }
    }
    findings
}

/// 规则 ⑧：**公开**声明名中出现疑似缩写的词干（启发式，警告级）。
///
/// 判据见 `BANNED_ABBREVIATIONS` 的说明：2~4 字母无元音词干，或命中显式黑名单。
/// 为什么只扫公开声明：私有局部变量的短名是局部可读性问题，而 naming §1 的目标是
/// "一眼可懂"的**对外**命名；全量扫描会把警告量推到几百条，而几百条警告等于没有警告。
#[must_use]
pub fn check_abbreviations(relative_path: &str, scanned: &crate::rustscan::Scan) -> Vec<Finding> {
    let mut findings = Vec::new();
    for declaration in declarations(&scanned.code) {
        for stem in split_stems(&declaration.name) {
            if !is_suspected_abbreviation(&stem) {
                continue;
            }
            findings.push(Finding::new(
                "comments/abbreviation",
                Severity::Warning,
                relative_path,
                declaration.line,
                format!(
                    "`{}` 含疑似缩写 `{stem}`：naming §3 只允许 23 个缩写，其余请写全（naming §10 ⑧）",
                    declaration.name
                ),
            ));
        }
    }
    findings
}

/// 规则 ⑧ 的判据：白名单外的「无元音短词干」或「显式黑名单词」。
fn is_suspected_abbreviation(stem: &str) -> bool {
    if ABBREVIATION_ALLOWLIST.contains(&stem) {
        return false;
    }
    if BANNED_ABBREVIATIONS.contains(&stem) {
        return true;
    }
    let length = stem.chars().count();
    if !(2..=4).contains(&length) || !stem.chars().all(|character| character.is_ascii_lowercase()) {
        return false;
    }
    !stem.chars().any(|character| VOWELS.contains(&character))
}

/// 行号集合：条目/模块文档注释所在行。
fn doc_comment_lines(comments: &[Comment]) -> Vec<usize> {
    comments
        .iter()
        .filter(|comment| matches!(comment.kind, CommentKind::LineDoc | CommentKind::ModuleDoc))
        .map(|comment| comment.line)
        .collect()
}

/// 判断某行之前是否存在文档注释（只穿过文档行与属性行）。
fn has_preceding_doc(code_lines: &[&str], doc_lines: &[usize], line: usize) -> bool {
    let mut cursor = line.saturating_sub(1);
    while cursor >= 1 {
        let Some(text) = code_lines.get(cursor - 1) else {
            return false;
        };
        if doc_lines.contains(&cursor) {
            return true;
        }
        let trimmed = text.trim();
        // `#[doc = "..."]` 与 `#[doc = concat!(...)]` 是宏里唯一可用的文档形式
        // （宏展开的 `pub struct $name` 无法挂 `///`），必须与 `///` 同等对待。
        if trimmed.starts_with("#[doc") {
            return true;
        }
        if trimmed.is_empty() || trimmed.starts_with("#[") {
            cursor -= 1;
            continue;
        }
        return false;
    }
    false
}

/// 从某行向上找 `// SAFETY:`，允许说明覆盖**紧邻的连续语句组**。
///
/// 判据的两次收紧都来自实跑：
/// ① `// SAFETY:` 说明通常占 2~3 行（首行结论 + 续行理由），只看紧邻上一行会把
///    已经写好说明的 `unsafe` 误判成违规（2026-09-29 首次实跑 49 条全是这个误报）；
/// ② 同一段只读属性查询常写成一串 `let x = unsafe { ... }.map_err(...).as_bool();`，
///    作者用**一条** `// SAFETY:` 说明整组 —— 这正是本仓库既有的写法（TASK-017 审计
///    记录也写着「unsafe 均有 SAFETY 注释」）。因此向上走到**空行**为止，
///    空行是"这组说明到此结束"的唯一明确信号。
fn has_safety_comment_above(scanned: &crate::rustscan::Scan, line: usize) -> bool {
    let code_lines: Vec<&str> = scanned.code.lines().collect();
    let mut cursor = line.saturating_sub(1);
    let mut steps = 0;
    // 上界 8 行：够覆盖"说明 + 3~4 条连续 unsafe 语句"，又不至于跨到上一段代码。
    while cursor >= 1 && steps < 8 {
        steps += 1;
        if let Some(comment) = scanned
            .comments
            .iter()
            .find(|comment| comment.line == cursor)
        {
            if comment.text.contains("SAFETY") {
                return true;
            }
            cursor -= 1;
            continue;
        }
        // 非注释行：空行 = 说明组结束；单独的 `}` = 上一块结束。
        let trimmed = code_lines.get(cursor - 1).copied().unwrap_or("").trim();
        if trimmed.is_empty() || trimmed == "}" {
            return false;
        }
        cursor -= 1;
    }
    false
}

/// `unsafe` 关键字判定：必须是独立词，避免 `unsafe_code` 之类的标识符命中。
fn contains_unsafe_token(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("unsafe ")
        || trimmed.starts_with("unsafe{")
        || trimmed.contains(" unsafe ")
        || trimmed.contains("= unsafe ")
}

/// 从降噪代码里抽取声明（`pub` 前缀 + 关键字 + 名称）。
fn declarations(code: &str) -> Vec<Declaration> {
    const KEYWORDS: [&str; 7] = ["fn", "struct", "enum", "trait", "const", "static", "type"];
    /// `pub` 与真正关键字之间可能出现的修饰符（`pub async fn` / `pub const fn` …）。
    const MODIFIER_LOOKAHEAD: usize = 3;
    let mut declarations = Vec::new();
    for (index, line) in code.lines().enumerate() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        // 找出前 3 个词里第一个真关键字：`pub const fn new` 的关键字是 `fn`（不是 `const`），
        // 否则会把 `fn` 本身当成名字 —— 2026-09-29 首次实跑时正是这个 bug 产生了 500+ 条噪声。
        let Some((keyword, name_index)) = find_keyword(&tokens, &KEYWORDS, MODIFIER_LOOKAHEAD)
        else {
            continue;
        };
        let Some(raw_name) = tokens.get(name_index).copied() else {
            continue;
        };
        let Some(name) = declaration_name(raw_name) else {
            continue;
        };
        declarations.push(Declaration {
            line: index + 1,
            keyword: keyword_static(keyword),
            name,
        });
    }
    declarations
}

/// 在 `tokens` 的前 `lookahead` 个词里找出关键字，并返回名字所在的下标。
///
/// `pub const fn new` 必须解析成 `(fn, 2)`：`const` 在这里是修饰符而不是声明关键字。
fn find_keyword<'a>(
    tokens: &[&'a str],
    keywords: &[&str],
    lookahead: usize,
) -> Option<(&'a str, usize)> {
    for (index, token) in tokens.iter().take(lookahead).enumerate() {
        if !keywords.contains(token) {
            continue;
        }
        if *token == "const" && tokens.get(index + 1).copied() == Some("fn") {
            return Some(("fn", index + 2));
        }
        return Some((token, index + 1));
    }
    None
}

/// 取声明名：到 `(`, `<`, `:`, ` `, `;`, `{` 为止。
fn declaration_name(remainder: &str) -> Option<String> {
    let name: String = remainder
        .chars()
        .take_while(|character| !matches!(character, '(' | '<' | ':' | ' ' | ';' | '{' | '='))
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

/// 把 `&str` 关键字转成 `'static`（关键字来自固定集合，故是安全的查表）。
fn keyword_static(keyword: &str) -> &'static str {
    match keyword {
        "fn" => "fn",
        "struct" => "struct",
        "enum" => "enum",
        "trait" => "trait",
        "const" => "const",
        "static" => "static",
        "type" => "type",
        _ => "item",
    }
}

/// 把 `CamelCase` / `snake_case` 标识符切成小写词干。
fn split_stems(name: &str) -> Vec<String> {
    let mut stems = Vec::new();
    let mut current = String::new();
    let mut previous_lowercase = false;
    for character in name.chars() {
        if character == '_' {
            if !current.is_empty() {
                stems.push(current.to_lowercase());
                current.clear();
            }
            previous_lowercase = false;
            continue;
        }
        if character.is_ascii_uppercase() && previous_lowercase && !current.is_empty() {
            stems.push(current.to_lowercase());
            current.clear();
        }
        previous_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
        current.push(character);
    }
    if !current.is_empty() {
        stems.push(current.to_lowercase());
    }
    stems
}

#[cfg(test)]
#[path = "comments_tests.rs"]
mod tests;
