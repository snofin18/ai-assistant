//! # refscan 子命令（ADR-0030 D3 的扩展：裸 ADR 待建引用 / 范围写法 / ps1 纯 ASCII）
//!
//! 职责：扫全仓 .md + .rs 找出
//! 1. ADR 编号范围写法（两个 4 位号之间夹 ~ / `FULLWIDE_TILDE` / -）
//! 2. 裸 ADR 待建引用（docs/adr/ 之外的 ADR-00NN 引用、且 NN 属待建号集合）
//! 3. .ps1 文件里所有 > 127 的字节（= 非 ASCII）
//!
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
//! - 不做豁免清单的解析（那是 ADR-0032 本身，本文件复用其 `ExemptionSet`）。
//!
//! ## 不变量
//! 1. 输出确定性（同 refscan.py 原型）。
//! 2. 豁免匹配：对每个 (rule, path, line) 三元组，先查豁免清单；命中 = 跳过。
//! 3. 扫到 0 个文件必须显式告警（与 main.rs 的 hygiene 不变量 4 同理）。

// TASK-052 (2026-09-19) 决策：本文件**无任何**模块级 `#![allow(...)]` 块；代码遵守 workspace `[lints.clippy]` 全部 deny 规则。
// 历史上 cherry-pick 10f78db 留下的 40-lint `#![allow(...)]` 块（解释本应）已在本卡全部清掉，
// 32 处 indexing_slicing 全替换为 safe pattern。详见 tasks/TASK-052-...md §5 + commit 收尾。

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
    // 逐 finding 输出（铁律 ①「无静默失败」：仅 counts = 「看着像在跑」的伪完成）
    if !findings.is_empty() {
        output
            .write_all(render(&findings).map_err(|e| e.to_string())?.as_bytes())
            .map_err(|e| e.to_string())?;
    }

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
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    // 用 Path::extension() + is_some_and(eq_ignore_ascii_case) 替代 lower.ends_with，
    // 让 clippy 能看到「已在比较前规范化」——消除 2 处 case_sensitive_file_extension_comparisons allow
    // （TASK-055 落地；语义不变：MD/md/Md 都算 .md；非 UTF-8 扩展名按原本 ends_with 失败的行为同样返回 false）
    let ext_is = |expected: &str| -> bool {
        std::path::Path::new(rel_path)
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(expected))
    };
    let is_md_or_rs = ext_is("md") || ext_is("rs");
    let is_ps1 = ext_is("ps1");

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
                        "发现 ADR 编号范围写法「{m}」—— ADR-0026 D3 禁止范围写法（用逐个列出替代）"
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
                        format!("裸引用待建号 ADR-{m} —— ADR-0032 豁免清单 E-NNN 可豁免；机器读入实现后 = PASS"),
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
    while let Some(c0) = chars.get(i) {
        if !c0.is_ascii_digit() {
            i += 1;
            continue;
        }
        let Some(c1) = chars.get(i + 1) else { break };
        if !c1.is_ascii_digit() {
            i += 1;
            continue;
        }
        let Some(c2) = chars.get(i + 2) else { break };
        if !c2.is_ascii_digit() {
            i += 1;
            continue;
        }
        let Some(c3) = chars.get(i + 3) else { break };
        if !c3.is_ascii_digit() {
            i += 1;
            continue;
        }
        let Some(c4) = chars.get(i + 4) else { break };
        if !c4.is_whitespace() {
            i += 1;
            continue;
        }
        let mut j = i + 5;
        while let Some(cj) = chars.get(j) {
            if !cj.is_whitespace() {
                break;
            }
            j += 1;
        }
        let Some(sep) = chars.get(j) else { break };
        if *sep != '~' && *sep != '-' {
            i += 1;
            continue;
        }
        let mut k = j + 1;
        while let Some(ck) = chars.get(k) {
            if !ck.is_whitespace() {
                break;
            }
            k += 1;
        }
        let Some(k0) = chars.get(k) else { break };
        let Some(k1) = chars.get(k + 1) else { break };
        let Some(k2) = chars.get(k + 2) else { break };
        let Some(k3) = chars.get(k + 3) else { break };
        if k0.is_ascii_digit() && k1.is_ascii_digit() && k2.is_ascii_digit() && k3.is_ascii_digit()
        {
            let end = k + 4;
            let Some(slice) = chars.get(i..end) else {
                continue;
            };
            let s: String = slice.iter().collect();
            hits.push(s);
            i = end;
        } else {
            i += 1;
        }
    }
    hits
}

/// 手动查找裸 ADR-00NN 引用（NN 在待建号集合里）。
fn find_bare_pending(line: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 7 <= bytes.len() {
        if !matches!(bytes.get(i..i + 4), Some(b"ADR-")) {
            i += 1;
            continue;
        }
        if !bytes.get(i + 4).is_some_and(u8::is_ascii_digit)
            || !bytes.get(i + 5).is_some_and(u8::is_ascii_digit)
            || !bytes.get(i + 6).is_some_and(u8::is_ascii_digit)
            || !bytes.get(i + 7).is_some_and(u8::is_ascii_digit)
        {
            i += 1;
            continue;
        }
        if bytes.get(i + 8).is_some_and(u8::is_ascii_digit) {
            i += 1;
            continue;
        }
        let Some(four_bytes) = bytes.get(i + 4..i + 8) else {
            i += 1;
            continue;
        };
        let four = std::str::from_utf8(four_bytes).unwrap_or("").to_string();
        if is_pending_adr(&four) {
            hits.push(four);
        }
        i += 8;
    }
    hits
}

/// 把 findings 转成 refscan.py 兼容的纯文本输出。
///
/// 返回 `Result` 取代之前的 `String`：writeln! 到 String 仅在 OOM 时失败（进程级崩溃
/// 由 OS 处理），错误类型 `std::fmt::Error` 直接上抛到 `run()` 的 `Result<u8, String>`。
/// 本卡（TASK-054）消除了 TASK-052 留下的 per-line `#[allow(clippy::unwrap_used, ...)]`。
pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error> {
    use std::fmt::Write as _;
    let mut out = String::new();
    for f in findings {
        writeln!(
            out,
            "{} {}:{} {}",
            short_rule(f.rule),
            f.path,
            f.line,
            extract_message_payload(&f.message),
        )?;
    }
    Ok(out)
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;

    #[test]
    fn detects_range_notation() {
        let f = scan_file("plans/x.md", "0018 ~ 0025 \n0021`FULLWIDE_TILDE`0026\n");
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
        assert_eq!(
            sorted.first().expect("non-empty").rule,
            "adr/number-range-notation"
        );
        assert_eq!(sorted.get(1).expect("non-empty").path, "a.md");
        assert_eq!(sorted.get(2).expect("non-empty").path, "b.md");
    }
}
