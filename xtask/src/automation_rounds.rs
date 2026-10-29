//! # automation-rounds 子命令
//!
//! 职责：
//! - 从 `docs/automations/<run-date>-round-<N>.md` 解析每个轮次的机制；
//! - 从每日 08:00、人类 prompt hook 水位与历史 human round 找 reset 边界；
//! - 统计 reset 边界后的 `机制=automation` 数量；
//! - 计算指定运行日下一可用轮次号。
//!
//! 边界：
//! - 读取仓库轮次文件、Codex session 首行元数据与仓库外 reset 水位；
//! - 不修改仓库轮次文件或台账；
//! - 不判断任务内容质量，只处理 ADR-0089 的轮次计数事实。
//!
//! 不变量：
//! 1. 缺机制字段按 automation fail-closed，不能静默跳过；
//! 2. 人类会话自动写单文件 reset 水位，automation 会话只读；
//! 3. 没有可验证 reset 边界时绝不伪造 reset。
//!
//! 相关：`docs/adr/0099-human-session-auto-reset.md`、
//! `docs/adr/0089-unattended-round-cap-derived-and-human-reset.md`、
//! `docs/automation-charter.md` §11.3、`docs/automations/2026-10-26-round-1.md`。

use std::path::Path;

use crate::automation_session::{
    self, CurrentSession, HumanReset, SessionSource, codex_home_from_environment,
};

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
    /// 最近 reset 边界：`daily:<date>`、`session:<id>` 或 human round 路径。
    pub latest_human: Option<String>,
    /// reset 边界之后、包含 Missing 的 automation 数。
    pub n_auto: usize,
    /// reset 边界之后缺机制声明的路径。
    pub missing_after_latest_human: Vec<String>,
    /// reset 边界之前缺机制声明的路径；仅作历史警告。
    pub missing_before_latest_human: Vec<String>,
    /// 当前 Codex 会话来源。
    pub session_source: Option<SessionSource>,
    /// reset 边界来源。
    pub reset_source: ResetSource,
}

/// reset 边界来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetSource {
    /// 人类 prompt hook 自动写入的会话水位。
    HumanSession,
    /// 当前运行日的本地 08:00 自动边界。
    DailyBoundary,
    /// 历史 `机制=human` 轮次文件。
    HumanRound,
    /// 没有可验证的 reset 边界。
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RoundBoundary {
    run_date: String,
    round: u32,
}

impl RoundBoundary {
    fn from_entry(entry: &RoundEntry) -> Self {
        Self {
            run_date: entry.run_date.clone(),
            round: entry.round,
        }
    }

    fn from_reset(reset: &HumanReset) -> Self {
        Self {
            run_date: reset.after_run_date.clone().unwrap_or_default(),
            round: reset.after_round,
        }
    }

    fn is_before_or_equal(&self, entry: &RoundEntry) -> bool {
        (&entry.run_date, entry.round) <= (&self.run_date, self.round)
    }
}

struct BoundaryChoice {
    boundary: Option<RoundBoundary>,
    source: ResetSource,
    latest_human: Option<String>,
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
    let current_session = automation_session::current_session_from_environment()?;
    let human_reset = prepare_human_reset(&entries, current_session.as_ref())?;
    let summary = summarize_with_human_reset(
        &entries,
        run_date,
        human_reset.as_ref(),
        current_session.as_ref(),
    );
    render(&summary, output)?;
    if summary.is_usable() {
        Ok(EXIT_OK)
    } else {
        Ok(EXIT_FINDINGS)
    }
}

/// `UserPromptSubmit` hook：每次人类消息自动更新 reset 水位。
///
/// automation 会话调用时直接 no-op；人类会话只写仓库外单文件水位，不产生 stdout。
///
/// # Errors
/// session 元数据不可验证、轮次扫描失败或 reset 水位无法写入时返回错误。
pub fn record_human_prompt(repo_root: &Path) -> Result<u8, String> {
    let current_session = automation_session::current_session_from_environment()?
        .ok_or_else(|| "UserPromptSubmit hook 缺少 Codex session 元数据".to_owned())?;
    if current_session.source == SessionSource::Automation {
        return Ok(crate::EXIT_OK);
    }
    let entries = scan_round_files(&repo_root.join("docs/automations"))?;
    let boundary = latest_boundary(&entries);
    let reset = HumanReset {
        session_id: current_session.session_id,
        recorded_at_epoch_ms: automation_session::epoch_milliseconds_now()?,
        after_run_date: boundary.as_ref().map(|value| value.run_date.clone()),
        after_round: boundary.as_ref().map_or(0, |value| value.round),
    };
    let codex_home = codex_home_from_environment()?;
    automation_session::save_human_reset(&codex_home, &reset)?;
    Ok(crate::EXIT_OK)
}

fn prepare_human_reset(
    entries: &[RoundEntry],
    current_session: Option<&CurrentSession>,
) -> Result<Option<HumanReset>, String> {
    match current_session.map(|session| session.source) {
        Some(SessionSource::Human) => {
            let boundary = latest_boundary(entries);
            let reset = HumanReset {
                session_id: current_session
                    .map(|session| session.session_id.clone())
                    .ok_or_else(|| "human reset 缺少 session id".to_owned())?,
                recorded_at_epoch_ms: automation_session::epoch_milliseconds_now()?,
                after_run_date: boundary.as_ref().map(|value| value.run_date.clone()),
                after_round: boundary.as_ref().map_or(0, |value| value.round),
            };
            let codex_home = codex_home_from_environment()?;
            automation_session::save_human_reset(&codex_home, &reset)?;
            Ok(Some(reset))
        }
        None | Some(SessionSource::Automation) => {
            let codex_home = codex_home_from_environment()?;
            automation_session::load_human_reset(&codex_home)
        }
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
#[cfg(test)]
#[must_use]
pub fn summarize(entries: &[RoundEntry], run_date: &str) -> RoundSummary {
    summarize_with_human_reset(entries, run_date, None, None)
}

/// 汇总会话 reset、历史 human round 与指定运行日下一轮号。
#[must_use]
pub fn summarize_with_human_reset(
    entries: &[RoundEntry],
    run_date: &str,
    human_session_reset: Option<&HumanReset>,
    current_session: Option<&CurrentSession>,
) -> RoundSummary {
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
    let boundary = choose_boundary(&ordered, latest_human_index, human_session_reset, run_date);
    let mut missing_after_latest_human = Vec::new();
    let mut missing_before_latest_human = Vec::new();
    for entry in &ordered {
        if entry.mechanism != Mechanism::Missing {
            continue;
        }
        match boundary.boundary.as_ref() {
            Some(value) if value.is_before_or_equal(entry) => {
                missing_before_latest_human.push(entry.path.clone());
            }
            Some(_) => missing_after_latest_human.push(entry.path.clone()),
            None => {}
        }
    }
    let n_auto = boundary.boundary.as_ref().map_or(0, |value| {
        ordered
            .iter()
            .filter(|entry| !value.is_before_or_equal(entry) && entry.mechanism != Mechanism::Human)
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
        latest_human: boundary.latest_human,
        n_auto,
        missing_after_latest_human,
        missing_before_latest_human,
        session_source: current_session.map(|session| session.source),
        reset_source: boundary.source,
    }
}

fn choose_boundary(
    ordered: &[&RoundEntry],
    latest_human_index: Option<usize>,
    human_session_reset: Option<&HumanReset>,
    run_date: &str,
) -> BoundaryChoice {
    let human_round = latest_human_index.and_then(|index| {
        ordered
            .get(index)
            .map(|entry| (RoundBoundary::from_entry(entry), entry.path.clone()))
    });
    let human_session = human_session_reset.map(|reset| {
        (
            RoundBoundary::from_reset(reset),
            format!("session:{}", reset.session_id),
        )
    });
    let human_choice = match (human_round, human_session) {
        (Some((round_boundary, round_label)), Some((session_boundary, session_label))) => {
            if session_boundary >= round_boundary {
                BoundaryChoice {
                    boundary: Some(session_boundary),
                    source: ResetSource::HumanSession,
                    latest_human: Some(session_label),
                }
            } else {
                BoundaryChoice {
                    boundary: Some(round_boundary),
                    source: ResetSource::HumanRound,
                    latest_human: Some(round_label),
                }
            }
        }
        (Some((boundary, label)), None) => BoundaryChoice {
            boundary: Some(boundary),
            source: ResetSource::HumanRound,
            latest_human: Some(label),
        },
        (None, Some((boundary, label))) => BoundaryChoice {
            boundary: Some(boundary),
            source: ResetSource::HumanSession,
            latest_human: Some(label),
        },
        (None, None) => BoundaryChoice {
            boundary: None,
            source: ResetSource::None,
            latest_human: None,
        },
    };
    let daily_boundary = RoundBoundary {
        run_date: run_date.to_owned(),
        round: 0,
    };
    if human_choice
        .boundary
        .as_ref()
        .is_some_and(|boundary| boundary >= &daily_boundary)
    {
        return human_choice;
    }
    BoundaryChoice {
        boundary: Some(daily_boundary),
        source: ResetSource::DailyBoundary,
        latest_human: Some(format!("daily:{run_date}")),
    }
}

fn latest_boundary(entries: &[RoundEntry]) -> Option<RoundBoundary> {
    entries
        .iter()
        .max_by(|left, right| {
            (&left.run_date, left.round, left.path.as_str()).cmp(&(
                &right.run_date,
                right.round,
                right.path.as_str(),
            ))
        })
        .map(RoundBoundary::from_entry)
}

/// 渲染稳定、易解析的输出。
fn render(summary: &RoundSummary, output: &mut dyn std::io::Write) -> Result<(), String> {
    let latest_human = summary.latest_human.as_deref().unwrap_or("NONE");
    let verdict = if summary.is_usable() {
        "PASSED"
    } else {
        "FAILED"
    };
    let session_source = match summary.session_source {
        Some(SessionSource::Human) => "human",
        Some(SessionSource::Automation) => "automation",
        None => "unknown",
    };
    let reset_source = match summary.reset_source {
        ResetSource::HumanSession => "human-session",
        ResetSource::HumanRound => "human-round",
        ResetSource::DailyBoundary => "daily-0800",
        ResetSource::None => "none",
    };
    writeln!(
        output,
        "== automation-rounds ==\nrun_date={}\nexisting_rounds_for_run_date={}\nnext_round={}\nlatest_human={}\nreset_source={}\nsession_source={}\nn_auto={}\nmissing_after_latest_human={}\nmissing_before_latest_human={}\n-- verdict: {}",
        summary.run_date,
        summary.existing_rounds_for_run_date,
        summary.next_round,
        latest_human,
        reset_source,
        session_source,
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
