//! # verify-schemas 子命令（TASK-011）
//!
//! 职责：对 `protocol/**/*.json` 做**结构性**校验，充当 codegen 之前的守门人。
//!
//! ## 边界（不做什么）
//! - **不是**完整 JSON Schema 校验器（xtask 零三方依赖政策）：不解析 `$ref`、
//!   不校验 pattern/format/minLength，只做下面「不变量」列出的检查
//! - 不写盘、不生成代码 —— 那是 `codegen` 的职责
//!
//! ## 不变量
//! 1. 五份 schema 必须存在 + JSON 合法 + 顶层 `version` = 期望值
//! 2. `error-codes` 的 `categories` 数据数组必须**恰好 13** 项
//! 3. `error-codes` 的 `categories` 数据与 `properties.categories.items.properties.category.enum`
//!    必须**同序逐项一致** —— 二者在同一文件里重复出现，是最容易漂移的一处
//! 4. `capability-matrix` 必须至少有 1 个 capability
//!
//! ## 退出码
//! 0 通过 | 1 有阻塞级发现项 | 4 IO 错误（由 `main.rs` 映射）

use std::fs;
use std::path::Path;

use crate::serde_json_lite::{self, Value};

/// 期望存在的 schema 文件 + 各自的顶层版本号。
const EXPECTED_SCHEMAS: &[(&str, &str)] = &[
    ("protocol/error-codes/error-codes-1.0.json", "1.0"),
    ("protocol/envelope/envelope-1.0.json", "1.0"),
    ("protocol/tool-schema/tool-schema-1.0.json", "1.0"),
    ("protocol/capability-matrix/capability-1.0.json", "1.0"),
    ("protocol/audit-event/audit-event-1.0.json", "1.0"),
];

/// 不变量 2：v2 §8.7 的错误分类数。
const REQUIRED_CATEGORY_COUNT: usize = 13;

/// 单份 schema 的检查结论。
struct SchemaCheck {
    /// 输出用的路径（成功时是绝对路径，读取失败时是仓库根相对路径）。
    path: String,
    /// JSON 是否解析成功。
    parsed_ok: bool,
    /// 顶层 `version` 是否等于期望值。
    version_ok: bool,
    /// 额外发现项（不变量 2~4）；`None` = 无问题。
    finding: Option<String>,
}

impl SchemaCheck {
    /// 是否通过（三条都满足才算 OK）。
    const fn is_ok(&self) -> bool {
        self.parsed_ok && self.version_ok && self.finding.is_none()
    }
}

/// 主入口：校验全部 schema 并输出报告。
///
/// # 错误
/// 仅在输出写入失败时返回 `Err`；**校验发现项**通过 `Ok(1)` 表达（退出码 1），
/// 因为"schema 不合规"是可预期的检查结论，不是 IO 故障。
pub fn run(repo_root: &Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    let checks: Vec<SchemaCheck> = EXPECTED_SCHEMAS
        .iter()
        .map(|(relative_path, expected_version)| {
            check_schema(repo_root, relative_path, expected_version)
        })
        .collect();

    writeln!(output, "== verify-schemas ==").map_err(|error| error.to_string())?;
    writeln!(output, "scanned_schemas={}", checks.len()).map_err(|error| error.to_string())?;
    let mut error_count = 0usize;
    for check in &checks {
        let status = if check.is_ok() {
            "OK"
        } else {
            error_count += 1;
            "FAIL"
        };
        writeln!(
            output,
            "  [{status}] {} (parsed={} version_ok={} extra={})",
            check.path,
            check.parsed_ok,
            check.version_ok,
            check.finding.as_deref().unwrap_or("-")
        )
        .map_err(|error| error.to_string())?;
    }
    writeln!(output, "-- summary: {error_count} error(s)").map_err(|error| error.to_string())?;
    let verdict = if error_count == 0 { "PASSED" } else { "FAILED" };
    writeln!(output, "-- verdict: {verdict}").map_err(|error| error.to_string())?;
    if error_count == 0 { Ok(0) } else { Ok(1) }
}

/// 校验单份 schema（不变量 1 + 按文件类型追加的不变量 2~4）。
///
/// 任何一步失败都不 panic、不静默：结论写进返回的 [`SchemaCheck`]。
fn check_schema(repo_root: &Path, relative_path: &str, expected_version: &str) -> SchemaCheck {
    let full_path = repo_root.join(relative_path);
    let text = match fs::read_to_string(&full_path) {
        Ok(text) => text,
        Err(error) => {
            return SchemaCheck {
                path: relative_path.to_string(),
                parsed_ok: false,
                version_ok: false,
                finding: Some(format!("read: {error}")),
            };
        }
    };
    let value = match serde_json_lite::parse(&text) {
        Ok(value) => value,
        Err(error) => {
            return SchemaCheck {
                path: relative_path.to_string(),
                parsed_ok: false,
                version_ok: false,
                finding: Some(format!("parse: {error}")),
            };
        }
    };
    let version_ok = value
        .get("version")
        .and_then(Value::as_str)
        .is_some_and(|version| version == expected_version);
    let finding = match relative_path {
        "protocol/error-codes/error-codes-1.0.json" => check_error_codes(&value),
        "protocol/capability-matrix/capability-1.0.json" => check_capability_matrix(&value),
        _ => None,
    };
    SchemaCheck {
        path: full_path.display().to_string(),
        parsed_ok: true,
        version_ok,
        finding,
    }
}

/// 不变量 2 + 3：13 项，且数据数组与 schema 的 `enum` 同序逐项一致。
fn check_error_codes(value: &Value) -> Option<String> {
    let Some(categories) = value.get("categories").and_then(Value::as_array) else {
        return Some("missing 'categories' array".to_string());
    };
    if categories.len() != REQUIRED_CATEGORY_COUNT {
        return Some(format!(
            "categories count = {} (expected {REQUIRED_CATEGORY_COUNT})",
            categories.len()
        ));
    }
    let declared: Vec<&str> = categories
        .iter()
        .filter_map(|entry| entry.get("category").and_then(Value::as_str))
        .collect();
    let enumerated = error_code_enum(value).unwrap_or_default();
    if declared != enumerated {
        return Some(format!(
            "categories data array and properties.categories.items.properties.category.enum disagree \
             (data={declared:?} enum={enumerated:?})"
        ));
    }
    None
}

/// 取出 `properties.categories.items.properties.category.enum`（不变量 3 的右侧）。
fn error_code_enum(value: &Value) -> Option<Vec<&str>> {
    let enum_values = value
        .get("properties")?
        .get("categories")?
        .get("items")?
        .get("properties")?
        .get("category")?
        .get("enum")?
        .as_array()?;
    Some(enum_values.iter().filter_map(Value::as_str).collect())
}

/// 不变量 4：至少 1 个 capability，且 id 不得重复（schema 的 `uniqueItems` 我们不实现，
/// 但"能力目录里出现两条同名能力"是必须拦下的真实缺陷，故在此显式检查）。
fn check_capability_matrix(value: &Value) -> Option<String> {
    let Some(capabilities) = value.get("capabilities").and_then(Value::as_array) else {
        return Some("missing 'capabilities' array".to_string());
    };
    if capabilities.is_empty() {
        return Some("capabilities array is empty".to_string());
    }
    let mut seen: Vec<&str> = Vec::with_capacity(capabilities.len());
    for entry in capabilities {
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            return Some("capability entry missing 'id'".to_string());
        };
        if seen.contains(&id) {
            return Some(format!("duplicate capability id: {id}"));
        }
        seen.push(id);
    }
    None
}
