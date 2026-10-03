//! TASK-228 真机验收记录的严格读取/汇总入口。
//!
//! 职责：读取 `acceptance_record.rs` 写出的一份 JSON，打印每个用例的状态与测量值，
//! 并以退出码区分通过、未通过和非法记录。
//! 边界：只读本地文件；不联网，不修改记录，不替代真机测试。
//!
//! 退出码：0 = 四个用例全部 pass；1 = 记录合法但存在 skip/fail；2 = 记录不合法或用法错误。
//!
//! 不变量：字段缺失、状态不在 `pass|skip|fail` 闭集、用例集合不完整或存在未知字段时，
//! 一律显式拒绝，绝不把 unknown / skip 当作 pass。

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::process::ExitCode;

use assistant_protocol::serde_json::{Map, Value};

const SCHEMA_VERSION: &str = "1.0";

const ALLOWED_CASES: [&str; 4] = [
    "pointer_move_lands_on_requested_physical_point",
    "unicode_text_and_ctrl_s_round_trip_through_real_notepad",
    "pointer_calibration_covers_real_display_set",
    "pointer_click_focuses_known_notepad_element",
];

const TOP_LEVEL_FIELDS: &[&str] = &[
    "schema_version",
    "run_id",
    "started_at",
    "finished_at",
    "cases",
    "host",
];

const CASE_FIELDS: &[&str] = &["name", "status", "reason", "measurements"];
const HOST_FIELDS: &[&str] = &["os_build", "scale_factor", "display_count"];

#[derive(Debug, Clone)]
struct CaseSummary {
    name: String,
    status: String,
    reason: String,
    measurements: Vec<(String, String)>,
}

#[derive(Debug)]
struct AcceptanceSummary {
    schema_version: String,
    run_id: String,
    started_at: u64,
    finished_at: u64,
    cases: Vec<CaseSummary>,
    pass_count: usize,
    skip_count: usize,
    fail_count: usize,
}

impl AcceptanceSummary {
    fn all_passed(&self) -> bool {
        self.pass_count == ALLOWED_CASES.len() && self.skip_count == 0 && self.fail_count == 0
    }
}

fn main() -> ExitCode {
    let path = match parse_single_argument(env::args_os()) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("usage: assistant-acceptance-report <record-path>");
            eprintln!("INVALID: {message}");
            return ExitCode::from(2);
        }
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(failure) => {
            eprintln!("INVALID: read {} failed: {failure}", path.display());
            return ExitCode::from(2);
        }
    };
    let summary = match parse_record(&text) {
        Ok(summary) => summary,
        Err(message) => {
            eprintln!("INVALID RECORD: {message}");
            return ExitCode::from(2);
        }
    };
    print_summary(&summary);
    if summary.all_passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn parse_single_argument(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<OsString, String> {
    let _program = arguments.next();
    let path = arguments
        .next()
        .ok_or_else(|| "missing record path".to_owned())?;
    if arguments.next().is_some() {
        return Err("expected exactly one record path".to_owned());
    }
    Ok(path)
}

fn parse_record(text: &str) -> Result<AcceptanceSummary, String> {
    let root: Value = assistant_protocol::serde_json::from_str(text)
        .map_err(|failure| format!("invalid JSON: {failure}"))?;
    let root = require_object(&root, "root")?;
    reject_unknown_fields(root, TOP_LEVEL_FIELDS, "root")?;

    let schema_version = require_nonempty_string(root, "schema_version", "root")?;
    if schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version `{schema_version}`; expected `{SCHEMA_VERSION}`"
        ));
    }
    let run_id = require_nonempty_string(root, "run_id", "root")?;
    let started_at = require_u64(root, "started_at", "root")?;
    let finished_at = require_u64(root, "finished_at", "root")?;
    if finished_at < started_at {
        return Err(format!(
            "finished_at ({finished_at}) is earlier than started_at ({started_at})"
        ));
    }
    validate_host(require_object(
        require_field(root, "host", "root")?,
        "host",
    )?)?;

    let case_values = require_array(require_field(root, "cases", "root")?, "cases")?;
    if case_values.len() != ALLOWED_CASES.len() {
        return Err(format!(
            "expected exactly {} cases, found {}",
            ALLOWED_CASES.len(),
            case_values.len()
        ));
    }

    let allowed_cases: BTreeSet<&str> = ALLOWED_CASES.into_iter().collect();
    let mut cases_by_name = BTreeMap::new();
    for (index, value) in case_values.iter().enumerate() {
        let context = format!("cases[{index}]");
        let object = require_object(value, &context)?;
        reject_unknown_fields(object, CASE_FIELDS, &context)?;
        let name = require_nonempty_string(object, "name", &context)?;
        if !allowed_cases.contains(name.as_str()) {
            return Err(format!("unknown case name `{name}`"));
        }
        let status = require_nonempty_string(object, "status", &context)?;
        if !matches!(status.as_str(), "pass" | "skip" | "fail") {
            return Err(format!(
                "case `{name}` has unknown status `{status}`; expected pass|skip|fail"
            ));
        }
        let reason = require_string(object, "reason", &context)?;
        if matches!(status.as_str(), "skip" | "fail") && reason.trim().is_empty() {
            return Err(format!(
                "case `{name}` status `{status}` requires a non-empty reason"
            ));
        }
        let measurements = parse_measurements(
            require_object(
                require_field(object, "measurements", &context)?,
                "measurements",
            )?,
            &name,
        )?;
        let summary = CaseSummary {
            name: name.clone(),
            status,
            reason,
            measurements,
        };
        if cases_by_name.insert(name.clone(), summary).is_some() {
            return Err(format!("duplicate case name `{name}`"));
        }
    }

    let mut cases = Vec::with_capacity(ALLOWED_CASES.len());
    for name in ALLOWED_CASES {
        let case = cases_by_name
            .remove(name)
            .ok_or_else(|| format!("missing case `{name}`"))?;
        cases.push(case);
    }

    Ok(AcceptanceSummary {
        schema_version,
        run_id,
        started_at,
        finished_at,
        pass_count: cases.iter().filter(|case| case.status == "pass").count(),
        skip_count: cases.iter().filter(|case| case.status == "skip").count(),
        fail_count: cases.iter().filter(|case| case.status == "fail").count(),
        cases,
    })
}

fn validate_host(host: &Map<String, Value>) -> Result<(), String> {
    reject_unknown_fields(host, HOST_FIELDS, "host")?;
    let os_build = require_nonempty_string(host, "os_build", "host")?;
    if os_build == "unknown" {
        return Err("host.os_build is unknown".to_owned());
    }
    let scale_factor = require_number(host, "scale_factor", "host")?;
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return Err(format!(
            "host.scale_factor must be a positive finite number, found {scale_factor}"
        ));
    }
    let display_count = require_u64(host, "display_count", "host")?;
    if display_count == 0 {
        return Err("host.display_count must be at least 1".to_owned());
    }
    Ok(())
}

fn parse_measurements(
    measurements: &Map<String, Value>,
    case_name: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut rendered = Vec::with_capacity(measurements.len());
    for (name, value) in measurements {
        let value = match value {
            Value::String(value) => format!("{value:?}"),
            Value::Number(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Null => {
                return Err(format!(
                    "case `{case_name}` measurement `{name}` must not be null"
                ));
            }
            Value::Array(_) | Value::Object(_) => {
                return Err(format!(
                    "case `{case_name}` measurement `{name}` must be string|number|bool"
                ));
            }
        };
        rendered.push((name.clone(), value));
    }
    Ok(rendered)
}

fn require_object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))
}

fn require_array<'a>(value: &'a Value, context: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{context} must be an array"))
}

fn require_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{context} is missing required field `{key}`"))
}

fn require_string(object: &Map<String, Value>, key: &str, context: &str) -> Result<String, String> {
    require_field(object, key, context)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{context}.{key} must be a string"))
}

fn require_nonempty_string(
    object: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, String> {
    let value = require_string(object, key, context)?;
    if value.trim().is_empty() {
        return Err(format!("{context}.{key} must not be empty"));
    }
    Ok(value)
}

fn require_u64(object: &Map<String, Value>, key: &str, context: &str) -> Result<u64, String> {
    require_field(object, key, context)?
        .as_u64()
        .ok_or_else(|| format!("{context}.{key} must be a non-negative integer"))
}

fn require_number(object: &Map<String, Value>, key: &str, context: &str) -> Result<f64, String> {
    require_field(object, key, context)?
        .as_f64()
        .ok_or_else(|| format!("{context}.{key} must be a number"))
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{context} has unknown field `{key}`"));
        }
    }
    Ok(())
}

fn print_summary(summary: &AcceptanceSummary) {
    if summary.all_passed() {
        println!("acceptance: PASS");
    } else {
        println!(
            "acceptance: NOT PASSED (pass={} skip={} fail={})",
            summary.pass_count, summary.skip_count, summary.fail_count
        );
    }
    println!(
        "schema_version={} run_id={} started_at={} finished_at={}",
        summary.schema_version, summary.run_id, summary.started_at, summary.finished_at
    );
    for case in &summary.cases {
        let measurements = case
            .measurements
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "- {}: {} reason={} measurements={{{measurements}}}",
            case.name, case.status, case.reason
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_record() -> String {
        r#"{
  "schema_version": "1.0",
  "run_id": "windows-input-123-456",
  "started_at": 456,
  "finished_at": 789,
  "cases": [
    {
      "name": "pointer_move_lands_on_requested_physical_point",
      "status": "pass",
      "reason": "completed",
      "measurements": { "scale_factor": 2.0, "error_x": 0 }
    },
    {
      "name": "unicode_text_and_ctrl_s_round_trip_through_real_notepad",
      "status": "pass",
      "reason": "completed",
      "measurements": { "saved": true }
    },
    {
      "name": "pointer_calibration_covers_real_display_set",
      "status": "pass",
      "reason": "completed",
      "measurements": { "sample_count": 6 }
    },
    {
      "name": "pointer_click_focuses_known_notepad_element",
      "status": "pass",
      "reason": "completed",
      "measurements": { "focused": true }
    }
  ],
  "host": {
    "os_build": "10.0.26200.9457",
    "scale_factor": 2.0,
    "display_count": 1
  }
}"#
        .to_owned()
    }

    #[test]
    fn test_valid_record_all_pass_is_accepted() {
        let summary = match parse_record(&valid_record()) {
            Ok(summary) => summary,
            Err(failure) => unreachable!("valid record must parse: {failure}"),
        };
        assert!(summary.all_passed());
        assert_eq!(summary.pass_count, 4);
        assert_eq!(summary.skip_count, 0);
        assert_eq!(summary.fail_count, 0);
    }

    #[test]
    fn test_only_skip_record_is_valid_but_not_passed() {
        let record = valid_record().replace("\"status\": \"pass\"", "\"status\": \"skip\"");
        let summary = match parse_record(&record) {
            Ok(summary) => summary,
            Err(failure) => unreachable!("skip record must still be structurally valid: {failure}"),
        };
        assert!(!summary.all_passed());
        assert_eq!(summary.pass_count, 0);
        assert_eq!(summary.skip_count, 4);
    }

    #[test]
    fn test_unknown_status_is_rejected() {
        let record = valid_record().replacen("\"status\": \"pass\"", "\"status\": \"unknown\"", 1);
        let failure = match parse_record(&record) {
            Ok(summary) => unreachable!("unknown status must be rejected: {summary:?}"),
            Err(failure) => failure,
        };
        assert!(failure.contains("unknown status"), "{failure}");
    }

    #[test]
    fn test_missing_status_is_rejected() {
        let record = valid_record().replacen("\"status\": \"pass\",", "", 1);
        let failure = match parse_record(&record) {
            Ok(summary) => unreachable!("missing status must be rejected: {summary:?}"),
            Err(failure) => failure,
        };
        assert!(
            failure.contains("missing required field `status`"),
            "{failure}"
        );
    }

    #[test]
    fn test_unknown_top_level_field_is_rejected() {
        let record =
            valid_record().replacen("\"run_id\":", "\"unexpected\": true,\n  \"run_id\":", 1);
        let failure = match parse_record(&record) {
            Ok(summary) => unreachable!("unknown field must be rejected: {summary:?}"),
            Err(failure) => failure,
        };
        assert!(failure.contains("unknown field `unexpected`"), "{failure}");
    }
}
