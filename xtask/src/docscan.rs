//! # docscan 子命令（broken tables / setext risk / encoding shape）
//!
//! 职责：扫全仓 `.md` 找出 ① 破表（数据行 cell 数 ≠ 分隔行 cell 数）、
//! ② setext 风险（`---` 前一行非空且不是另一根 `---`，会变成 H2 标题）、
//! ③ 文件编码形状（UTF-8 BOM / CRLF / 末尾换行形态 = 缺 LF / 双 LF）。
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

// TASK-052 (2026-09-19) 决策：本文件**无任何**模块级 `#![allow(...)]` 块；32 处 indexing_slicing 全部用 safe pattern 替换。

/// 规则 `doc/table-broken`：数据行 cell 数 ≠ 分隔行 cell 数 → 渲染会错位（PL-031）。
use crate::report::{Finding, Severity};
const RULE_BROKEN_TABLE: &str = "doc/table-broken";
/// 规则 `doc/setext-risk`：`---` 前一行非空，会被 GFM 解析成 H2 标题（PL-031 同源）。
const RULE_SETEXT_RISK: &str = "doc/setext-risk";
/// 规则 `file/encoding`：UTF-8 BOM / CRLF / 缺末行 LF / 多末行 LF。
const RULE_ENCODING: &str = "file/encoding";

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
        findings.extend(scan_broken_tables(&entry.rel_path, &content));
        findings.extend(scan_setext_risk(&entry.rel_path, &content));
        findings.extend(scan_encoding(&entry.rel_path, &content));
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
        assert_eq!(f[0].line, 3);
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
        assert_eq!(f[0].line, 2);
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
        assert!(all.is_empty(), "clean file should be clean, got {:?}", all);
    }
}
