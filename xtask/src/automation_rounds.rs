//! # automation-rounds 子命令
//!
//! 职责：
//! - 从 `docs/automations/<run-date>-round-<N>.md` 解析每个轮次的机制；
//! - 找到最近一次 `机制=human`，统计其后的 `机制=automation` 数量；
//! - 计算指定运行日下一可用轮次号。
//!
//! 边界：
//! - 不读取 git 历史，只读取当前仓库内容；
//! - 不修改轮次文件或台账；
//! - 不判断任务内容质量，只处理 ADR-0089 的轮次计数事实。
//!
//! 不变量：
//! 1. 缺机制字段按 automation fail-closed，不能静默跳过；
//! 2. 输出只依赖文件名和机制声明，同一仓库状态必得同一结果；
//! 3. 没有 human 轮次时绝不伪造 reset。
//!
//! 相关：`docs/adr/0089-unattended-round-cap-derived-and-human-reset.md`、
//! `docs/automation-charter.md` §11.3、`docs/automations/2026-10-26-round-1.md`。

use std::path::Path;

/// 一个轮次文件的机制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    /// 人类在场会话。
    Human,
    /// 无人值守 automation。
    Automation,
    /// 文件存在，但没有可识别的机制声明；统计时按 automation 处理。
    Missing,
}

/// 从文件名和内容解析出的轮次事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundEntry {
    /// 仓库相对路径，使用 `/` 分隔。
    pub path: String,
    /// 运行日，格式固定为 `YYYY-MM-DD`。
    pub run_date: String,
    /// 运行日内的轮次号。
    pub round: u32,
    /// 文件声明的机制。
    pub mechanism: Mechanism,
}

/// 提供给 automation prompt 和人工审计的派生结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundSummary {
    /// 查询的运行日。
    pub run_date: String,
    /// 该运行日已有轮次数。
    pub existing_rounds_for_run_date: usize,
    /// 该运行日下一轮次号。
    pub next_round: u32,
    /// 最近一次 human 轮次路径。
    pub latest_human: Option<String>,
    /// 最近 human 之后、包含 Missing 的 automation 数。
    pub n_auto: usize,
    /// 最近 human 之后缺机制声明的路径。
    pub missing_after_latest_human: Vec<String>,
    /// 最近 human 之前缺机制声明的路径；仅作历史警告。
    pub missing_before_latest_human: Vec<String>,
}

impl RoundSummary {
    /// 当前状态是否可用于 automation 计数。
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        self.latest_human.is_some() && self.missing_after_latest_human.is_empty()
    }
}

/// 执行 `automation-rounds --run-date YYYY-MM-DD`。
///
/// # 错误
/// - 运行日格式非法；
/// - `docs/automations` 无法读取；
/// - 文件无法读取。
pub fn run(
    repo_root: &Path,
    run_date: &str,
    output: &mut dyn std::io::Write,
) -> Result<u8, String> {
    use crate::{EXIT_FINDINGS, EXIT_OK};

    validate_run_date(run_date)?;
    let entries = scan_round_files(&repo_root.join("docs/automations"))?;
    let summary = summarize(&entries, run_date);
    render(&summary, output)?;
    if summary.is_usable() {
        Ok(EXIT_OK)
    } else {
        Ok(EXIT_FINDINGS)
    }
}

/// 从目录扫描并排序轮次文件。
fn scan_round_files(directory: &Path) -> Result<Vec<RoundEntry>, String> {
    let read_dir = std::fs::read_dir(directory)
        .map_err(|error| format!("读 {} 失败：{error}", directory.display()))?;
    let mut entries = Vec::new();
    for item in read_dir {
        let item = item.map_err(|error| format!("遍历 {} 失败：{error}", directory.display()))?;
        let path = item.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((run_date, round)) = parse_round_file_name(file_name) else {
            continue;
        };
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("读 {} 失败：{error}", path.display()))?;
        entries.push(RoundEntry {
            path: format!("docs/automations/{file_name}"),
            run_date,
            round,
            mechanism: classify_mechanism(&content),
        });
    }
    entries.sort_by(|left, right| {
        (&left.run_date, left.round, &left.path).cmp(&(&right.run_date, right.round, &right.path))
    });
    Ok(entries)
}

/// 汇总最近 human reset 与指定运行日下一轮号。
#[must_use]
pub fn summarize(entries: &[RoundEntry], run_date: &str) -> RoundSummary {
    let mut ordered = entries.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        (left.run_date.as_str(), left.round, left.path.as_str()).cmp(&(
            right.run_date.as_str(),
            right.round,
            right.path.as_str(),
        ))
    });
    let latest_human_index = ordered
        .iter()
        .rposition(|entry| entry.mechanism == Mechanism::Human);
    let mut missing_after_latest_human = Vec::new();
    let mut missing_before_latest_human = Vec::new();
    for (index, entry) in ordered.iter().enumerate() {
        if entry.mechanism != Mechanism::Missing {
            continue;
        }
        if latest_human_index.is_some_and(|human_index| index > human_index) {
            missing_after_latest_human.push(entry.path.clone());
        } else if latest_human_index.is_some_and(|human_index| index < human_index) {
            missing_before_latest_human.push(entry.path.clone());
        }
    }
    let n_auto = latest_human_index.map_or(0, |human_index| {
        ordered
            .get(human_index.saturating_add(1)..)
            .unwrap_or_default()
            .iter()
            .filter(|entry| entry.mechanism != Mechanism::Human)
            .count()
    });
    let existing_rounds_for_run_date = ordered
        .iter()
        .filter(|entry| entry.run_date == run_date)
        .count();
    let next_round = ordered
        .iter()
        .filter(|entry| entry.run_date == run_date)
        .map(|entry| entry.round)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    RoundSummary {
        run_date: run_date.to_owned(),
        existing_rounds_for_run_date,
        next_round,
        latest_human: latest_human_index
            .and_then(|index| ordered.get(index).map(|entry| entry.path.clone())),
        n_auto,
        missing_after_latest_human,
        missing_before_latest_human,
    }
}

/// 渲染稳定、易解析的输出。
fn render(summary: &RoundSummary, output: &mut dyn std::io::Write) -> Result<(), String> {
    let latest_human = summary.latest_human.as_deref().unwrap_or("NONE");
    let verdict = if summary.is_usable() {
        "PASSED"
    } else {
        "FAILED"
    };
    writeln!(
        output,
        "== automation-rounds ==\nrun_date={}\nexisting_rounds_for_run_date={}\nnext_round={}\nlatest_human={}\nn_auto={}\nmissing_after_latest_human={}\nmissing_before_latest_human={}\n-- verdict: {}",
        summary.run_date,
        summary.existing_rounds_for_run_date,
        summary.next_round,
        latest_human,
        summary.n_auto,
        summary.missing_after_latest_human.len(),
        summary.missing_before_latest_human.len(),
        verdict
    )
    .map_err(|error| format!("写 stdout 失败：{error}"))
    ?;
    for path in &summary.missing_after_latest_human {
        writeln!(output, "missing_after_latest_human_path={path}")
            .map_err(|error| format!("写 stdout 失败：{error}"))?;
    }
    Ok(())
}

/// 解析 `YYYY-MM-DD-round-N.md`；不匹配的文件返回 `None`。
#[must_use]
pub fn parse_round_file_name(file_name: &str) -> Option<(String, u32)> {
    let stem = file_name.strip_suffix(".md")?;
    let (run_date, remainder) = stem.split_at_checked(10)?;
    if !is_iso_date(run_date) {
        return None;
    }
    let round_text = remainder.strip_prefix("-round-")?;
    if round_text.is_empty() || !round_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let round = round_text.parse::<u32>().ok()?;
    if round == 0 {
        return None;
    }
    Some((run_date.to_owned(), round))
}

/// 从轮次文件内容识别机制声明。
///
/// 只检查明显是机制声明的前缀行，避免被正文里讨论“human 会话”或“automation”
/// 的普通句子误触发。第一个可识别声明生效。
#[must_use]
pub fn classify_mechanism(content: &str) -> Mechanism {
    for line in content.lines() {
        let normalized = line.trim().replace(['*', '_'], "").to_ascii_lowercase();
        let Some(marker) = normalized.find("机制") else {
            continue;
        };
        let declaration_prefix = normalized
            .get(marker..marker.saturating_add(32))
            .map_or_else(
                || normalized.get(marker..).unwrap_or_default(),
                |prefix| prefix,
            );
        if !declaration_prefix.contains('：')
            && !declaration_prefix.contains(':')
            && !declaration_prefix.contains('=')
        {
            continue;
        }
        if declaration_prefix.contains("human") || declaration_prefix.contains("人类") {
            return Mechanism::Human;
        }
        if declaration_prefix.contains("automation") || declaration_prefix.contains("自动") {
            return Mechanism::Automation;
        }
    }
    Mechanism::Missing
}

/// 校验输入运行日。
pub fn validate_run_date(run_date: &str) -> Result<(), String> {
    if is_iso_date(run_date) {
        Ok(())
    } else {
        Err(format!("--run-date 必须是 YYYY-MM-DD，收到 `{run_date}`"))
    }
}

/// 固定形状校验，不做日历合法性推断。
fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[cfg(test)]
#[path = "automation_rounds_tests.rs"]
mod tests;
