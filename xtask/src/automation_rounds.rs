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

use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::automation_session::{
    self, CurrentSession, HumanReset, SessionSource, codex_home_from_environment,
};

const AUTOMATION_PATHSPEC: &str = "docs/automations/";
const MAX_MAIN_TREE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ROUND_FILES: usize = 8_192;
const MAX_ROUND_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_GIT_STDERR_BYTES: usize = 64 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

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
    let entries = load_main_round_files(repo_root)?;
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
    let entries = load_main_round_files(repo_root)?;
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

/// 从固定 main commit 快照读取并排序轮次文件。
fn load_main_round_files(repo_root: &Path) -> Result<Vec<RoundEntry>, String> {
    let main_commit = resolve_main_ref(repo_root)?;
    let args = [
        "ls-tree",
        "-z",
        "--format=%(objecttype) %(path)",
        &main_commit,
        "--",
        AUTOMATION_PATHSPEC,
    ];
    let listing = run_git_output(repo_root, &args, MAX_MAIN_TREE_BYTES)?;
    let mut entries = Vec::new();
    let paths = listing
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty());
    for (index, raw_path) in paths.enumerate() {
        if index >= MAX_ROUND_FILES {
            return Err(format!("main round file count exceeds {MAX_ROUND_FILES}"));
        }
        let separator = raw_path
            .iter()
            .position(|byte| *byte == b' ')
            .ok_or_else(|| "git ls-tree returned malformed entry".to_owned())?;
        let object_type = raw_path
            .get(..separator)
            .ok_or_else(|| "git ls-tree returned malformed object type".to_owned())?;
        if object_type == b"tree" {
            continue;
        }
        if object_type != b"blob" {
            return Err(format!(
                "git ls-tree returned unsupported object type: {object_type:?}"
            ));
        }
        let path = raw_path
            .get(separator + 1..)
            .ok_or_else(|| "git ls-tree returned malformed path".to_owned())?;
        let path = std::str::from_utf8(path)
            .map_err(|error| format!("git ls-tree returned non-UTF-8 path: {error}"))?;
        let file_name = path
            .strip_prefix("docs/automations/")
            .filter(|name| !name.contains('/'))
            .ok_or_else(|| format!("main round path is outside the direct directory: {path}"))?;
        let Some((run_date, round)) = parse_round_file_name(file_name) else {
            if is_allowed_non_round_file(file_name) || is_allowed_legacy_round_file(file_name) {
                continue;
            }
            return Err(format!("unexpected file in docs/automations: {path}"));
        };
        let object = format!("{main_commit}:{path}");
        let content = run_git_output(repo_root, &["show", &object], MAX_ROUND_FILE_BYTES)?;
        let content = String::from_utf8(content)
            .map_err(|error| format!("main file {path} is not valid UTF-8: {error}"))?;
        entries.push(RoundEntry {
            path: path.to_owned(),
            run_date,
            round,
            mechanism: classify_mechanism(&content),
        });
    }
    entries.sort_by_key(|entry| (entry.run_date.clone(), entry.round, entry.path.clone()));
    Ok(entries)
}

fn is_allowed_non_round_file(file_name: &str) -> bool {
    const FIXED_FILES: [&str; 3] = [
        ".gitkeep",
        "codex-automations-operations.md",
        "scheduler-acceptance-test.md",
    ];
    if FIXED_FILES.contains(&file_name) {
        return true;
    }
    let Some((run_date, remainder)) = file_name
        .strip_suffix(".md")
        .and_then(|stem| stem.split_at_checked(10))
    else {
        return false;
    };
    is_iso_date(run_date) && (remainder.starts_with("-report") || remainder == "-skipped")
}

fn is_allowed_legacy_round_file(file_name: &str) -> bool {
    let Some((run_date, remainder)) = file_name
        .strip_suffix(".md")
        .and_then(|stem| stem.split_at_checked(10))
    else {
        return false;
    };
    let Some((round, suffix)) = remainder
        .strip_prefix("-round-")
        .and_then(|tail| tail.split_once('-'))
    else {
        return false;
    };
    is_iso_date(run_date)
        && !round.is_empty()
        && round.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(suffix, "run1415" | "skipped")
}

fn resolve_main_ref(root: &Path) -> Result<String, String> {
    let candidate = if git_ref_exists(root, "refs/remotes/origin/main")? {
        "refs/remotes/origin/main"
    } else if git_ref_exists(root, "refs/heads/main")? {
        "refs/heads/main"
    } else {
        return Err("no usable main ref: neither origin/main nor main exists".to_owned());
    };
    let revision = format!("{candidate}^{{commit}}");
    let bytes = run_git_output(root, &["rev-parse", "--verify", &revision], 128)
        .map_err(|error| format!("no usable main ref {candidate}: {error}"))?;
    let commit = std::str::from_utf8(&bytes)
        .map_err(|_| format!("no usable main ref {candidate}: non-UTF-8 commit hash"))?
        .trim();
    if commit.is_empty() || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "no usable main ref {candidate}: invalid commit `{commit}`"
        ));
    }
    Ok(commit.to_owned())
}

fn git_ref_exists(root: &Path, reference: &str) -> Result<bool, String> {
    let args = ["show-ref", "--verify", "--quiet", reference];
    let output = run_git_command(root, &args, 128)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) if output.stderr.is_empty() => Ok(false),
        _ => Err(format!(
            "git {args:?} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

fn run_git_command(root: &Path, args: &[&str], max_bytes: usize) -> Result<Output, String> {
    let mut command = Command::new("git");
    command.env_clear();
    for (key, value) in std::env::vars_os() {
        if !is_git_environment_name(&key) {
            command.env(key, value);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start git {args:?}: {error}"))?;
    let (sender, receiver) = mpsc::channel();
    let readers = [
        spawn_bounded_reader(child.stdout.take(), max_bytes, true, sender.clone()),
        spawn_bounded_reader(child.stderr.take(), MAX_GIT_STDERR_BYTES, false, sender),
    ];
    let started = Instant::now();
    let mut stdout = None;
    let mut stderr = None;
    while stdout.is_none() || stderr.is_none() {
        let remaining = GIT_TIMEOUT.saturating_sub(started.elapsed());
        match receiver.recv_timeout(remaining) {
            Ok((is_stdout, Ok(bytes))) if is_stdout && stdout.is_none() => stdout = Some(bytes),
            Ok((is_stdout, Ok(bytes))) if !is_stdout && stderr.is_none() => stderr = Some(bytes),
            Ok((_, Ok(_))) => {
                return Err(abort_git(&mut child, args, "duplicate pipe", readers));
            }
            Ok((_, Err(error))) => return Err(abort_git(&mut child, args, &error, readers)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(abort_git(&mut child, args, "timed out", readers));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(abort_git(
                    &mut child,
                    args,
                    "pipe readers stopped before completion",
                    readers,
                ));
            }
        }
    }
    let status = wait_git(&mut child, started, args, readers)?;
    Ok(Output {
        status,
        stdout: stdout.ok_or_else(|| format!("git {args:?} has no stdout"))?,
        stderr: stderr.ok_or_else(|| format!("git {args:?} has no stderr"))?,
    })
}

fn wait_git(
    child: &mut Child,
    started: Instant,
    args: &[&str],
    readers: [JoinHandle<()>; 2],
) -> Result<ExitStatus, String> {
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < GIT_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                return Err(abort_git(child, args, "timed out", readers));
            }
            Err(error) => {
                return Err(abort_git(
                    child,
                    args,
                    &format!("wait failed: {error}"),
                    readers,
                ));
            }
        }
    };
    join_readers(readers).map_err(|error| format!("git {args:?}: {error}"))?;
    Ok(status)
}

fn spawn_bounded_reader<R: Read + Send + 'static>(
    reader: Option<R>,
    limit: usize,
    is_stdout: bool,
    sender: Sender<(bool, Result<Vec<u8>, String>)>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let result = reader.map_or_else(
            || Err("pipe unavailable".to_owned()),
            |reader| {
                reader
                    .take(limit as u64 + 1)
                    .read_to_end(&mut output)
                    .map_err(|error| format!("read failed: {error}"))
                    .and_then(|_| {
                        if output.len() > limit {
                            Err(format!("pipe output exceeded {limit} bytes"))
                        } else {
                            Ok(output)
                        }
                    })
            },
        );
        drop(sender.send((is_stdout, result)));
    })
}

fn is_git_environment_name(name: &OsStr) -> bool {
    name.to_string_lossy()
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
}

fn abort_git(
    child: &mut Child,
    args: &[&str],
    error: &str,
    readers: [JoinHandle<()>; 2],
) -> String {
    let mut cleanup_errors = Vec::new();
    if let Err(cleanup_error) = terminate_git(child) {
        cleanup_errors.push(cleanup_error);
    }
    if let Err(cleanup_error) = join_readers(readers) {
        cleanup_errors.push(cleanup_error);
    }
    if cleanup_errors.is_empty() {
        format!("git {args:?}: {error}")
    } else {
        format!(
            "git {args:?}: {error}; cleanup: {}",
            cleanup_errors.join("; ")
        )
    }
}

fn terminate_git(child: &mut Child) -> Result<(), String> {
    let mut errors = Vec::new();
    if cfg!(windows) {
        let process_id = child.id().to_string();
        if let Err(error) = taskkill_tree(&process_id) {
            errors.push(error);
        }
    }
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Err(error) = child.kill() {
                errors.push(format!("kill failed: {error}"));
            }
        }
        Err(error) => errors.push(format!("pre-kill wait failed: {error}")),
    }
    if let Err(error) = child.wait() {
        errors.push(format!("wait failed: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn taskkill_tree(process_id: &str) -> Result<(), String> {
    if !cfg!(windows) {
        return Ok(());
    }
    let mut killer = Command::new("taskkill")
        .args(["/T", "/F", "/PID", process_id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("taskkill failed to start: {error}"))?;
    let started = Instant::now();
    loop {
        match killer.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("taskkill exited {status}")),
            Ok(None) if started.elapsed() < CLEANUP_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                let kill_error = killer.kill().err();
                let wait_error = killer.wait().err();
                return Err(format!(
                    "taskkill timed out; kill_error={kill_error:?}; wait_error={wait_error:?}"
                ));
            }
            Err(error) => return Err(format!("taskkill wait failed: {error}")),
        }
    }
}

fn join_readers(readers: [JoinHandle<()>; 2]) -> Result<(), String> {
    let mut errors = Vec::new();
    for reader in readers {
        let started = Instant::now();
        while !reader.is_finished() && started.elapsed() < CLEANUP_TIMEOUT {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !reader.is_finished() {
            errors.push("reader did not stop within cleanup timeout".to_owned());
        } else if reader.join().is_err() {
            errors.push("reader panicked".to_owned());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn run_git_output(root: &Path, args: &[&str], max_bytes: usize) -> Result<Vec<u8>, String> {
    let output = run_git_command(root, args, max_bytes)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status;
    if !status.success() {
        return Err(format!("git {args:?} failed ({status}): {}", stderr.trim()));
    }
    Ok(output.stdout)
}

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
