//! TASK-228 的真机验收结构化记录。
//!
//! 职责：把 `acceptance.rs` 里四个 `#[ignore]` 真机用例的最终状态固化为一份 JSON，
//! 让 `pass` / `skip` / `fail` 不再都落成 Rust test 框架的 `ok`。
//! 边界：只服务真机测试；不进入产品 API，不联网，不保存历史，不替代断言。
//!
//! ## 不变量
//! 1. 一次进程运行只维护一份记录，固定四个用例，每次更新都覆盖写。
//! 2. 用例开始后先写成 `fail`（未完成），结束后再改成真实结果；进程中途崩溃不会留下假 pass。
//! 3. `skip` 和 `fail` 必须有非空原因；渲染失败或写入失败会让测试显式失败。
//! 4. 默认写到 workspace 的 `target/acceptance/windows-input-acceptance.json`，
//!    可用 `AIA_ACCEPTANCE_RECORD_PATH` 覆盖；两者都在版本库之外。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::UI::HiDpi::GetDpiForSystem;

use super::enumerate_monitors;

/// 本记录的 schema 版本；读取工具只接受这一版。
pub(super) const SCHEMA_VERSION: &str = "1.0";

/// 记录路径覆盖环境变量。
pub(super) const RECORD_PATH_ENV: &str = "AIA_ACCEPTANCE_RECORD_PATH";

/// 真机用例 1 的稳定名称。
pub(super) const POINTER_MOVE_CASE: &str = "pointer_move_lands_on_requested_physical_point";

/// 真机用例 2 的稳定名称。
pub(super) const UNICODE_NOTEPAD_CASE: &str =
    "unicode_text_and_ctrl_s_round_trip_through_real_notepad";

/// 真机用例 3 的稳定名称。
pub(super) const POINTER_CALIBRATION_CASE: &str = "pointer_calibration_covers_real_display_set";

/// 真机用例 4 的稳定名称。
pub(super) const POINTER_CLICK_CASE: &str = "pointer_click_focuses_known_notepad_element";

/// 固定用例集合；顺序也是记录里的输出顺序。
pub(super) const ALL_CASES: [&str; 4] = [
    POINTER_MOVE_CASE,
    UNICODE_NOTEPAD_CASE,
    POINTER_CALIBRATION_CASE,
    POINTER_CLICK_CASE,
];

/// 一个测量值；闭集只允许文本、有限数值和布尔值。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum MeasurementValue {
    /// 可读文本（例如显示器名称）。
    Text(String),
    /// 无符号整数（计数、像素数等）。
    Integer(u64),
    /// 有限数值（像素数、缩放倍数等）。
    Number(f64),
    /// 布尔结果。
    Boolean(bool),
}

impl MeasurementValue {
    /// 构造文本测量值。
    pub(super) fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// 构造数值测量值。
    pub(super) fn number(value: impl Into<f64>) -> Self {
        Self::Number(value.into())
    }

    /// 构造整数测量值。
    pub(super) fn integer(value: impl Into<u64>) -> Self {
        Self::Integer(value.into())
    }

    /// 构造布尔测量值。
    pub(super) const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }
}

/// 用例体返回的终态。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum CaseOutcome {
    /// 用例执行并满足断言。
    Pass(Vec<(String, MeasurementValue)>),
    /// 用例因明确前置条件未满足而没有执行。
    Skip {
        /// 非空原因。
        reason: String,
        /// 前置阶段已能取得的测量值。
        measurements: Vec<(String, MeasurementValue)>,
    },
}

impl CaseOutcome {
    /// 构造通过结果。
    pub(super) fn pass(measurements: Vec<(&str, MeasurementValue)>) -> Self {
        Self::Pass(owned_measurements(measurements))
    }

    /// 构造跳过结果；原因会按原样写入记录。
    pub(super) fn skip(
        reason: impl Into<String>,
        measurements: Vec<(&str, MeasurementValue)>,
    ) -> Self {
        Self::Skip {
            reason: reason.into(),
            measurements: owned_measurements(measurements),
        }
    }
}

fn owned_measurements(
    measurements: Vec<(&str, MeasurementValue)>,
) -> Vec<(String, MeasurementValue)> {
    measurements
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// 用例状态闭集。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseStatus {
    Pass,
    Skip,
    Fail,
}

impl CaseStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Skip => "skip",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone)]
struct CaseRecord {
    name: &'static str,
    status: CaseStatus,
    reason: String,
    measurements: Vec<(String, MeasurementValue)>,
}

impl CaseRecord {
    fn pending(name: &'static str) -> Self {
        Self {
            name,
            status: CaseStatus::Skip,
            reason: "case was not executed in this process".to_owned(),
            measurements: Vec::new(),
        }
    }
}

struct AcceptanceRecord {
    run_id: String,
    started_at_milliseconds: u64,
    finished_at_milliseconds: u64,
    os_build: String,
    scale_factor: f64,
    display_count: u64,
    cases: Vec<CaseRecord>,
    path: PathBuf,
}

impl AcceptanceRecord {
    fn initialize() -> Self {
        if let Err(failure) = super::declare_per_monitor_v2() {
            super::note(format_args!(
                "WARN: 记录主机环境前无法确认 Per-Monitor V2（{failure}）"
            ));
        }
        let started_at_milliseconds = current_time_milliseconds();
        let display_count = match enumerate_monitors() {
            Ok(displays) => u64::try_from(displays.len()).unwrap_or(u64::MAX),
            Err(failure) => {
                super::note(format_args!(
                    "WARN: 枚举显示器失败，记录中的 display_count 置 0（{failure}）"
                ));
                0
            }
        };
        Self {
            run_id: format!(
                "windows-input-{}-{started_at_milliseconds}",
                std::process::id()
            ),
            started_at_milliseconds,
            finished_at_milliseconds: started_at_milliseconds,
            os_build: windows_build(),
            scale_factor: current_scale_factor(),
            display_count,
            cases: ALL_CASES.into_iter().map(CaseRecord::pending).collect(),
            path: record_path(),
        }
    }

    fn case_mut(&mut self, name: &str) -> &mut CaseRecord {
        self.cases
            .iter_mut()
            .find(|case| case.name == name)
            .unwrap_or_else(|| unreachable!("acceptance record has no case named `{name}`"))
    }
}

/// 运行一个真机用例，无论 assertion panic 还是正常 skip，都会先写记录再让测试框架看到原结果。
pub(super) fn run_case<F>(name: &str, body: F)
where
    F: FnOnce() -> CaseOutcome,
{
    with_record(|record| {
        let case = record.case_mut(name);
        case.status = CaseStatus::Fail;
        "case started but did not finish".clone_into(&mut case.reason);
        case.measurements.clear();
    });
    persist_or_unreachable();

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    match outcome {
        Ok(CaseOutcome::Pass(measurements)) => {
            with_record(|record| {
                let case = record.case_mut(name);
                case.status = CaseStatus::Pass;
                "completed".clone_into(&mut case.reason);
                case.measurements = measurements;
            });
            persist_or_unreachable();
        }
        Ok(CaseOutcome::Skip {
            reason,
            measurements,
        }) => {
            let trimmed = reason.trim();
            if trimmed.is_empty() {
                unreachable!("skip case `{name}` must provide a non-empty reason");
            }
            with_record(|record| {
                let case = record.case_mut(name);
                case.status = CaseStatus::Skip;
                trimmed.clone_into(&mut case.reason);
                case.measurements = measurements;
            });
            persist_or_unreachable();
        }
        Err(payload) => {
            let reason = panic_reason(payload.as_ref());
            with_record(|record| {
                let case = record.case_mut(name);
                case.status = CaseStatus::Fail;
                case.reason = reason;
            });
            if let Err(failure) = persist_record() {
                super::note(format_args!(
                    "WARN: 写 fail 记录失败；仍保留原始 panic 让测试变红（{failure}）"
                ));
            }
            std::panic::resume_unwind(payload)
        }
    }
}

fn panic_reason(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    "panic payload was not a string".to_owned()
}

fn acceptance_record() -> &'static Mutex<AcceptanceRecord> {
    static RECORD: OnceLock<Mutex<AcceptanceRecord>> = OnceLock::new();
    RECORD.get_or_init(|| Mutex::new(AcceptanceRecord::initialize()))
}

fn with_record<T>(action: impl FnOnce(&mut AcceptanceRecord) -> T) -> T {
    {
        let mut guard = match acceptance_record().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        action(&mut guard)
    }
}

fn persist_or_unreachable() {
    if let Err(failure) = persist_record() {
        unreachable!("write acceptance record failed: {failure}");
    }
}

fn persist_record() -> Result<(), String> {
    let rendered = with_record(|record| {
        record.finished_at_milliseconds = current_time_milliseconds();
        render_record(record)
    })?;
    let path = with_record(|record| record.path.clone());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|failure| {
            format!(
                "create acceptance record directory {} failed: {failure}",
                parent.display()
            )
        })?;
    }
    std::fs::write(&path, rendered).map_err(|failure| {
        format!(
            "write acceptance record {} failed: {failure}",
            path.display()
        )
    })
}

fn render_record(record: &AcceptanceRecord) -> Result<String, String> {
    let mut output = String::new();
    output.push_str("{\n");
    output.push_str("  \"schema_version\": ");
    push_json_string(&mut output, SCHEMA_VERSION);
    output.push_str(",\n  \"run_id\": ");
    push_json_string(&mut output, &record.run_id);
    output.push_str(",\n  \"started_at\": ");
    output.push_str(&record.started_at_milliseconds.to_string());
    output.push_str(",\n  \"finished_at\": ");
    output.push_str(&record.finished_at_milliseconds.to_string());
    output.push_str(",\n  \"cases\": [\n");
    for (index, case) in record.cases.iter().enumerate() {
        output.push_str("    {\n      \"name\": ");
        push_json_string(&mut output, case.name);
        output.push_str(",\n      \"status\": ");
        push_json_string(&mut output, case.status.as_str());
        output.push_str(",\n      \"reason\": ");
        push_json_string(&mut output, &case.reason);
        output.push_str(",\n      \"measurements\": ");
        push_measurements(&mut output, &case.measurements)?;
        output.push_str("\n    }");
        if index + 1 != record.cases.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("  ],\n  \"host\": {\n    \"os_build\": ");
    push_json_string(&mut output, &record.os_build);
    output.push_str(",\n    \"scale_factor\": ");
    push_number(&mut output, record.scale_factor)?;
    output.push_str(",\n    \"display_count\": ");
    output.push_str(&record.display_count.to_string());
    output.push_str("\n  }\n}\n");
    Ok(output)
}

fn push_measurements(
    output: &mut String,
    measurements: &[(String, MeasurementValue)],
) -> Result<(), String> {
    output.push('{');
    for (index, (name, value)) in measurements.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_json_string(output, name);
        output.push_str(": ");
        push_measurement_value(output, value)?;
    }
    output.push('}');
    Ok(())
}

fn push_measurement_value(output: &mut String, value: &MeasurementValue) -> Result<(), String> {
    match value {
        MeasurementValue::Text(text) => push_json_string(output, text),
        MeasurementValue::Integer(number) => output.push_str(&number.to_string()),
        MeasurementValue::Number(number) => push_number(output, *number)?,
        MeasurementValue::Boolean(true) => output.push_str("true"),
        MeasurementValue::Boolean(false) => output.push_str("false"),
    }
    Ok(())
}

fn push_number(output: &mut String, number: f64) -> Result<(), String> {
    if !number.is_finite() {
        return Err(format!("measurement number is not finite: {number}"));
    }
    output.push_str(&number.to_string());
    Ok(())
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control.is_control() => {
                output.push_str("\\u");
                push_hex_escape(output, u32::from(control));
            }
            other => output.push(other),
        }
    }
    output.push('"');
}

fn push_hex_escape(output: &mut String, code: u32) {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
    for shift in [12_u32, 8, 4, 0] {
        let index = usize::try_from((code >> shift) & 0xF).unwrap_or(0);
        let digit = HEX_DIGITS.get(index).copied().unwrap_or(b'0');
        output.push(char::from(digit));
    }
}

fn current_time_milliseconds() -> u64 {
    let elapsed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed,
        Err(failure) => {
            super::note(format_args!(
                "WARN: 系统时间早于 Unix epoch，记录时间戳置 0（{failure}）"
            ));
            return 0;
        }
    };
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn current_scale_factor() -> f64 {
    // SAFETY: `GetDpiForSystem` 是无参数只读查询，不转移任何所有权。
    let dpi = unsafe { GetDpiForSystem() };
    if dpi == 0 {
        super::note(format_args!(
            "WARN: GetDpiForSystem 返回 0，记录中的 scale_factor 置 0"
        ));
        return 0.0;
    }
    f64::from(dpi) / 96.0
}

fn windows_build() -> String {
    match Command::new("cmd.exe").args(["/C", "ver"]).output() {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout);
            extract_windows_build(&text)
        }
        Err(failure) => {
            super::note(format_args!(
                "WARN: 查询 Windows build 失败，记录中的 os_build 置 unknown（{failure}）"
            ));
            "unknown".to_owned()
        }
    }
}

fn extract_windows_build(output: &str) -> String {
    let trimmed = output.trim();
    let Some(open) = trimmed.find('[') else {
        return trimmed.to_owned();
    };
    let Some(close) = trimmed.get(open + 1..).and_then(|rest| rest.find(']')) else {
        return trimmed.to_owned();
    };
    match trimmed.get(open + 1..open + 1 + close) {
        Some(build) if !build.trim().is_empty() => {
            build.trim().strip_prefix("Version").map_or_else(
                || build.trim().to_owned(),
                |version| version.trim().to_owned(),
            )
        }
        _ => trimmed.to_owned(),
    }
}

fn record_path() -> PathBuf {
    match std::env::var(RECORD_PATH_ENV) {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        Ok(_) | Err(std::env::VarError::NotPresent) => default_record_path(),
        Err(std::env::VarError::NotUnicode(_)) => {
            super::note(format_args!(
                "WARN: {RECORD_PATH_ENV} 不是合法 Unicode，改用默认记录路径"
            ));
            default_record_path()
        }
    }
}

fn default_record_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/acceptance/windows-input-acceptance.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_windows_build_reads_bracketed_version() {
        assert_eq!(
            extract_windows_build("Microsoft Windows [Version 10.0.26200.9457]\r\n"),
            "10.0.26200.9457"
        );
    }

    #[test]
    fn test_render_record_contains_closed_status_and_escaped_reason() {
        let record = AcceptanceRecord {
            run_id: "run-1".to_owned(),
            started_at_milliseconds: 10,
            finished_at_milliseconds: 20,
            os_build: "10.0.26200.9457".to_owned(),
            scale_factor: 2.0,
            display_count: 1,
            cases: vec![CaseRecord {
                name: POINTER_MOVE_CASE,
                status: CaseStatus::Skip,
                reason: "not \"ready\"".to_owned(),
                measurements: vec![("target_x".to_owned(), MeasurementValue::Number(100.0))],
            }],
            path: PathBuf::from("unused.json"),
        };
        let rendered = match render_record(&record) {
            Ok(rendered) => rendered,
            Err(failure) => unreachable!("record fixture must render: {failure}"),
        };
        assert!(rendered.contains("\"status\": \"skip\""));
        assert!(rendered.contains("\"reason\": \"not \\\"ready\\\"\""));
        assert!(rendered.contains("\"target_x\": 100"));
    }
}
