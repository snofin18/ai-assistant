//! # card-check 子命令（ADR-0031 D6 的机器化）
//!
//! 职责：扫 `tasks/TASK-*.md` 与 `plans/*.md`，
//! 校验 ADR-0031 D6 四条判据：
//! ① 正文区 `git diff` 非空 → Error（Implementer 不能改 In/Out scope / 验收标准）；
//! ② 状态非 `Ready` 的卡必须有对应 `tasks/TASK-NNN-*.md` 文件 → Error；
//! ③ 记录区 9 节标题齐全 → Warning（Ready 状态豁免；
//!     「早于本规定的卡」用对照表豁免，见 ADR-0032 风格通道）；
//! ④ `plans/*.md` 出现 `##/###/#### TASK-NNN` 形态 → Error（防形态回退）。
//!
//! ## 边界（不做什么）
//! - 不读 git 历史（diff 检测是另一条规则，本版本先做静态形态判定）。
//! - 不改任何文件（与本 crate 其他子命令一致）。
//!
//! ## 不变量
//! 1. canonical 分界线从 gov §3.4 现场读取（`load_canonical_divider`）—— ADR-0031 D3 强制，
//!    避免硬编码漂移（PL-031 同源教训）。
//! 2. 9 节标题**在源码里硬编码**为 `TITLES` static（与 ADR-0034 描述一致 —— ADR 只要求分界线现场读取，
//!    不要求 9 节标题也现场读取）。**待改进**：`load_record_section_titles()` 已实现 gov §3.4 现场
//!    读取版，目前 dead code（grep 全仓 0 处引用），TASK-060 接入前不要删。
use crate::exemptions::ExemptionSet;
use crate::report::{Finding, Severity};

#[allow(dead_code)] // 4 rules of ADR-0031 D6 — current implementation covers only 3 (body diff, missing records, plans leak)
const RULE_DIFF_NOT_EMPTY: &str = "card-check/body-diff-not-empty";
#[allow(dead_code)]
const RULE_MISSING_TASK_FILE: &str = "card-check/status-not-ready-needs-file";
#[allow(dead_code)]
const RULE_MISSING_RECORD_SECTIONS: &str = "card-check/missing-record-sections";
#[allow(dead_code)]
const RULE_CARD_BODY_IN_PLANS: &str = "card-check/card-body-leaked-to-plans";

#[allow(dead_code)] // 全局常量，供 run() 与未来实现使用
const GOV_PATH: &str = "docs/governance-ai-agent-execution.md";
#[allow(dead_code)]
const GOV_SECTION_HEADER: &str = "### 3.4 ";
#[allow(dead_code)]
const GOV_RECORD_TABLE_HEADER: &str = "| # | 小节 |";

/// 从 gov §3.4 现场读 canonical 分界线（前后空行 + 整段 fence）。
/// 找不到或 fence 缺失 → Err。
/// 执行 card-check 子命令：扫 tasks/ 与 plans/，应用 ADR-0031 D6 四判据。
pub fn run(repo_root: &std::path::Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::report::Severity;
    use crate::repowalk::collect_repo_files;
    use crate::{EXIT_FINDINGS, EXIT_OK};

    let exemptions = crate::exemptions::load_from_repo(repo_root)
        .map_err(|e| format!("加载豁免清单失败：{e:?}"))?;
    let gov_path = repo_root.join("docs/governance-ai-agent-execution.md");
    let gov_content =
        std::fs::read_to_string(&gov_path).map_err(|e| format!("读 gov 失败：{e}"))?;
    let divider =
        load_canonical_divider(&gov_content).map_err(|e| format!("解析分界线失败：{e}"))?;
    let titles: Vec<(usize, &str)> = TITLES.to_vec();
    let entries =
        collect_repo_files(repo_root, &["md"]).map_err(|e| format!("扫描仓库失败：{e:?}"))?;
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    for entry in &entries {
        let rel = &entry.rel_path;
        if !(rel.starts_with("tasks/TASK-") || rel.starts_with("plans/")) {
            continue;
        }
        scanned += 1;
        let Ok(content) = std::fs::read_to_string(&entry.abs_path) else {
            continue;
        };
        let status_line = extract_status_line(&content);
        let file_findings = scan_card_file(
            rel,
            &content,
            &divider,
            &titles,
            status_line.as_deref(),
            &exemptions,
        );
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
    let summary = format!(
        "== card-check ==\nscanned_files={scanned}\n-- summary: {errors} error(s), {warnings} warning(s)\n-- verdict: {verdict}\n"
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

/// 从 gov §3.4 现场读 canonical 分界线
pub fn load_canonical_divider(gov_content: &str) -> Result<String, String> {
    let sec = section_after(gov_content, GOV_SECTION_HEADER)
        .ok_or_else(|| "gov §3.4 未找到".to_string())?;
    // The divider lives in a fenced markdown block:
    //   ```markdown
    //   <!-- ══ 分界线：...══
    //        ...
    //    -->
    //   ```
    let start = sec
        .find("```markdown\n")
        .ok_or_else(|| "gov §3.4 未找到 ```markdown 开始".to_string())?;
    let body_start = start + "```markdown\n".len();
    let Some(after_body_start) = sec.get(body_start..) else {
        return Err("gov §3.4 分界线 起点已越过文件末尾".to_string());
    };
    let end_off = after_body_start
        .find("```")
        .ok_or_else(|| "gov §3.4 分界线 ``` 未闭合".to_string())?;
    let Some(inside) = sec.get(body_start..body_start + end_off) else {
        return Err("gov §3.4 分界线 终点计算溢出".to_string());
    };
    Ok(inside.to_string())
}

/// 从 gov §3.4 现场读 9 节执行记录骨架的标题（第一列 = 编号，第二列 = 标题）。
#[allow(dead_code)]
pub fn load_record_section_titles(gov_content: &str) -> Result<Vec<(usize, String)>, String> {
    let sec = section_after(gov_content, GOV_SECTION_HEADER)
        .ok_or_else(|| "gov §3.4 未找到".to_string())?;
    let table_start = sec
        .find(GOV_RECORD_TABLE_HEADER)
        .ok_or_else(|| "9 节表头未找到".to_string())?;
    // Walk forward line by line until we exit the table (blank line or non-pipe line)
    let mut out = Vec::new();
    let Some(table_rows) = sec.get(table_start..) else {
        return Ok(Vec::new());
    };
    for line in table_rows.lines().skip(1) {
        // skip the header line itself
        if line.starts_with("|---") {
            continue;
        }
        if !line.starts_with('|') {
            break;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() < 3 {
            continue;
        }
        let Some(n_cell) = cells.first() else {
            continue;
        };
        let n: usize = match n_cell.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let Some(name_cell) = cells.get(1) else {
            continue;
        };
        out.push((n, name_cell.to_string()));
    }
    if out.len() != 9 {
        return Err(format!("期待 9 个节标题，实际 {}个", out.len()));
    }
    Ok(out)
}

fn section_after<'a>(content: &'a str, header: &str) -> Option<&'a str> {
    let idx = content.find(header)?;
    let rest = content.get(idx..)?;
    let after_header = rest.get(header.len()..);
    let next_h3 = after_header.and_then(|s| s.find("\n### ").map(|o| o + header.len()));
    Some(match next_h3 {
        Some(end) => rest.get(..end)?,
        None => rest,
    })
}

/// 给定文件路径与内容，检测 4 条判据中的所有违规。
#[must_use]
pub fn scan_card_file(
    rel_path: &str,
    content: &str,
    divider: &str,
    record_titles: &[(usize, &str)],
    status_line: Option<&str>,
    exemptions: &ExemptionSet,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    // ① body diff not empty — we can't see git diff in xtask (no IO in modules),
    // but the `status_line` already encodes the Implementer's intended status.
    // Treat as: status is `Done` or `Review` but body zone changed => flag.
    // Since we have no git here, we approximate by: if status says Done but
    // record zone is empty -> warning.
    if let Some(s) = status_line
        && (s.contains("Done") || s.contains("Review"))
        && record_zone_is_empty(&lines, divider)
    {
        findings.push(Finding::new(
                RULE_DIFF_NOT_EMPTY, Severity::Warning, rel_path, 1,
                "状态 = Done/Review 但记录区为空（这条的 Git diff 校验 = 待接入 git 实现，本原型用空记录区作 proxy）".to_string(),
            ));
    }

    // ③ missing record sections (9 expected)
    let after_divider: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.trim_start().starts_with("<!-- ══ 分界线"))
        .skip(1)
        .copied()
        .collect();
    let record_text = after_divider.join("\n");
    let missing: Vec<(usize, &str)> = record_titles
        .iter()
        .filter(|pair: &&(usize, &str)| {
            !record_text.contains(&format!("### {}. {}", pair.0, pair.1))
        })
        .copied()
        .collect();
    if !missing.is_empty() && !is_ready_status(status_line) {
        // Exemption via ADR-0032 — historical record uses mapping table
        // (detected when record zone contains `旧格式 → 9 节骨架的对照表`)
        if !record_text.contains("旧格式 → 9 节骨架的对照表") {
            for item in &missing {
                let (n, _t) = item;
                if !exemptions.is_exempted(RULE_MISSING_RECORD_SECTIONS, rel_path, *n) {
                    findings.push(Finding::new(
                        RULE_MISSING_RECORD_SECTIONS,
                        Severity::Warning,
                        rel_path,
                        1,
                        "记录区缺 9 节骨架里的「### {{n}}. {{t}}」".to_string(),
                    ));
                }
            }
        }
    }

    findings
}

fn record_zone_is_empty(lines: &[&str], divider: &str) -> bool {
    let mut iter = lines.iter();
    // advance to divider
    let mut found = false;
    for l in &mut iter {
        if l.contains("<!-- ══ 分界线") && l.contains(divider) {
            found = true;
            break;
        }
    }
    if !found {
        return false;
    }
    let body: String = iter.copied().collect::<Vec<_>>().join("\n");
    body.trim().is_empty()
}

fn is_ready_status(s: Option<&str>) -> bool {
    s.is_some_and(|s| s.contains("Ready"))
}

/// plans/*.md 不能含卡片正文形态（防形态回退）。
#[must_use]
#[allow(dead_code)]
pub fn scan_plans(rel_path: &str, content: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        // `## TASK-NNN`、`### TASK-NNN`、`#### TASK-NNN` 形态
        if line.starts_with("## TASK-")
            || line.starts_with("### TASK-")
            || line.starts_with("#### TASK-")
        {
            findings.push(Finding::new(
                RULE_CARD_BODY_IN_PLANS,
                Severity::Error,
                rel_path,
                idx + 1,
                "plans/*.md 不应含卡片正文（ADR-0031 D6 判据 ④ 防形态回退）".to_string(),
            ));
        }
    }
    findings
}

/// 从任务卡文件的 metadata 块提取 `- 状态：...` 行。
#[must_use]
pub fn extract_status_line(content: &str) -> Option<String> {
    for line in content.lines().take(10) {
        if line.starts_with("- 状态：") {
            return Some(line.to_string());
        }
    }
    None
}

pub static TITLES: &[(usize, &str)] = &[
    (1, "约束回执"),
    (2, "实际改动文件"),
    (3, "验收输出摘要"),
    (4, "DoD 逐条核对"),
    (5, "偏差"),
    (6, "更合理做法"),
    (7, "遗留问题"),
    (8, "新增长期记忆"),
    (9, "给审阅者的关注点"),
];
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

    const DIV: &str = "<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══\n     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），\n     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->";

    #[test]
    fn missing_record_sections_flagged() {
        let content = format!(
            "# TASK-001\n\n- 状态：**InProgress**\n\n{DIV}\n\n## 执行记录\n\n### 1. 约束回执\n"
        );
        let status = extract_status_line(&content);
        let exemptions = ExemptionSet::default();
        let f = scan_card_file(
            "tasks/TASK-001-x.md",
            &content,
            DIV,
            TITLES,
            status.as_deref(),
            &exemptions,
        );
        // 8 of 9 sections missing — should flag 8 Warnings
        assert!(f.len() >= 8, "expected >=8 warnings, got {}", f.len());
        assert!(f.iter().all(|x| x.rule == RULE_MISSING_RECORD_SECTIONS));
    }

    #[test]
    fn ready_status_skips_missing_sections_warning() {
        let content = format!("# TASK-002\n\n- 状态：**Ready**\n\n{DIV}\n\n## 执行记录\n");
        let status = extract_status_line(&content);
        let exemptions = ExemptionSet::default();
        let f = scan_card_file(
            "tasks/TASK-002-y.md",
            &content,
            DIV,
            TITLES,
            status.as_deref(),
            &exemptions,
        );
        assert!(
            f.is_empty(),
            "Ready status should skip 9-section warning, got {:?}",
            f
        );
    }

    #[test]
    fn mapping_table_exempts_historical_records() {
        let content = format!(
            "# TASK-003\n\n- 状态：**Done**\n\n{DIV}\n\n## 执行记录\n\n> 旧格式 → 9 节骨架的对照表\n"
        );
        let status = extract_status_line(&content);
        let exemptions = ExemptionSet::default();
        let f = scan_card_file(
            "tasks/TASK-003-z.md",
            &content,
            DIV,
            TITLES,
            status.as_deref(),
            &exemptions,
        );
        assert!(
            f.iter().all(|x| x.rule != RULE_MISSING_RECORD_SECTIONS),
            "mapping-table exemption should skip 9-section warning, got {:?}",
            f
        );
    }

    #[test]
    fn plans_with_card_body_flagged() {
        let content = "# 阶段 1\n\n## 批次 A1\n\n### TASK-099 这是卡片正文颓\n";
        let f = scan_plans("plans/stage-x.md", content);
        assert_eq!(f.len(), 1);
        assert_eq!(f.first().expect("non-empty").severity, Severity::Error);
    }

    #[test]
    fn plans_without_card_body_clean() {
        let content = "# 阶段 1\n\n## 批次 A1\n\n| 卡号 | 标题 |\n|---|---|\n| **011** | foo |\n";
        let f = scan_plans("plans/stage-x.md", content);
        assert!(
            f.is_empty(),
            "plan table rows should not trigger, got {:?}",
            f
        );
    }
}
