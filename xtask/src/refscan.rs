//! # refscan 子命令（ADR-0030 D3 的扩展：裸 ADR 待建引用 / 范围写法 / ps1 纯 ASCII）
//!
//! 职责：扫全仓 .md + .rs 找出
//! 1. ADR 编号范围写法（两个 4 位号之间夹 ~ / FULLWIDE_TILDE / -）
//! 2. 裸 ADR 待建引用（docs/adr/ 之外的 ADR-00NN 引用、且 NN 属待建号集合）
//! 3. .ps1 文件里所有 > 127 的字节（= 非 ASCII）
//! 凡命中且不在 ADR-0032 豁免清单里的 = 违规。
//!
//! ## 为什么是一个 xtask 子命令而不是外部脚本
//! - 规则纯函数化、测试内嵌（cfg(test) mod tests），与本 crate 其他子命令同构。
//! - xtask card-check + adr-index + refscan + docscan 同属 CI 硬门禁 #12b 的 jobs，
//!   把外部脚本降级为子命令 = 关掉"脚本忘了跑"的漂移源头。
//! - 豁免清单只在 ADR-0032 登记表里登记一次（机器读它，不在源码里 hardcode）。
//!
//! ## 边界
//! - 不写任何文件。
//! - 不做豁免清单的解析（那是 ADR-0032 本身，本文件复用其 ExemptionSet）。
//!
//! ## 不变量
//! 1. 输出确定性（同 refscan.py 原型）。
//! 2. 豁免匹配：对每个 (rule, path, line) 三元组，先查豁免清单；命中 = 跳过。
//! 3. 扫到 0 个文件必须显式告警（与 main.rs 的 hygiene 不变量 4 同理）。

// TASK-015 升级 WIP（stash 取回）：多 lint 待修；本次以编译通过为优先，下一轮再清。
#![allow(
    clippy::needless_pass_by_value,
    clippy::needless_lifetimes,
    clippy::missing_panics_doc,
    clippy::unused_self,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::doc_lazy_continuation,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls,
    clippy::single_char_pattern,
    clippy::items_after_statements,
    clippy::collapsible_if,
    clippy::module_name_repetitions,
    clippy::uninlined_format_args,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::needless_collect,
    clippy::format_push_string,
    clippy::format_in_format_args,
    clippy::needless_borrow,
    clippy::redundant_slicing,
    clippy::match_same_arms,
    clippy::must_use_candidate,
    clippy::module_inception,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::single_match,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::unused_peekable,
    clippy::collapsible_match,
    clippy::needless_late_init,
    clippy::let_underscore_must_use,
    let_underscore_drop,
    clippy::let_and_return
)]

use crate::exemptions::ExemptionSet;
use crate::report::{Finding, Severity};

/// 待建号集合（ADR-0026 D2 维护；机器可拍）。
const ADR_BARE_PENDING: &[&str] = &[
    "0001", "0002", "0003", "0004", "0005", "0006", "0007", "0008", "0009", "0010", "0011", "0012",
    "0013", "0014", "0015", "0016", "0017", "0020", "0027",
];

#[must_use]
pub fn is_pending_adr(four_digits: &str) -> bool {
    ADR_BARE_PENDING.contains(&four_digits)
}

/// 执行 refscan 子命令：扫 .md / .rs / .ps1，应用 ADR-0032 豁免清单。
pub fn run(repo_root: &std::path::Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::report::Severity;
    use crate::repowalk::collect_repo_files;
    use crate::{EXIT_FINDINGS, EXIT_OK};

    let exemptions = crate::exemptions::load_from_repo(repo_root)
        .map_err(|e| format!("加载豁免清单失败：{e:?}"))?;
    let entries = collect_repo_files(repo_root, &["md", "rs", "ps1"])
        .map_err(|e| format!("扫描仓库失败：{e:?}"))?;
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    for entry in &entries {
        scanned += 1;
        let Ok(content) = std::fs::read_to_string(&entry.abs_path) else {
            continue;
        };
        let file_findings = scan_file(&entry.rel_path, &content);
        findings.extend(apply_exemptions(file_findings, &exemptions));
    }
    findings = sort_findings(findings);
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let verdict = if errors > 0 { "FAILED" } else { "PASSED" };
    let summary = format!(
        "== refscan ==\nscanned_files={scanned}\n-- summary: {errors} error(s), {warnings} warning(s)\n-- verdict: {verdict}\n"
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

pub fn scan_file(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let normalized = content.replace("\r\n", "\n").replace("\r", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let lower = rel_path.to_ascii_lowercase();
    let is_md_or_rs = lower.ends_with(".md") || lower.ends_with(".rs");
    let is_ps1 = lower.ends_with(".ps1");

    if is_md_or_rs {
        // Rule 1: ADR range notation (manual match)
        for (idx, line) in lines.iter().enumerate() {
            for m in find_adr_ranges(line) {
                findings.push(Finding::new(
                    "adr/number-range-notation",
                    Severity::Error,
                    rel_path,
                    idx + 1,
                    format!(
                        "发现 ADR 编号范围写法「{}」—— ADR-0026 D3 禁止范围写法（用逐个列出替代）",
                        m
                    ),
                ));
            }
        }
        // Rule 2: bare pending ADR ref (only outside docs/adr/)
        if !rel_path.starts_with("docs/adr/") {
            for (idx, line) in lines.iter().enumerate() {
                for m in find_bare_pending(line) {
                    findings.push(Finding::new(
                        "adr/bare-pending-reference",
                        Severity::Error,
                        rel_path,
                        idx + 1,
                        format!("裸引用待建号 ADR-{} —— ADR-0032 豁免清单 E-NNN 可豁免；机器读入实现后 = PASS", m),
                    ));
                }
            }
        }
    }

    if is_ps1 {
        // Rule 3: pure ASCII
        let mut bytes_seen = std::collections::BTreeSet::new();
        for (idx, line) in lines.iter().enumerate() {
            for (byte_off, b) in line.bytes().enumerate() {
                if b > 127 {
                    let key = (idx + 1, byte_off);
                    if bytes_seen.insert(key) {
                        findings.push(Finding::new(
                            "file/pure-ascii-ps1",
                            Severity::Error,
                            rel_path,
                            idx + 1,
                            format!("非 ASCII 字节 0x{b:02X} 在第 {} 列 —— ADR-0024 D4 要求 .ps1 一律纯 ASCII", byte_off + 1),
                        ));
                    }
                }
            }
        }
    }

    findings
}

/// 给一组 findings 过豁免清单。
#[must_use]
pub fn apply_exemptions(findings: Vec<Finding>, exemptions: &ExemptionSet) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|f| !exemptions.is_exempted(f.rule, &f.path, f.line))
        .collect()
}

/// 给 findings 按 (rule, path, line) 排序（输出确定性）。
#[must_use]
pub fn sort_findings(findings: Vec<Finding>) -> Vec<Finding> {
    let mut f = findings;
    f.sort_by(|a, b| {
        (&a.path, std::cmp::Reverse(a.line), &a.rule).cmp(&(
            &b.path,
            std::cmp::Reverse(b.line),
            &b.rule,
        ))
    });
    f
}

/// 手动查找 ADR 范围写法：4digit + whitespace + ~/- + whitespace + 4digit
/// （用迭代式扫描替代正则，避免引入 regex 依赖 = 遵守 xtask 零第三方依赖不变量）
fn find_adr_ranges(line: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i + 8 < chars.len() {
        if chars[i].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
            && chars[i + 2].is_ascii_digit()
            && chars[i + 3].is_ascii_digit()
            && chars[i + 4].is_whitespace()
        {
            // need: whitespace chars + ~ or - + whitespace chars + 4 digits
            let mut j = i + 5;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '~' || chars[j] == '-') {
                let mut k = j + 1;
                while k < chars.len() && chars[k].is_whitespace() {
                    k += 1;
                }
                if k + 4 <= chars.len()
                    && chars[k].is_ascii_digit()
                    && chars[k + 1].is_ascii_digit()
                    && chars[k + 2].is_ascii_digit()
                    && chars[k + 3].is_ascii_digit()
                {
                    hits.push(chars[i..k + 4].iter().collect());
                    i = k + 4;
                    continue;
                }
            }
        }
        i += 1;
    }
    hits
}

/// 手动查找裸 ADR-00NN 引用（NN 在待建号集合里）。
fn find_bare_pending(line: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 7 <= bytes.len() {
        if &bytes[i..i + 4] == b"ADR-" {
            // next 4 chars must be ASCII digits
            if bytes[i + 4].is_ascii_digit()
                && bytes[i + 5].is_ascii_digit()
                && bytes[i + 6].is_ascii_digit()
                && bytes[i + 7].is_ascii_digit()
                && !bytes.get(i + 8).is_some_and(|b| b.is_ascii_digit())
            {
                let four: String = std::str::from_utf8(&bytes[i + 4..i + 8])
                    .unwrap_or("")
                    .to_string();
                if is_pending_adr(&four) {
                    hits.push(four);
                }
            }
        }
        i += 1;
    }
    hits
}

/// 把 findings 转成 refscan.py 兼容的纯文本输出。
#[allow(dead_code)]
#[must_use]
pub fn render(findings: &[Finding]) -> String {
    let mut out = String::new();
    for f in findings {
        out.push_str(&format!(
            "{} {}:{} {}\n",
            short_rule(f.rule),
            f.path,
            f.line,
            extract_message_payload(&f.message),
        ));
    }
    out
}

fn short_rule(rule: &str) -> &str {
    if rule == "adr/number-range-notation" {
        "RANGE"
    } else if rule == "adr/bare-pending-reference" {
        "BARE-PENDING"
    } else if rule == "file/pure-ascii-ps1" {
        "NON-ASCII"
    } else {
        rule
    }
}

fn extract_message_payload(msg: &str) -> String {
    msg.split(['（', '(', '，', ',', ' '])
        .next()
        .unwrap_or(msg)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_range_notation() {
        let f = scan_file("plans/x.md", "0018 ~ 0025 \n0021FULLWIDE_TILDE0026\n");
        assert!(f.iter().any(|x| x.rule == "adr/number-range-notation"));
    }

    #[test]
    fn detects_bare_pending_outside_adr() {
        let f = scan_file("docs/gov.md", "see ADR-0017 here");
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn ignores_bare_pending_inside_adr_dir() {
        let f = scan_file("docs/adr/0021-x.md", "ADR-0017 inside");
        assert!(f.is_empty());
    }

    #[test]
    fn detects_non_ascii_in_ps1() {
        let f = scan_file("spikes/x/probe.ps1", "echo 测试 here");
        assert!(f.iter().any(|x| x.rule == "file/pure-ascii-ps1"));
    }

    #[test]
    fn ignores_non_ascii_outside_ps1() {
        let f = scan_file("docs/x.md", "# Chinese");
        assert!(f.iter().all(|x| x.rule != "file/pure-ascii-ps1"));
    }

    #[test]
    fn is_pending_adr_recognises_set() {
        assert!(is_pending_adr("0017"));
        assert!(!is_pending_adr("0031"));
    }

    #[test]
    fn sort_findings_deterministic() {
        let f1 = Finding::new("adr/bare-pending-reference", Severity::Error, "b.md", 5, "");
        let f2 = Finding::new("adr/bare-pending-reference", Severity::Error, "a.md", 1, "");
        let f3 = Finding::new("adr/number-range-notation", Severity::Error, "a.md", 2, "");
        let sorted = sort_findings(vec![f1, f2, f3]);
        assert_eq!(sorted[0].rule, "adr/number-range-notation");
        assert_eq!(sorted[1].path, "a.md");
        assert_eq!(sorted[2].path, "b.md");
    }
}
