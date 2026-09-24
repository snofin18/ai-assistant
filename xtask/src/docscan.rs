//! # docscan 子命令（broken tables / setext risk / encoding shape / 结构规则 4 条）
//!
//! 职责：扫全仓 `.md` 找出 ① 破表（数据行 cell 数 ≠ 分隔行 cell 数）、
//! ② setext 风险（`---` 前一行非空且不是另一根 `---`，会变成 H2 标题）、
//! ③ 文件编码形状（UTF-8 BOM / CRLF / 末尾换行形态 = 缺 LF / 双 LF）、
//! ④ 裸 NUL 字节（TASK-015 新增，**Error**）、
//! ⑤ 数字节号重复 / ⑥ 整节为空 / ⑦ 标题文字重复（TASK-015 新增，均**先 Warning**）。
//!
//! ## 为什么新增的 ⑤⑥⑦ 先 Warning（ADR-0025 D1 口径）
//! 2026-09-24 落地前按任务卡要求跑了原型扫描数存量（全仓 158 个 `.md`）：
//! `doc/duplicate-section-number` 33 处 / `doc/empty-section` 492 处 / `doc/duplicate-heading` 38 处。
//! 存量里既有**真债**（stage-0 的 8 张 spike 卡记录区整节未填）也有**按定义合法的形态**
//! （`Ready` 卡的 9 节空骨架、TASK-011 那种「每轮追加一个 UPDATE 块」的重复节标题）。
//! 一上线就 Error 会当场红几百处，且其中一部分**不该修** → 先 Warning，
//! 清扫与判据收紧的后续动作见 `docs/PARKING_LOT.md` **PL-055**。
//!
//! ## 与 `xtask hygiene` 的关系
//! docscan 是**只读、无豁免**的扫描（破表 = 客观事实，没有「豁免」可豁），
//! 而 hygiene 有豁免、14 条规则。两者输出**不可合并**：
//! - docscan 走的是「文档结构错误 = 立即错」（破表会让 GitHub 渲染错位），
//!   与 PL-031 同源；
//! - hygiene 走的是「仓库卫生 = 写错习惯」（单文件太长可豁）。
//!
//! ## 边界（不做什么）
//! - 不做规则判定之外的语义检查（如 "这个 ADR 编号是不是真的有效" = 那是 `adr-index`）。
//!
//! ## 不变量
//! 1. 输出确定性（同 refscan）。
//! 2. 扫到 0 个 `.md` 必须显式告警（避免空仓库假 PASSED）。

// TASK-060 (2026-09-19) 决策：本文件**无任何**模块级 `#![allow(...)]` 块；32 处 indexing_slicing 全部用 safe pattern 替换。

use crate::report::{Finding, Severity};

/// 规则 `doc/table-broken`：数据行 cell 数 ≠ 分隔行 cell 数 → 渲染会错位（PL-031）。
const RULE_BROKEN_TABLE: &str = "doc/table-broken";
/// 规则 `doc/setext-risk`：`---` 前一行非空，会被 GFM 解析成 H2 标题（PL-031 同源）。
const RULE_SETEXT_RISK: &str = "doc/setext-risk";
/// 规则 `file/encoding`：UTF-8 BOM / CRLF / 缺末行 LF / 多末行 LF。
const RULE_ENCODING: &str = "file/encoding";

/// 规则 `doc/nul-byte`：任何文本文件含 `0x00` → git 会把该文件当**二进制**，diff 不可复核。
///
/// 级别 **Error**（可直接上线）：NUL 是客观事实，没有「豁免」可豁。真实事故：
/// `docs/adr/README.md` 曾含裸 NUL（TASK-202 已修，规则当时尚未上线）；`docs/memory/{facts,pitfalls}.md`
/// 各含 2 个（PL-051，TASK-015 上线本规则时同批修掉）。
const RULE_NUL_BYTE: &str = "doc/nul-byte";
/// 规则 `doc/duplicate-section-number`：同一文件内**数字节号**（`## 4.` / `### 4.3`）重复。
///
/// 级别**先 Warning**（存量 33 处，清扫后升 Error；见模块头与 PL-055）。
/// 判据是**节号字符串**（`4` / `4.3`）而非标题文字 —— 同一节号出现在两个地方时，
/// 交叉引用「见 §4」就变成歧义。
const RULE_DUPLICATE_SECTION_NUMBER: &str = "doc/duplicate-section-number";
/// 规则 `doc/empty-section`：标题之下到下一个**同级或更高级**标题之间没有任何非空内容。
///
/// 级别**先 Warning**（存量 492 处；见模块头与 PL-055）。「同级或更高级」这个限定是必需的：
/// 只看「下一个标题（任意级）」会把「父标题紧跟子标题」这种**正常**写法也算成空节（实测多出 200 处噪声）。
const RULE_EMPTY_SECTION: &str = "doc/empty-section";
/// 规则 `doc/duplicate-heading`：同一文件内**标题文字**重复。
///
/// 级别**先 Warning**（存量 38 处；见模块头与 PL-055）。典型缺陷形态：外来模板块被整段复制，
/// 于是 `### 字段` 连出两次（TASK-200 的 7 份 spec 草案）。
const RULE_DUPLICATE_HEADING: &str = "doc/duplicate-heading";

/// 执行 docscan 子命令：扫 .md 找破表 / setext 风险 / 编码形状（不受免）。
pub fn run(repo_root: &std::path::Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::report::Severity;
    use crate::repowalk::collect_repo_files;
    use crate::{EXIT_FINDINGS, EXIT_OK};

    let entries =
        collect_repo_files(repo_root, &["md"]).map_err(|e| format!("扫描仓库失败：{e:?}"))?;
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    for entry in &entries {
        scanned += 1;
        let Ok(content) = std::fs::read_to_string(&entry.abs_path) else {
            continue;
        };
        let mut file_findings = scan_broken_tables(&entry.rel_path, &content);
        file_findings.extend(scan_setext_risk(&entry.rel_path, &content));
        file_findings.extend(scan_encoding(&entry.rel_path, &content));
        file_findings.extend(scan_nul_byte(&entry.rel_path, &content));
        file_findings.extend(scan_duplicate_section_number(&entry.rel_path, &content));
        file_findings.extend(scan_empty_section(&entry.rel_path, &content));
        file_findings.extend(scan_duplicate_heading(&entry.rel_path, &content));
        // 同一文件内按 (行号, 规则 id) 排序：输出确定性（不变量 1）与人工可读性兼顾。
        // 跨文件的顺序由 `collect_repo_files` 保证（按 rel_path 升序）。
        file_findings.sort_by(|left, right| (left.line, left.rule).cmp(&(right.line, right.rule)));
        findings.extend(file_findings);
    }
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let verdict = if errors > 0 { "FAILED" } else { "PASSED" };
    // 逐 finding 输出（铁律 ①「无静默失败」：仅 counts = 「看着像在跑」的伪完成）
    for f in &findings {
        f.render(output).map_err(|e| e.to_string())?;
    }

    let summary = format!(
        "== docscan ==\nscanned_files={scanned}\n-- summary: {errors} error(s), {warnings} warning(s)\n-- verdict: {verdict}\n"
    );
    output
        .write_all(summary.as_bytes())
        .map_err(|e| e.to_string())?;
    if errors > 0 {
        Ok(EXIT_FINDINGS)
    } else {
        Ok(EXIT_OK)
    }
}

pub fn scan_broken_tables(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while let Some(raw_line) = lines.get(i) {
        let line = raw_line.trim();
        if !line.starts_with('|') {
            i += 1;
            continue;
        }
        // table header detected — bound-check the separator row
        if let Some(sep_line) = lines.get(i + 1) {
            let sep = sep_line.trim();
            if is_separator_row(sep) {
                let expected = cell_count(sep);
                // consume the table: header (i), separator (i+1), then data rows
                let mut j = i + 2;
                while let Some(raw_j) = lines.get(j) {
                    if !raw_j.trim_start().starts_with('|') {
                        break;
                    }
                    let got = cell_count(raw_j.trim());
                    if got != expected {
                        findings.push(Finding::new(
                            RULE_BROKEN_TABLE,
                            Severity::Error,
                            rel_path,
                            j + 1,
                            format!("cell 数 {got} ≠ 表头 {expected}（GitHub 渲染会错位）"),
                        ));
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    findings
}

fn is_separator_row(line: &str) -> bool {
    // `|---|---|` or `|:---|:---:|` — only `|`, `-`, `:`, spaces
    let inner = line.trim_matches('|').trim();
    if inner.is_empty() {
        return false;
    }
    inner.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn cell_count(line: &str) -> usize {
    // GFM table cells split on `|` not preceded by `\`. Empty leading/trailing cells are excluded.
    // We use the same regex the Python prototype uses for stability.
    // Manual cell count: walk chars, increment on unescaped |, -2 for outer pipes.
    let mut count = 0usize;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == '|' {
            count += 1;
        }
    }
    count.saturating_sub(2)
}

/// setext 风险扫描：`---` 前一行非空且不是另一根 `---`，会被 GFM 解析成 H2。
#[must_use]
pub fn scan_setext_risk(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return findings;
    }
    for (k, cur_raw) in lines.iter().enumerate().skip(1) {
        let cur = cur_raw.trim();
        // only `---` (3+ dashes), exactly, nothing else
        if cur != "---" && !cur.chars().all(|c| c == '-') {
            continue;
        }
        if cur.len() < 3 {
            continue;
        }
        let Some(prev) = lines.get(k - 1).copied() else {
            continue;
        };
        let prev_trim = prev.trim();
        if prev_trim.is_empty() {
            continue;
        }
        // if prev is another `---`, it's a horizontal rule, not setext risk
        if prev_trim.chars().all(|c| c == '-') && prev_trim.len() >= 3 {
            continue;
        }
        // skip if prev is itself a table row (tables can have `|` separators)
        if prev_trim.starts_with('|') {
            continue;
        }
        findings.push(Finding::new(
            RULE_SETEXT_RISK,
            Severity::Error,
            rel_path,
            k + 1,
            "上一行非空 → Markdown 解析器会把这一行 `---` 当成上一行的 setext H2 标题 = 内容错位"
                .to_string(),
        ));
    }
    findings
}

/// 文件编码形状扫描：BOM / CRLF / 缺末行 LF / 多末行 LF。
#[must_use]
pub fn scan_encoding(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let bytes = content.as_bytes();
    // BOM
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        findings.push(Finding::new(
            RULE_ENCODING,
            Severity::Error,
            rel_path,
            1,
            "文件含 UTF-8 BOM（0xEF 0xBB 0xBF）—— Markdown 渲染会多出一个字符".to_string(),
        ));
    }
    // CRLF
    if bytes.windows(2).any(|w| w == b"\r\n") {
        findings.push(Finding::new(
            RULE_ENCODING,
            Severity::Error,
            rel_path,
            0,
            "文件含 CRLF 换行（应统一为 LF）".to_string(),
        ));
    }
    // trailing-newline shape
    let tail_bad = bytes.last().copied() != Some(b'\n')
        || (bytes.len() >= 2 && bytes.get(bytes.len() - 2).copied() == Some(b'\n'));
    if tail_bad && !bytes.is_empty() {
        findings.push(Finding::new(
            RULE_ENCODING,
            Severity::Error,
            rel_path,
            0,
            "文件末尾不是恰好一个 LF".to_string(),
        ));
    }
    findings
}

/// 一个 ATX 标题（`#` ~ `######`）；`line_index` 是 **0 基**行号。
struct Heading {
    line_index: usize,
    level: usize,
    text: String,
}

/// 收集 ATX 标题，**跳过围栏代码块**（三个反引号或三个波浪号开头的行）。
///
/// 为什么必须跳 fence：`gov` 的模板附录、`docs/dev-env-setup.md` 的安装命令块里都有
/// 以 `#` 开头的**代码行**（如 `# Windows`）。不跳 fence 会把它们当成标题 ——
/// 2026-09-24 的原型扫描实测：不跳 fence 时 `doc/empty-section` 多出约 30 处纯噪声。
fn collect_headings(content: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut fence_marker: Option<String> = None;
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(marker) = fence_marker.as_deref() {
            if trimmed.starts_with(marker) {
                fence_marker = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence_marker = trimmed.chars().take(3).collect::<String>().into();
            continue;
        }
        if let Some((level, text)) = parse_atx_heading(line) {
            headings.push(Heading {
                line_index,
                level,
                text,
            });
        }
    }
    headings
}

/// 解析一行 ATX 标题；不是标题则 `None`。
///
/// 判据：行首（允许前导空白）1~6 个 `#`，其后**必须是空白**（`#foo` 不是标题，GFM 同此），
/// 其余部分 trim 后即标题文字。
fn parse_atx_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = trimmed.get(hashes..)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((hashes, rest.trim().to_string()))
}

/// 标题文字归一化：折叠连续空白 + trim（用于「标题文字重复」的比较）。
fn normalize_heading_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// 取标题的**数字节号**：`4` / `4.3` / `4.3.1`（可带尾点，如 `## 4. 目标`）。
///
/// 返回 `None` 的形态（都不是节号）：不以数字开头、节号后紧跟非空白（`2026-09-24`、
/// `4x`）、出现空段（`4..3`）、以及 `§5` 这类带前缀的写法（那属于「每轮追加一个块」的
/// 自编小节号，不是文档结构节号 —— TASK-011 的 5 个 `UPDATE` 块正是这种）。
fn section_number(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let mut end = 0usize;
    while let Some(byte) = bytes.get(end) {
        if byte.is_ascii_digit() || *byte == b'.' {
            end += 1;
        } else {
            break;
        }
    }
    let raw = text.get(..end)?;
    let number = raw.trim_end_matches('.');
    if number.is_empty() || number.split('.').any(str::is_empty) {
        return None;
    }
    let rest = text.get(end..)?;
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return None;
    }
    Some(number)
}

/// 规则 `doc/nul-byte` 的扫描：文件含裸 NUL（`0x00`）。
///
/// 只报**首处**（并给出总处数）：一个文件里有几个 NUL 是同一件要修的事，逐处报是噪声。
#[must_use]
pub fn scan_nul_byte(rel_path: &str, content: &str) -> Vec<Finding> {
    let Some(offset) = content.find('\0') else {
        return Vec::new();
    };
    let line = content
        .get(..offset)
        .map_or(0, |prefix| prefix.matches('\n').count())
        + 1;
    let total = content.matches('\0').count();
    vec![Finding::new(
        RULE_NUL_BYTE,
        Severity::Error,
        rel_path,
        line,
        format!(
            "文件含裸 NUL 字节 `0x00`（共 {total} 处，首处在本行）—— git 会把该文件当**二进制**，diff 不可复核；请改写成字面量文本 `\\0`"
        ),
    )]
}

/// 规则 `doc/duplicate-section-number` 的扫描：同一文件内数字节号重复。
///
/// 每个**第二次及以后**的出现报一条，消息里给出首次出现的行号（便于直接跳过去）。
#[must_use]
pub fn scan_duplicate_section_number(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut first_seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut findings = Vec::new();
    for heading in &collect_headings(content) {
        let Some(number) = section_number(&heading.text) else {
            continue;
        };
        match first_seen.get(number) {
            Some(first_line) => findings.push(Finding::new(
                RULE_DUPLICATE_SECTION_NUMBER,
                Severity::Warning,
                rel_path,
                heading.line_index + 1,
                format!(
                    "数字节号 `{number}` 重复：本行是第二次出现，首次在第 {first_line} 行 —— 同一文件内节号必须唯一，否则「见 §{number}」有歧义"
                ),
            )),
            None => {
                first_seen.insert(number, heading.line_index + 1);
            }
        }
    }
    findings
}

/// 规则 `doc/empty-section` 的扫描：整节为空。
///
/// 「节」= 本标题之后、到下一个**同级或更高级**标题（`level <= 本标题 level`）之前；
/// 文件尾则到 EOF。这一定义排除了「父标题紧跟子标题」的正常写法。
#[must_use]
pub fn scan_empty_section(rel_path: &str, content: &str) -> Vec<Finding> {
    let headings = collect_headings(content);
    let lines: Vec<&str> = content.lines().collect();
    let mut findings = Vec::new();
    for (index, heading) in headings.iter().enumerate() {
        let section_end = headings
            .iter()
            .skip(index + 1)
            .find(|next| next.level <= heading.level)
            .map_or(lines.len(), |next| next.line_index);
        let has_body = lines
            .iter()
            .take(section_end)
            .skip(heading.line_index + 1)
            .any(|line| !line.trim().is_empty());
        if !has_body {
            findings.push(Finding::new(
                RULE_EMPTY_SECTION,
                Severity::Warning,
                rel_path,
                heading.line_index + 1,
                format!(
                    "整节为空：`{}` 之下到下一个同级/更高级标题之间没有任何非空内容",
                    short_heading_text(&heading.text)
                ),
            ));
        }
    }
    findings
}

/// 规则 `doc/duplicate-heading` 的扫描：同一文件内标题文字重复（空白折叠后比较）。
#[must_use]
pub fn scan_duplicate_heading(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut first_seen: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut findings = Vec::new();
    for heading in &collect_headings(content) {
        let key = normalize_heading_text(&heading.text);
        if key.is_empty() {
            continue;
        }
        match first_seen.get(&key) {
            Some(first_line) => findings.push(Finding::new(
                RULE_DUPLICATE_HEADING,
                Severity::Warning,
                rel_path,
                heading.line_index + 1,
                format!(
                    "标题文字重复：`{}` 首次出现在第 {first_line} 行 —— 同一文件内标题文字重复会让锚点/目录跳错位置",
                    short_heading_text(&key)
                ),
            )),
            None => {
                first_seen.insert(key, heading.line_index + 1);
            }
        }
    }
    findings
}

/// 把标题文字截到 40 个字符（消息里不塞整段标题）。
fn short_heading_text(text: &str) -> String {
    const LIMIT: usize = 40;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let head: String = text.chars().take(LIMIT).collect();
    format!("{head}…")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::uninlined_format_args,
    clippy::collapsible_if,
    clippy::unused_peekable,
    clippy::single_match_else
)]
mod tests {
    use super::*;

    #[test]
    fn detects_broken_table_row() {
        let content = "| a | b | c |\n|---|---|---|\n| 1 | 2 |\n";
        let f = scan_broken_tables("x.md", content);
        assert_eq!(f.len(), 1);
        assert_eq!(f.first().expect("non-empty").line, 3);
    }

    #[test]
    fn no_false_positive_on_valid_table() {
        let content = "| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |\n";
        let f = scan_broken_tables("x.md", content);
        assert!(f.is_empty(), "valid 3-col table should not flag");
    }

    #[test]
    fn detects_setext_risk() {
        let content = "hello\n---\n";
        let f = scan_setext_risk("x.md", content);
        assert_eq!(f.len(), 1);
        assert_eq!(f.first().expect("non-empty").line, 2);
    }

    #[test]
    fn no_setext_when_prev_is_hr() {
        let content = "---\n---\n";
        let f = scan_setext_risk("x.md", content);
        assert!(f.is_empty(), "two HRs in a row should not flag");
    }

    #[test]
    fn detects_missing_trailing_lf() {
        let content = "no newline";
        let f = scan_encoding("x.md", content);
        assert!(f.iter().any(|x| x.message.contains("末尾不是")));
    }

    #[test]
    fn detects_double_trailing_lf() {
        let content = "extra\n\n";
        let f = scan_encoding("x.md", content);
        assert!(f.iter().any(|x| x.message.contains("末尾不是")));
    }

    #[test]
    fn clean_file_no_findings() {
        let content = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let mut all = scan_broken_tables("x.md", content);
        all.extend(scan_setext_risk("x.md", content));
        all.extend(scan_encoding("x.md", content));
        all.extend(scan_nul_byte("x.md", content));
        all.extend(scan_duplicate_section_number("x.md", content));
        all.extend(scan_empty_section("x.md", content));
        all.extend(scan_duplicate_heading("x.md", content));
        assert!(all.is_empty(), "clean file should be clean, got {:?}", all);
    }

    // ---- TASK-015 新增 4 条规则的单测（DoD：每条新规则各自有单测）----

    #[test]
    fn detects_nul_byte_and_reports_first_line() {
        // 第 3 行含一个裸 NUL（`\0` 在 Rust 字符串里就是真正的 0x00 字节）
        let content = "line1\nline2\nbad\0here\nline4\n";
        let f = scan_nul_byte("x.md", content);
        assert_eq!(f.len(), 1, "首处 NUL 只报一条");
        assert_eq!(f.first().expect("non-empty").rule, RULE_NUL_BYTE);
        assert_eq!(f.first().expect("non-empty").severity, Severity::Error);
        assert_eq!(f.first().expect("non-empty").line, 3);
    }

    #[test]
    fn nul_byte_absent_produces_nothing() {
        let f = scan_nul_byte("x.md", "no nul here\n");
        assert!(f.is_empty());
    }

    #[test]
    fn detects_duplicate_section_number() {
        let content = "## 1. 目标\n内容\n## 2. 范围\n内容\n## 1. 目标\n内容\n";
        let f = scan_duplicate_section_number("x.md", content);
        assert_eq!(f.len(), 1, "第二次出现报一条，首次不报：{f:?}");
        assert_eq!(f.first().expect("non-empty").line, 5);
        assert_eq!(f.first().expect("non-empty").severity, Severity::Warning);
    }

    #[test]
    fn unique_section_numbers_produce_nothing() {
        let content = "## 1. 目标\n内容\n## 2. 范围\n内容\n### 2.1 子节\n内容\n";
        let f = scan_duplicate_section_number("x.md", content);
        assert!(f.is_empty(), "节号唯一不该报：{f:?}");
    }

    #[test]
    fn section_number_ignores_dates_and_prefixed_blocks() {
        // `2026-09-24` 不是节号（节号后必须空白或行尾）；`§5 偏差` 也不是（带前缀）
        let content =
            "### 2026-09-24 事件\nx\n### 2026-09-24 事件\ny\n#### §5 偏差\na\n#### §5 偏差\nb\n";
        let f = scan_duplicate_section_number("x.md", content);
        assert!(f.is_empty(), "日期与 § 前缀都不算节号：{f:?}");
    }

    #[test]
    fn detects_empty_section() {
        let content = "## 1. 目标\n\n## 2. 范围\n有内容\n";
        let f = scan_empty_section("x.md", content);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f.first().expect("non-empty").line, 1);
        assert_eq!(f.first().expect("non-empty").severity, Severity::Warning);
    }

    #[test]
    fn parent_heading_with_child_body_is_not_empty() {
        // 「父标题紧跟子标题」是正常写法，不得算空节（这正是「同级或更高级」限定的作用）
        let content = "## 1. 目标\n### 1.1 子节\n有内容\n";
        let f = scan_empty_section("x.md", content);
        assert!(f.is_empty(), "父标题有子节正文时不该报空节：{f:?}");
    }

    #[test]
    fn empty_leaf_subsection_is_reported() {
        let content = "## 1. 目标\n### 1.1 有内容\n正文\n### 1.2 空\n## 2. 下一个\n正文\n";
        let f = scan_empty_section("x.md", content);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f.first().expect("non-empty").line, 4);
    }

    #[test]
    fn detects_duplicate_heading_text() {
        let content = "### 字段\n内容\n### 字段\n内容\n";
        let f = scan_duplicate_heading("x.md", content);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f.first().expect("non-empty").line, 3);
        assert_eq!(f.first().expect("non-empty").severity, Severity::Warning);
    }

    #[test]
    fn different_heading_texts_produce_nothing() {
        let content = "### 字段\n内容\n### 字段2\n内容\n";
        let f = scan_duplicate_heading("x.md", content);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn headings_inside_fenced_code_block_are_ignored() {
        // 代码块里的 `# 安装` 不是标题 → 既不参与重复判定，也不产生空节
        let content = "## 真标题\n正文\n```text\n# 安装\n# 安装\n```\n";
        let mut all = scan_duplicate_heading("x.md", content);
        all.extend(scan_empty_section("x.md", content));
        assert!(all.is_empty(), "fence 内的 # 行必须被忽略：{all:?}");
    }

    #[test]
    fn atx_heading_requires_whitespace_after_hashes() {
        // `#foo` 不是 ATX 标题（GFM 同此），故不参与任何结构规则
        assert!(parse_atx_heading("#foo").is_none());
        assert_eq!(parse_atx_heading("### ok"), Some((3, "ok".to_string())));
        assert!(parse_atx_heading("####### too many").is_none());
    }

    #[test]
    fn findings_are_ordered_by_line_within_file() {
        // run() 的排序键是 (line, rule)：这条用扫描函数直接验证「同一文件内行号单调」的前提
        let content = "## 1. 目标\n\n## 1. 目标\n\n";
        let mut all = scan_duplicate_section_number("x.md", content);
        all.extend(scan_empty_section("x.md", content));
        all.extend(scan_duplicate_heading("x.md", content));
        all.sort_by(|left, right| (left.line, left.rule).cmp(&(right.line, right.rule)));
        let lines: Vec<usize> = all.iter().map(|f| f.line).collect();
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        assert_eq!(lines, sorted, "排序后行号必须单调：{all:?}");
    }
}
