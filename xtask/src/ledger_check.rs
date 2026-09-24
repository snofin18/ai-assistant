//! # check-ledger 子命令（ADR-0039 D3 的两条「新鲜度」规则）
//!
//! 职责：校验 `PLAN.md` / `README.md` 是否**跟着台账走**：
//! ① `PLAN.md`「当前状态」块的「更新日期」**不得早于** `LEDGER.md` **最后一条记录的日期**；
//! ② `README.md` 必须存在一行以 `> 状态：` 开头的状态行，且其中出现当前阶段名（`阶段 N`）。
//!
//! ## 为什么只查「新鲜度」而不查「真话」
//! ADR-0039 D3 原话：人写不出「`PLAN.md` 说的是不是真话」的机器判据，但**新鲜度**可以。
//! 规则 ① 恰好抓住 **DRIFT-202-2** 的形态（LEDGER 有 09-24 的行，PLAN 停在 09-23）；
//! 规则 ② 抓住「状态行被删 / 被清空」。内容真实性仍靠 review —— 本模块**不假装**机器能读语义。
//!
//! ## 为什么级别是 Error（不是 ADR-0025 D1 默认的 Warning）
//! ADR-0039 D3 第 3 条：「先 Warning、清扫后升 Error；**若上线即全绿则可直接 Error**」。
//! 2026-09-24 上线时两条规则在本仓库**都是绿的**（`PLAN.md` 更新日期 = `LEDGER.md` 末行日期 = 2026-09-24；
//! `README.md` 有 `> 状态：**阶段 1（…）**`）→ 无存量可清扫，直接 Error。
//!
//! ## 边界（不做什么）
//! - **不自动写回**：ADR-0030 选项 1 已否决「工具替你改文档」（写回会让「谁在什么时候声称了什么」
//!   失去作者署名）。本子命令只出红灯。
//! - 不判断 `PLAN.md` 的正文是否被越界改动（那是 review 的活）。
//! - 不读 git 历史，只看工作区当前内容。
//!
//! ## 不变量
//! 1. **无静默失败**（铁律 1）：读不到 `PLAN.md` / `LEDGER.md` / `README.md`，或解析不出
//!    「更新日期 / LEDGER 末行日期 / 当前阶段名」→ 必须 `Err`（退出码 4），
//!    **不得**当成「没有发现项 → PASSED」。这正是 ADR-0030 立下的口径。
//! 2. 输出确定性：同一仓库状态两次运行输出逐字节相同。
//!
//! 相关：`docs/adr/0039-task-end-state-sync-contract.md` D3、`AGENTS.md` §11.2 第 6 步、
//! `docs/governance-ai-agent-execution.md` §5.1 #16。

use std::path::Path;

use crate::report::{Finding, Severity};

/// 规则 `ledger/plan-date-stale`：`PLAN.md` 的「更新日期」早于 `LEDGER.md` 末行日期。
const RULE_PLAN_DATE_STALE: &str = "ledger/plan-date-stale";
/// 规则 `ledger/readme-status-line`：`README.md` 缺 `> 状态：` 状态行，或其中没有当前阶段名。
const RULE_README_STATUS_LINE: &str = "ledger/readme-status-line";

/// 执行 `check-ledger`：读三份文件 → 跑两条规则 → 渲染。
///
/// 错误语义：任一必需文件读不到、或任一字段解析不出 → `Err`（映射成退出码 4，铁律 1）。
pub fn run(repo_root: &Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::{EXIT_FINDINGS, EXIT_OK};

    let plan = read_required(repo_root, "PLAN.md")?;
    let ledger = read_required(repo_root, "LEDGER.md")?;
    let readme = read_required(repo_root, "README.md")?;

    let (plan_date_line, plan_date) = parse_plan_update_date(&plan).ok_or_else(|| {
        "PLAN.md 里找不到「更新日期」+ `YYYY-MM-DD`（ADR-0039 D1 要求它在「当前状态」块里）"
            .to_string()
    })?;
    let (ledger_line, ledger_date) = parse_ledger_last_date(&ledger).ok_or_else(|| {
        "LEDGER.md 里找不到任何以 ISO 日期开头的表格行（末行日期无法判定）".to_string()
    })?;
    let stage = parse_current_stage(&plan).ok_or_else(|| {
        "PLAN.md 里找不到「当前阶段」+ `阶段 N`（README 状态行没有可对照的阶段名）".to_string()
    })?;

    let mut findings =
        check_plan_date_freshness(plan_date_line, &plan_date, ledger_line, &ledger_date);
    findings.extend(check_readme_status_line(&readme, &stage));

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let verdict = if errors > 0 { "FAILED" } else { "PASSED" };
    for finding in &findings {
        finding.render(output).map_err(|e| e.to_string())?;
    }
    let summary = format!(
        "== check-ledger ==\nplan_date={plan_date} (PLAN.md:{plan_date_line})\nledger_last_date={ledger_date} (LEDGER.md:{ledger_line})\ncurrent_stage={stage}\n-- summary: {errors} error(s), {warnings} warning(s)\n-- verdict: {verdict}\n"
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

/// 读一份必需文件；读不到 → `Err`（不变量 1：不得静默当成空内容）。
fn read_required(repo_root: &Path, relative: &str) -> Result<String, String> {
    let path = repo_root.join(relative);
    std::fs::read_to_string(&path).map_err(|error| format!("读 {relative} 失败：{error}"))
}

/// 规则 ①：`PLAN.md` 的更新日期 **不得早于** `LEDGER.md` 末行日期。
///
/// 日期是 `YYYY-MM-DD` 定长格式 → 字典序即时间序，直接比较字符串。
#[must_use]
pub fn check_plan_date_freshness(
    plan_line: usize,
    plan_date: &str,
    ledger_line: usize,
    ledger_date: &str,
) -> Vec<Finding> {
    if plan_date >= ledger_date {
        return Vec::new();
    }
    vec![Finding::new(
        RULE_PLAN_DATE_STALE,
        Severity::Error,
        "PLAN.md",
        plan_line,
        format!(
            "「更新日期」{plan_date} 早于 `LEDGER.md` 末行（第 {ledger_line} 行）的 {ledger_date} —— 卡 Done 时必须同批更新 PLAN.md（ADR-0039 D1/D2：阶段没变也要改日期）"
        ),
    )]
}

/// 规则 ②：`README.md` 必须有 `> 状态：` 状态行，且其中出现当前阶段名（`阶段 N`）。
#[must_use]
pub fn check_readme_status_line(readme: &str, stage: &str) -> Vec<Finding> {
    // `stage` 形如 `阶段 1`；README 里允许写成 `阶段 1` / `阶段1`（空格可省）。
    let compact_stage = stage.replace(' ', "");
    let status_line = readme.lines().enumerate().find(|(_, line)| {
        let trimmed = line.trim_start();
        trimmed.starts_with("> 状态：") || trimmed.starts_with("> 状态:")
    });
    let Some((index, line)) = status_line else {
        return vec![Finding::new(
            RULE_README_STATUS_LINE,
            Severity::Error,
            "README.md",
            0,
            format!(
                "找不到以 `> 状态：` 开头的状态行（ADR-0039 D3 规则 ② 要求它存在并写出当前阶段 `{stage}`）"
            ),
        )];
    };
    if line.contains(stage) || line.replace(' ', "").contains(&compact_stage) {
        return Vec::new();
    }
    vec![Finding::new(
        RULE_README_STATUS_LINE,
        Severity::Error,
        "README.md",
        index + 1,
        format!("状态行里没有当前阶段名 `{stage}`（`PLAN.md` 的「当前阶段」说的就是它）"),
    )]
}

/// 从 `PLAN.md` 取「更新日期」的 `YYYY-MM-DD`，返回（1 基行号, 日期）。
#[must_use]
pub fn parse_plan_update_date(plan: &str) -> Option<(usize, String)> {
    for (index, line) in plan.lines().enumerate() {
        if !line.contains("更新日期") {
            continue;
        }
        if let Some(date) = extract_iso_date(line) {
            return Some((index + 1, date));
        }
    }
    None
}

/// 从 `LEDGER.md` 取**最后一条**以 ISO 日期开头的表格行，返回（1 基行号, 日期）。
///
/// 只认「第一格恰好是 `YYYY-MM-DD`」的行：表头 `| 日期 | 提出者 |` 与说明段落自然被跳过。
#[must_use]
pub fn parse_ledger_last_date(ledger: &str) -> Option<(usize, String)> {
    let mut last: Option<(usize, String)> = None;
    for (index, line) in ledger.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('|') {
            continue;
        }
        let Some(cell) = trimmed.trim_start_matches('|').split('|').next() else {
            continue;
        };
        let cell = cell.trim();
        // 只认「整格恰好是 ISO 日期」的行：`| 2026-09-24 | ... |` ✓，`| 日期 | ... |` ✗
        if extract_iso_date(cell).is_some_and(|date| date == cell) {
            last = Some((index + 1, cell.to_string()));
        }
    }
    last
}

/// 从 `PLAN.md` 取当前阶段名（规范化为 `阶段 N`，`N` 为十进制数字串）。
#[must_use]
pub fn parse_current_stage(plan: &str) -> Option<String> {
    for line in plan.lines() {
        if !line.contains("当前阶段") {
            continue;
        }
        if let Some(number) = extract_stage_number(line) {
            return Some(format!("阶段 {number}"));
        }
    }
    None
}

/// 取 `阶段` 之后的连续数字串（`当前阶段    ：**阶段 1（三试点闭环）**` → `1`）。
///
/// 注意必须**先定位 `当前阶段` 再找它后面的 `阶段`**：`当前阶段` 这四个字里本身就含
/// `阶段`，直接 `find("阶段")` 会命中它自己（2026-09-24 首跑就踩了这个，报「找不到当前阶段」）。
fn extract_stage_number(text: &str) -> Option<String> {
    let marker = text.find("当前阶段")?;
    let after_marker = text.get(marker + "当前阶段".len()..)?;
    let stage_at = after_marker.find("阶段")?;
    let rest = after_marker.get(stage_at + "阶段".len()..)?;
    let digits: String = rest
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

/// 取文本里第一处 `YYYY-MM-DD`（定长 10 字符，数字段与连字符位置都必须是数字/`-`）。
fn extract_iso_date(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let is_digit = |offset: usize| bytes.get(offset).is_some_and(u8::is_ascii_digit);
    for start in 0..bytes.len() {
        if start + 10 > bytes.len() {
            break;
        }
        if !(is_digit(start) && is_digit(start + 1) && is_digit(start + 2) && is_digit(start + 3)) {
            continue;
        }
        if bytes.get(start + 4) != Some(&b'-') {
            continue;
        }
        if !(is_digit(start + 5) && is_digit(start + 6)) {
            continue;
        }
        if bytes.get(start + 7) != Some(&b'-') {
            continue;
        }
        if !(is_digit(start + 8) && is_digit(start + 9)) {
            continue;
        }
        return text.get(start..start + 10).map(str::to_string);
    }
    None
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

    const PLAN_OK: &str = "# PLAN.md\n\n## 当前状态\n\n```text\n更新日期    ：2026-09-24（TASK-014 收尾）\n当前阶段    ：**阶段 1（三试点闭环）**\n```\n";
    const LEDGER_OK: &str = "# LEDGER\n\n| 日期 | 提出者 |\n|---|---|\n| 2026-09-23 | TASK-011 |\n| 2026-09-24 | TASK-014 |\n";
    const README_OK: &str = "# 项目\n\n> 状态：**阶段 1（三试点闭环）** —— 进行中\n";

    #[test]
    fn parses_plan_update_date() {
        let (line, date) = parse_plan_update_date(PLAN_OK).expect("应解析出更新日期");
        assert_eq!(date, "2026-09-24");
        assert_eq!(line, 6);
    }

    #[test]
    fn parses_ledger_last_date_as_the_final_row() {
        let (line, date) = parse_ledger_last_date(LEDGER_OK).expect("应解析出末行日期");
        assert_eq!(date, "2026-09-24");
        assert_eq!(line, 6);
    }

    #[test]
    fn parses_current_stage_from_bold_text() {
        assert_eq!(parse_current_stage(PLAN_OK).as_deref(), Some("阶段 1"));
    }

    #[test]
    fn fresh_plan_date_produces_nothing() {
        let findings = check_plan_date_freshness(6, "2026-09-24", 6, "2026-09-24");
        assert!(findings.is_empty(), "同一天算新鲜：{findings:?}");
    }

    #[test]
    fn stale_plan_date_is_an_error() {
        let findings = check_plan_date_freshness(6, "2026-09-23", 6, "2026-09-24");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, RULE_PLAN_DATE_STALE);
        assert_eq!(findings[0].severity, Severity::Error);
        assert_eq!(findings[0].line, 6);
        assert!(findings[0].message.contains("2026-09-23"));
        assert!(findings[0].message.contains("2026-09-24"));
    }

    #[test]
    fn readme_with_matching_stage_produces_nothing() {
        let findings = check_readme_status_line(README_OK, "阶段 1");
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn readme_without_status_line_is_an_error() {
        let findings = check_readme_status_line("# 项目\n\n没有任何状态行\n", "阶段 1");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, RULE_README_STATUS_LINE);
        assert_eq!(findings[0].severity, Severity::Error);
        assert_eq!(findings[0].line, 0, "仓库级发现项的行号约定是 0");
    }

    #[test]
    fn readme_status_line_with_wrong_stage_is_an_error() {
        let findings = check_readme_status_line("> 状态：**阶段 0（文档与 Spike）**\n", "阶段 1");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
        assert!(findings[0].message.contains("阶段 1"));
    }

    #[test]
    fn readme_status_line_accepts_halfwidth_colon() {
        let findings = check_readme_status_line("> 状态: **阶段 1**\n", "阶段 1");
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn unparsable_plan_returns_none_instead_of_guessing() {
        // 铁律 1 的正向对照：解析不到必须 None（由 run() 转成 Err），不得猜一个日期
        assert!(parse_plan_update_date("# PLAN\n没有那一行\n").is_none());
        assert!(parse_plan_update_date("更新日期    ：待定\n").is_none());
    }

    #[test]
    fn unparsable_ledger_returns_none() {
        assert!(parse_ledger_last_date("| 日期 | 提出者 |\n|---|---|\n").is_none());
        assert!(parse_ledger_last_date("没有任何表格\n").is_none());
    }

    #[test]
    fn stage_number_is_taken_after_the_current_stage_marker() {
        // 回归用例：`当前阶段` 自身含 `阶段` 二字，实现必须先跳过它
        assert_eq!(
            parse_current_stage("当前阶段    ：**阶段 1（三试点闭环）** —— stage-0\n").as_deref(),
            Some("阶段 1")
        );
        assert_eq!(
            parse_current_stage("当前阶段：阶段 12\n").as_deref(),
            Some("阶段 12")
        );
    }

    #[test]
    fn unparsable_stage_returns_none() {
        assert!(parse_current_stage("当前阶段    ：待定\n").is_none());
        assert!(parse_current_stage("# PLAN\n").is_none());
    }

    #[test]
    fn iso_date_extraction_rejects_partial_matches() {
        assert_eq!(
            extract_iso_date("2026-09-24"),
            Some("2026-09-24".to_string())
        );
        assert_eq!(extract_iso_date("截止 2026-9-4"), None, "月份/日必须两位");
        assert_eq!(extract_iso_date("20260924"), None);
        assert_eq!(extract_iso_date("2026-09-2x"), None);
    }
}
