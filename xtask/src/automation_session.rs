//! # Codex 会话来源与 human reset 水位
//!
//! 职责：
//! - 从 `CODEX_SESSION_ID` 定位当前 Codex rollout 的 `session_meta`；
//! - 区分 automation 与人类会话；
//! - 在仓库外维护一个有界的 human prompt / session reset 水位。
//!
//! 边界：
//! - 不读取会话正文，只读取首行元数据中的 `thread_source`；
//! - 不修改仓库或 Codex 数据库；
//! - 只保留最新一个 reset 水位，不追加无界历史。
//!
//! 不变量：
//! 1. 只有明确 `thread_source=automation` 才是 automation；
//! 2. 会话 ID 存在但文件缺失、重复或来源未知时显式失败；
//! 3. reset 文件损坏时显式失败，不静默回退为 human。
//!
//! 相关：ADR-0099、ADR-0089、`docs/automation-charter.md` §11.3。

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SESSION_SCAN_LIMIT: usize = 8_192;
const SESSION_META_READ_LIMIT: u64 = 64 * 1_024;
const RESET_FILE_READ_LIMIT: u64 = 4 * 1_024;
const RESET_SCHEMA_VERSION: &str = "1";
const RESET_FILE_NAME: &str = "human-session-reset.v1";

/// 当前 Codex 会话来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSource {
    /// 人类驱动的会话。
    Human,
    /// Codex automation 会话。
    Automation,
}

/// 从 rollout 元数据识别出的当前会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSession {
    /// Codex session / thread id。
    pub session_id: String,
    /// 会话来源。
    pub source: SessionSource,
    /// rollout 文件路径，仅用于诊断。
    pub session_file: PathBuf,
}

/// 人类会话记录的单文件 reset 水位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanReset {
    /// 触发 reset 的 session id。
    pub session_id: String,
    /// reset 记录时间，Unix epoch milliseconds。
    pub recorded_at_epoch_ms: u128,
    /// reset 时仓库中最后一个 round 的运行日；没有历史 round 时为 `None`。
    pub after_run_date: Option<String>,
    /// reset 时仓库中最后一个 round 的轮次；没有历史 round 时为 0。
    pub after_round: u32,
}

/// 从当前进程环境识别 Codex 会话。
///
/// # Errors
/// 当 `CODEX_SESSION_ID` / `CODEX_THREAD_ID` 存在，但 Codex home、rollout 文件或
/// `thread_source` 无法可靠解析时返回错误。未设置 session id 时返回 `Ok(None)`，
/// 供 CI / 非 Codex 调用保留历史 human-round 兼容模式。
pub fn current_session_from_environment() -> Result<Option<CurrentSession>, String> {
    let Some(session_id) = env::var("CODEX_SESSION_ID")
        .ok()
        .or_else(|| env::var("CODEX_THREAD_ID").ok())
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let codex_home = codex_home_from_environment()?;
    discover_current_session(&codex_home, session_id.trim()).map(Some)
}

/// 从指定 Codex home 与 session id 定位并分类当前会话。
///
/// # Errors
/// rollout 文件缺失、重复、无法读取或缺少可识别来源时返回错误。
pub fn discover_current_session(
    codex_home: &Path,
    session_id: &str,
) -> Result<CurrentSession, String> {
    let sessions_root = codex_home.join("sessions");
    let session_file = find_session_file(&sessions_root, session_id)?.ok_or_else(|| {
        format!(
            "未找到 session `{session_id}` 的 rollout 文件（根目录：{}）",
            sessions_root.display()
        )
    })?;
    let first_line = read_first_line(&session_file)?;
    let source = classify_session_meta(&first_line).ok_or_else(|| {
        format!(
            "无法从 {} 的 session_meta 识别 thread_source",
            session_file.display()
        )
    })?;
    Ok(CurrentSession {
        session_id: session_id.to_owned(),
        source,
        session_file,
    })
}

/// 从 `session_meta` JSON 首行识别 `thread_source`。
///
/// 该函数只接受明确的来源声明。因为 `thread_source` 位于首行、格式化 JSON 可能会出现
/// ASCII 空白，所以先压缩空白再做固定字段匹配；不会扫描会话正文。
#[must_use]
pub fn classify_session_meta(meta_line: &str) -> Option<SessionSource> {
    let compact = meta_line
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact.contains("\"thread_source\":\"automation\"") {
        return Some(SessionSource::Automation);
    }
    compact
        .contains("\"thread_source\":\"")
        .then_some(SessionSource::Human)
}

/// 返回仓库外的唯一 reset 水位路径。
#[must_use]
pub fn reset_watermark_path(codex_home: &Path) -> PathBuf {
    codex_home.join("automations").join(RESET_FILE_NAME)
}

/// 读取最新的 human reset 水位。
///
/// # Errors
/// 文件存在但无法读取、字段缺失、版本不支持或内容损坏时返回错误。
pub fn load_human_reset(codex_home: &Path) -> Result<Option<HumanReset>, String> {
    let path = reset_watermark_path(codex_home);
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "读取 human reset 水位元数据 {} 失败：{error}",
            path.display()
        )
    })?;
    if metadata.len() > RESET_FILE_READ_LIMIT {
        return Err(format!(
            "human reset 水位 {} 超过 {RESET_FILE_READ_LIMIT} 字节",
            path.display()
        ));
    }
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("读取 human reset 水位 {} 失败：{error}", path.display()))?;
    parse_human_reset(&content)
        .map(Some)
        .map_err(|error| format!("解析 human reset 水位 {} 失败：{error}", path.display()))
}

/// 写入并同步最新的 human reset 水位。
///
/// # Errors
/// 无法创建目录、写文件或 flush 时返回错误。
pub fn save_human_reset(codex_home: &Path, reset: &HumanReset) -> Result<PathBuf, String> {
    let path = reset_watermark_path(codex_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建 reset 目录 {} 失败：{error}", parent.display()))?;
    }
    let after_run_date = reset.after_run_date.as_deref().unwrap_or("NONE");
    let content = format!(
        "schema_version={RESET_SCHEMA_VERSION}\nsession_id={}\nrecorded_at_epoch_ms={}\nafter_run_date={after_run_date}\nafter_round={}\n",
        reset.session_id, reset.recorded_at_epoch_ms, reset.after_round
    );
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|error| format!("打开 human reset 水位 {} 失败：{error}", path.display()))?;
    file.write_all(content.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("写入 human reset 水位 {} 失败：{error}", path.display()))?;
    Ok(path)
}

/// 当前时刻的 Unix epoch milliseconds。
///
/// # Errors
/// 系统时钟早于 Unix epoch 时返回错误。
pub fn epoch_milliseconds_now() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| format!("系统时钟早于 Unix epoch：{error}"))
}

pub fn codex_home_from_environment() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join(".codex"));
    }
    if let Some(path) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join(".codex"));
    }
    Err("无法从 CODEX_HOME / USERPROFILE / HOME 定位 Codex home".to_owned())
}

fn find_session_file(root: &Path, session_id: &str) -> Result<Option<PathBuf>, String> {
    if !root.exists() {
        return Ok(None);
    }
    let suffix = format!("{session_id}.jsonl");
    let mut pending = vec![root.to_path_buf()];
    let mut scanned = 0usize;
    let mut matched = None;
    while let Some(directory) = pending.pop() {
        let read_dir = fs::read_dir(&directory)
            .map_err(|error| format!("遍历 sessions 目录 {} 失败：{error}", directory.display()))?;
        for item in read_dir {
            scanned = scanned.saturating_add(1);
            if scanned > SESSION_SCAN_LIMIT {
                return Err(format!(
                    "sessions 扫描超过 {SESSION_SCAN_LIMIT} 个条目，拒绝无界搜索"
                ));
            }
            let item = item.map_err(|error| {
                format!("读取 sessions 条目 {} 失败：{error}", directory.display())
            })?;
            let file_type = item.file_type().map_err(|error| {
                format!("读取 {} 的文件类型失败：{error}", item.path().display())
            })?;
            if file_type.is_dir() {
                pending.push(item.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let file_name = item.file_name().to_string_lossy().into_owned();
            if !file_name.ends_with(&suffix) {
                continue;
            }
            if matched.is_some() {
                return Err(format!("session `{session_id}` 对应多个 rollout 文件"));
            }
            matched = Some(item.path());
        }
    }
    Ok(matched)
}

fn read_first_line(path: &Path) -> Result<String, String> {
    let file = File::open(path)
        .map_err(|error| format!("打开 rollout 文件 {} 失败：{error}", path.display()))?;
    let mut reader = BufReader::new(file.take(SESSION_META_READ_LIMIT));
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(|error| format!("读取 rollout 首行 {} 失败：{error}", path.display()))?;
    if first_line.is_empty() {
        return Err(format!("rollout 文件 {} 为空", path.display()));
    }
    Ok(first_line)
}

fn parse_human_reset(content: &str) -> Result<HumanReset, String> {
    let fields = parse_reset_fields(content)?;
    if fields.get("schema_version").map(String::as_str) != Some(RESET_SCHEMA_VERSION) {
        return Err("schema_version 不是 1".to_owned());
    }
    let session_id = required_field(&fields, "session_id")?.to_owned();
    if session_id.is_empty() {
        return Err("session_id 为空".to_owned());
    }
    let recorded_at_epoch_ms = required_field(&fields, "recorded_at_epoch_ms")?
        .parse::<u128>()
        .map_err(|error| format!("recorded_at_epoch_ms 非法：{error}"))?;
    let (after_run_date, after_round) = parse_reset_boundary(&fields)?;
    Ok(HumanReset {
        session_id,
        recorded_at_epoch_ms,
        after_run_date,
        after_round,
    })
}

fn parse_reset_fields(content: &str) -> Result<BTreeMap<String, String>, String> {
    let mut fields = BTreeMap::new();
    for line in content.lines().filter(|line| !line.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("非法行 `{line}`"));
        };
        if fields.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(format!("重复字段 `{key}`"));
        }
    }
    Ok(fields)
}

fn parse_reset_boundary(
    fields: &BTreeMap<String, String>,
) -> Result<(Option<String>, u32), String> {
    let after_run_date = match required_field(fields, "after_run_date")? {
        "NONE" => None,
        value if is_iso_date(value) => Some(value.to_owned()),
        value => return Err(format!("after_run_date 非法：{value}")),
    };
    let after_round = required_field(fields, "after_round")?
        .parse::<u32>()
        .map_err(|error| format!("after_round 非法：{error}"))?;
    if after_run_date.is_none() && after_round != 0 {
        return Err("after_run_date=NONE 时 after_round 必须为 0".to_owned());
    }
    if after_run_date.is_some() && after_round == 0 {
        return Err("after_run_date 存在时 after_round 必须大于 0".to_owned());
    }
    Ok((after_run_date, after_round))
}

fn required_field<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    fields
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("缺少字段 `{key}`"))
}

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
#[path = "automation_session_tests.rs"]
mod tests;
