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
//! 1. 六份 schema 必须存在 + JSON 合法 + 顶层 `version` = 期望值
//! 2. `error-codes` 的 `categories` 数据数组必须**恰好 13** 项
//! 3. `error-codes` 的 `categories` 数据与 `properties.categories.items.properties.category.enum`
//!    必须**同序逐项一致** —— 二者在同一文件里重复出现，是最容易漂移的一处
//! 4. `capability-matrix` 必须至少有 1 个 capability
//!
//! ## 退出码
//! 0 通过 | 1 有阻塞级发现项 | 4 IO 错误（由 `main.rs` 映射）

use std::fs;
use std::path::Path;

use crate::adapter_catalog_check::check_adapter_catalog;
use crate::serde_json_lite::{self, Value};

/// 期望存在的 schema 文件 + 各自的顶层版本号。
const EXPECTED_SCHEMAS: &[(&str, &str)] = &[
    ("protocol/error-codes/error-codes-1.0.json", "1.0"),
    ("protocol/envelope/envelope-1.0.json", "1.0"),
    ("protocol/tool-schema/tool-schema-1.0.json", "1.0"),
    ("protocol/tool-schema/tool-schema-1.1.json", "1.1"),
    ("protocol/capability-matrix/capability-1.0.json", "1.0"),
    ("protocol/audit-event/audit-event-1.0.json", "1.0"),
    ("protocol/adapter-catalog/adapter-catalog-1.0.json", "1.0"),
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
        "protocol/tool-schema/tool-schema-1.0.json" => check_tool_schema_v10(&value),
        "protocol/tool-schema/tool-schema-1.1.json" => check_tool_schema_v11(&value),
        "protocol/adapter-catalog/adapter-catalog-1.0.json" => check_adapter_catalog(&value),
        _ => None,
    };
    SchemaCheck {
        path: full_path.display().to_string(),
        parsed_ok: true,
        version_ok,
        finding,
    }
}

/// `ToolSchema` 1.0 remains the compatibility baseline and must not gain 1.1 fields.
fn check_tool_schema_v10(value: &Value) -> Option<String> {
    if let Some(error) = check_tool_schema_v10_required_fields(value) {
        return Some(error);
    }
    let Some(properties) = value.get("properties") else {
        return Some("ToolSchema 1.0 missing properties object".to_owned());
    };
    if let Some(error) = check_tool_schema_v10_property_shape(properties) {
        return Some(error);
    }
    if let Some(error) = check_tool_schema_v10_literals(properties) {
        return Some(error);
    }
    if value.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
        return Some("ToolSchema 1.0 additionalProperties must be false".to_owned());
    }
    None
}

fn check_tool_schema_v10_required_fields(value: &Value) -> Option<String> {
    let Some(required) = value.get("required").and_then(Value::as_array) else {
        return Some("ToolSchema 1.0 missing required array".to_owned());
    };
    for field in [
        "version",
        "name",
        "description",
        "input",
        "output",
        "effect",
        "reversibility",
        "risk_level",
    ] {
        if !required.iter().any(|entry| entry.as_str() == Some(field)) {
            return Some(format!("ToolSchema 1.0 missing required field {field}"));
        }
    }
    None
}

fn check_tool_schema_v10_property_shape(properties: &Value) -> Option<String> {
    let Some(property_map) = properties.as_object() else {
        return Some("ToolSchema 1.0 properties must be an object".to_owned());
    };
    if property_map.len() != 11 {
        return Some(format!(
            "ToolSchema 1.0 must define exactly 11 properties, found {}",
            property_map.len()
        ));
    }
    for field in [
        "version",
        "name",
        "description",
        "input",
        "output",
        "effect",
        "reversibility",
        "risk_level",
        "requires_approval",
        "idempotent",
        "tags",
    ] {
        if properties.get(field).is_none() {
            return Some(format!("ToolSchema 1.0 missing property {field}"));
        }
    }
    for (field, expected_type) in [
        ("version", "string"),
        ("name", "string"),
        ("description", "string"),
        ("input", "object"),
        ("effect", "string"),
        ("reversibility", "string"),
        ("risk_level", "string"),
        ("requires_approval", "boolean"),
        ("idempotent", "boolean"),
        ("tags", "array"),
    ] {
        if properties
            .get(field)
            .and_then(|property| property.get("type"))
            .and_then(Value::as_str)
            != Some(expected_type)
        {
            return Some(format!(
                "ToolSchema 1.0 property {field} type must be {expected_type}"
            ));
        }
    }
    None
}

fn check_tool_schema_v10_literals(properties: &Value) -> Option<String> {
    if properties
        .get("version")
        .and_then(|version| version.get("const"))
        .and_then(Value::as_str)
        != Some("1.0")
    {
        return Some("ToolSchema 1.0 version const must be 1.0".to_owned());
    }
    if properties
        .get("output")
        .and_then(|output| output.get("$ref"))
        .and_then(Value::as_str)
        != Some("../envelope/envelope-1.0.json")
    {
        return Some("ToolSchema 1.0 output must reference envelope-1.0.json".to_owned());
    }
    if !property_enum_matches(properties, "effect", &["read", "write"]) {
        return Some("ToolSchema 1.0 effect enum mismatch".to_owned());
    }
    if !property_enum_matches(
        properties,
        "reversibility",
        &[
            "l0_undo_stack",
            "l1_snapshot",
            "l2_compensation",
            "l3_irreversible",
        ],
    ) {
        return Some("ToolSchema 1.0 reversibility enum mismatch".to_owned());
    }
    if !property_enum_matches(
        properties,
        "risk_level",
        &["low", "medium", "high", "critical"],
    ) {
        return Some("ToolSchema 1.0 risk_level enum mismatch".to_owned());
    }
    None
}

fn property_enum_matches(properties: &Value, field: &str, expected: &[&str]) -> bool {
    let Some(actual) = properties
        .get(field)
        .and_then(|property| property.get("enum"))
        .and_then(Value::as_array)
    else {
        return false;
    };
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(entry, expected)| entry.as_str() == Some(*expected))
}

/// `ToolSchema` 1.1：支持 `none_readonly`，但只允许与 `effect=read` 组合；
/// 同时必须在同一 schema 中声明 `AdapterToolEnvelopeV1` 的三个字段。
fn check_tool_schema_v11(value: &Value) -> Option<String> {
    if let Some(error) = check_tool_schema_v11_header(value) {
        return Some(error);
    }
    if let Some(error) = check_none_readonly_rule(value) {
        return Some(error);
    }
    check_adapter_envelope_v11(value)
}

fn check_tool_schema_v11_header(value: &Value) -> Option<String> {
    let Some(required) = value.get("required").and_then(Value::as_array) else {
        return Some("ToolSchema 1.1 missing required array".to_owned());
    };
    for field in [
        "version",
        "name",
        "description",
        "input",
        "output",
        "effect",
        "reversibility",
        "risk_level",
    ] {
        if !required.iter().any(|entry| entry.as_str() == Some(field)) {
            return Some(format!("ToolSchema 1.1 missing required field {field}"));
        }
    }
    let Some(properties) = value.get("properties") else {
        return Some("missing properties object".to_owned());
    };
    if properties
        .get("version")
        .and_then(|version| version.get("const"))
        .and_then(Value::as_str)
        != Some("1.1")
    {
        return Some("ToolSchema 1.1 version const must be 1.1".to_owned());
    }
    None
}

fn check_none_readonly_rule(value: &Value) -> Option<String> {
    let Some(properties) = value.get("properties") else {
        return Some("missing properties object".to_owned());
    };
    let Some(reversibility) = properties
        .get("reversibility")
        .and_then(|reversibility| reversibility.get("enum"))
        .and_then(Value::as_array)
    else {
        return Some("missing properties.reversibility.enum".to_owned());
    };
    if !reversibility
        .iter()
        .any(|entry| entry.as_str() == Some("none_readonly"))
    {
        return Some("reversibility enum is missing none_readonly".to_owned());
    }
    let none_rule = value
        .get("allOf")
        .and_then(Value::as_array)
        .and_then(|rules| rules.first())
        .is_some_and(|rule| {
            rule.get("if")
                .and_then(|condition| condition.get("properties"))
                .and_then(|properties| properties.get("reversibility"))
                .and_then(|reversibility| reversibility.get("const"))
                .and_then(Value::as_str)
                == Some("none_readonly")
                && rule
                    .get("then")
                    .and_then(|condition| condition.get("properties"))
                    .and_then(|properties| properties.get("effect"))
                    .and_then(|effect| effect.get("const"))
                    .and_then(Value::as_str)
                    == Some("read")
        });
    if !none_rule {
        return Some("none_readonly must require effect=read".to_owned());
    }
    None
}

fn check_adapter_envelope_v11(value: &Value) -> Option<String> {
    let Some(envelope) = value
        .get("definitions")
        .and_then(|definitions| definitions.get("AdapterToolEnvelopeV1"))
    else {
        return Some("missing definitions.AdapterToolEnvelopeV1".to_owned());
    };
    let Some(required) = envelope.get("required").and_then(Value::as_array) else {
        return Some("AdapterToolEnvelopeV1 missing required array".to_owned());
    };
    for field in ["schema", "capabilities", "postconditions"] {
        if !required.iter().any(|entry| entry.as_str() == Some(field)) {
            return Some(format!(
                "AdapterToolEnvelopeV1 missing required field {field}"
            ));
        }
    }
    let Some(properties) = envelope.get("properties") else {
        return Some("AdapterToolEnvelopeV1 missing properties".to_owned());
    };
    let capabilities_min_items = properties
        .get("capabilities")
        .and_then(|field| field.get("minItems"))
        .is_some_and(number_is_one);
    if !capabilities_min_items {
        return Some("AdapterToolEnvelopeV1 capabilities minItems must be 1".to_owned());
    }
    let postconditions_min_items = properties
        .get("postconditions")
        .and_then(|field| field.get("minItems"))
        .is_some_and(number_is_one);
    if !postconditions_min_items {
        return Some("AdapterToolEnvelopeV1 postconditions minItems must be 1".to_owned());
    }
    None
}

const fn number_is_one(value: &Value) -> bool {
    matches!(value, Value::Integer(1))
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! 负向验证（ADR-0019 形式 N1）：**喂坏输入必须产生阻塞级发现项**。
    //!
    //! 只证明"这次跑绿了"不算验收 —— 必须同时证明"该红的时候会红"。
    //! 本模块不碰磁盘（除最后一条显式验证"读不到就是错"），全部是纯逻辑。

    use super::*;

    /// error-codes 夹具模板：`__DATA__` = 数据数组，`__ENUM__` = schema 的 enum 数组。
    /// 用占位符替换而不是 `format!`，避免在 JSON 里逃逸大括号（可读性优先）。
    const ERROR_CODES_TEMPLATE: &str = r#"{"categories":[__DATA__],"properties":{"categories":{"items":{"properties":{"category":{"enum":[__ENUM__]}}}}}}"#;
    const CAPABILITY_TEMPLATE: &str = r#"{"capabilities":[__DATA__]}"#;
    const TOOL_SCHEMA_V10_TEMPLATE: &str = r#"{"required":["version","name","description","input","output","effect","reversibility","risk_level"],"properties":{"version":{"type":"string","const":"1.0"},"name":{"type":"string"},"description":{"type":"string"},"input":{"type":"object"},"output":{"$ref":"../envelope/envelope-1.0.json"},"effect":{"type":"string","enum":["read","write"]},"reversibility":{"type":"string","enum":["l0_undo_stack","l1_snapshot","l2_compensation","l3_irreversible"]},"risk_level":{"type":"string","enum":["low","medium","high","critical"]},"requires_approval":{"type":"boolean"},"idempotent":{"type":"boolean"},"tags":{"type":"array"}},"additionalProperties":false}"#;
    const TOOL_SCHEMA_V11_TEMPLATE: &str = r#"{"required":["version","name","description","input","output","effect","reversibility","risk_level"],"properties":{"version":{"const":"1.1"},"effect":{"enum":["read","write"]},"reversibility":{"enum":["l0_undo_stack","none_readonly"]}},"allOf":[__RULE__],"definitions":{"AdapterToolEnvelopeV1":{"required":["schema","capabilities","postconditions"],"properties":{"capabilities":{"minItems":1},"postconditions":{"minItems":1}}}}}"#;
    const NONE_READONLY_RULE: &str = r#"{"if":{"properties":{"reversibility":{"const":"none_readonly"}}},"then":{"properties":{"effect":{"const":"read"}}}}"#;

    fn category_entries(names: &[&str]) -> String {
        names
            .iter()
            .map(|name| format!(r#"{{"category":"{name}"}}"#))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn quoted(names: &[&str]) -> String {
        names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn error_codes_value(data: &[&str], enum_names: &[&str]) -> Value {
        let json = ERROR_CODES_TEMPLATE
            .replace("__DATA__", &category_entries(data))
            .replace("__ENUM__", &quoted(enum_names));
        serde_json_lite::parse(&json).expect("测试夹具必须是合法 JSON")
    }

    fn capability_value(ids: &[&str]) -> Value {
        let data = ids
            .iter()
            .map(|id| format!(r#"{{"id":"{id}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let json = CAPABILITY_TEMPLATE.replace("__DATA__", &data);
        serde_json_lite::parse(&json).expect("测试夹具必须是合法 JSON")
    }

    fn adapter_catalog_value() -> Value {
        serde_json_lite::parse(include_str!(
            "../../protocol/adapter-catalog/adapter-catalog-1.0.json"
        ))
        .expect("generated adapter catalog must parse")
    }

    fn tool_schema_v11_value() -> Value {
        let json = TOOL_SCHEMA_V11_TEMPLATE.replace("__RULE__", NONE_READONLY_RULE);
        serde_json_lite::parse(&json).expect("ToolSchema 1.1 fixture must parse")
    }

    fn tool_schema_v10_value() -> Value {
        serde_json_lite::parse(TOOL_SCHEMA_V10_TEMPLATE).expect("ToolSchema 1.0 fixture must parse")
    }

    /// 13 类且数据数组与 enum 同序 → 无发现项（正向基线，证明断言不是恒真）。
    #[test]
    fn test_error_codes_accepts_aligned_data_and_enum() {
        let names = [
            "C0", "C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10", "C11", "C12",
        ];
        let value = error_codes_value(&names, &names);
        assert!(
            check_error_codes(&value).is_none(),
            "同序一致的 13 类不应有发现项"
        );
    }

    /// 负向：只有 12 类 → 必须报出数量不符。
    #[test]
    fn test_error_codes_rejects_wrong_category_count() {
        let names = [
            "C0", "C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10", "C11",
        ];
        let value = error_codes_value(&names, &names);
        let finding = check_error_codes(&value).expect("12 类必须被拦下");
        assert!(
            finding.contains("count = 12"),
            "发现项必须点明实际数量，实际：{finding}"
        );
    }

    /// 负向：数据数组与 schema 的 enum 不同序 → 必须报出不一致（不变量 3）。
    #[test]
    fn test_error_codes_rejects_data_enum_disagreement() {
        let data = [
            "C0", "C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10", "C11", "C12",
        ];
        // 把最后两项对调：数量仍是 13，但同序一致性被破坏。
        let mut enum_names = data;
        enum_names.swap(11, 12);
        let value = error_codes_value(&data, &enum_names);
        let finding = check_error_codes(&value).expect("数据数组与 enum 不一致必须被拦下");
        assert!(
            finding.contains("disagree"),
            "发现项必须点明两侧不一致，实际：{finding}"
        );
    }

    /// 负向：`categories` 整个缺失 → 必须报出缺失，而不是当作空数组放过。
    #[test]
    fn test_error_codes_rejects_missing_categories() {
        let value = serde_json_lite::parse(r#"{"version":"1.0"}"#).expect("夹具合法");
        let finding = check_error_codes(&value).expect("缺 categories 必须被拦下");
        assert!(finding.contains("missing"), "实际：{finding}");
    }

    /// capability：id 互不相同 → 无发现项（正向基线）。
    #[test]
    fn test_capability_matrix_accepts_unique_ids() {
        let value = capability_value(&["target.list", "target.resolve"]);
        assert!(check_capability_matrix(&value).is_none());
    }

    /// 负向：空目录 → 必须报出（否则"一条能力都没有"会被当成通过）。
    #[test]
    fn test_capability_matrix_rejects_empty() {
        let value = capability_value(&[]);
        let finding = check_capability_matrix(&value).expect("空 capabilities 必须被拦下");
        assert!(finding.contains("empty"), "实际：{finding}");
    }

    /// 负向：重复 id → 必须报出（schema 的 `uniqueItems` 我们没实现，所以这条是唯一防线）。
    #[test]
    fn test_capability_matrix_rejects_duplicate_id() {
        let value = capability_value(&["target.list", "target.list"]);
        let finding = check_capability_matrix(&value).expect("重复 id 必须被拦下");
        assert!(finding.contains("duplicate"), "实际：{finding}");
    }

    #[test]
    fn test_adapter_catalog_accepts_generated_catalog() {
        assert!(check_adapter_catalog(&adapter_catalog_value()).is_none());
    }

    #[test]
    fn test_adapter_catalog_rejects_missing_capabilities() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        object.remove("capabilities");
        let finding = check_adapter_catalog(&value).expect("missing capabilities must fail");
        assert!(finding.contains("capabilities"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_unknown_top_level_field() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        object.insert("unknown".to_string(), Value::Bool(true));
        let finding = check_adapter_catalog(&value).expect("unknown top-level field must fail");
        assert!(finding.contains("unknown top-level"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_unsorted_capabilities() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        let Some(Value::Array(capabilities)) = object.get_mut("capabilities") else {
            panic!("capabilities must be an array");
        };
        capabilities.swap(0, 1);
        let finding = check_adapter_catalog(&value).expect("unsorted capabilities must fail");
        assert!(finding.contains("byte-order sorted"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_missing_postcondition_kind() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        let Some(Value::Array(postconditions)) = object.get_mut("postconditions") else {
            panic!("postconditions must be an array");
        };
        let Some(Value::Object(first)) = postconditions.first_mut() else {
            panic!("postconditions must contain an object");
        };
        first.remove("kind");
        let finding = check_adapter_catalog(&value).expect("missing kind must fail");
        assert!(finding.contains("kind"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_unknown_postcondition_field() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        let Some(Value::Array(postconditions)) = object.get_mut("postconditions") else {
            panic!("postconditions must be an array");
        };
        let Some(Value::Object(first)) = postconditions.first_mut() else {
            panic!("postconditions must contain an object");
        };
        first.insert("unexpected".to_string(), Value::Bool(true));
        let finding = check_adapter_catalog(&value).expect("unknown field must fail");
        assert!(finding.contains("unknown field"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_unknown_field_type_vocabulary() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        let Some(Value::Array(postconditions)) = object.get_mut("postconditions") else {
            panic!("postconditions must be an array");
        };
        let Some(Value::Object(first)) = postconditions.first_mut() else {
            panic!("postconditions must contain an object");
        };
        let Some(Value::Object(field_types)) = first.get_mut("field_types") else {
            panic!("field_types must be an object");
        };
        let Some(first_type) = field_types.values_mut().next() else {
            panic!("field_types must be non-empty");
        };
        *first_type = Value::String("made_up_type".to_string());
        let finding = check_adapter_catalog(&value).expect("unknown field type must fail");
        assert!(finding.contains("not recognized"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_missing_constraints_array() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        let Some(Value::Array(postconditions)) = object.get_mut("postconditions") else {
            panic!("postconditions must be an array");
        };
        let Some(Value::Object(first)) = postconditions.first_mut() else {
            panic!("postconditions must contain an object");
        };
        first.remove("constraints");
        let finding = check_adapter_catalog(&value).expect("missing constraints must fail");
        assert!(finding.contains("constraints"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_rejects_unknown_constraint_tag() {
        let mut value = adapter_catalog_value();
        let Value::Object(object) = &mut value else {
            panic!("adapter catalog root must be an object");
        };
        let Some(Value::Array(postconditions)) = object.get_mut("postconditions") else {
            panic!("postconditions must be an array");
        };
        let Some(Value::Object(first)) = postconditions.first_mut() else {
            panic!("postconditions must contain an object");
        };
        first.insert(
            "constraints".to_string(),
            Value::Array(vec![
                serde_json_lite::parse(r#"{"kind":"unknown_constraint"}"#)
                    .expect("constraint fixture"),
            ]),
        );
        let finding = check_adapter_catalog(&value).expect("unknown constraint must fail");
        assert!(finding.contains("unknown constraint kind"), "{finding}");
    }

    #[test]
    fn test_adapter_catalog_accepts_fingerprint_constraint_case_type() {
        let value = adapter_catalog_value();
        let state_assert = value
            .get("postconditions")
            .and_then(Value::as_array)
            .and_then(|postconditions| {
                postconditions
                    .iter()
                    .find(|entry| entry.get("kind").and_then(Value::as_str) == Some("state_assert"))
            })
            .expect("state_assert catalog entry");
        let fingerprint_case = state_assert
            .get("constraints")
            .and_then(Value::as_array)
            .and_then(|constraints| constraints.first())
            .and_then(|constraint| constraint.get("cases"))
            .and_then(|cases| cases.get("fingerprint"))
            .and_then(Value::as_str);
        assert_eq!(fingerprint_case, Some("fingerprint"));
        assert!(check_adapter_catalog(&value).is_none());
    }

    /// `ToolSchema` 1.1 正向基线：`none_readonly` 与只读规则、envelope 字段齐全。
    #[test]
    fn test_tool_schema_v11_accepts_complete_contract() {
        assert!(check_tool_schema_v11(&tool_schema_v11_value()).is_none());
    }

    /// 负向：`write + none_readonly` 的跨字段限制被删掉时必须报错。
    #[test]
    fn test_tool_schema_v11_rejects_missing_none_readonly_rule() {
        let json = TOOL_SCHEMA_V11_TEMPLATE.replace("__RULE__", "{}");
        let value = serde_json_lite::parse(&json).expect("fixture must parse");
        let finding = check_tool_schema_v11(&value).expect("missing rule must fail");
        assert!(finding.contains("effect=read"), "{finding}");
    }

    /// 负向：envelope 定义缺失时必须报错，不能把“没有 envelope”当成 1.0 兼容通过。
    #[test]
    fn test_tool_schema_v11_rejects_missing_adapter_envelope() {
        let json = TOOL_SCHEMA_V11_TEMPLATE
            .replace("__RULE__", NONE_READONLY_RULE)
            .replace("AdapterToolEnvelopeV1", "OtherEnvelope");
        let value = serde_json_lite::parse(&json).expect("fixture must parse");
        let finding = check_tool_schema_v11(&value).expect("missing envelope must fail");
        assert!(finding.contains("AdapterToolEnvelopeV1"), "{finding}");
    }

    #[test]
    fn test_tool_schema_v11_rejects_wrong_version_const() {
        let json = TOOL_SCHEMA_V11_TEMPLATE
            .replace("__RULE__", NONE_READONLY_RULE)
            .replace("\"const\":\"1.1\"", "\"const\":\"2.0\"");
        let value = serde_json_lite::parse(&json).expect("fixture must parse");
        let finding = check_tool_schema_v11(&value).expect("wrong schema version must fail");
        assert!(finding.contains("version const"), "{finding}");
    }

    #[test]
    fn test_tool_schema_v10_rejects_invalid_shape() {
        let valid = tool_schema_v10_value();
        assert!(check_tool_schema_v10(&valid).is_none());

        let with_none_text = TOOL_SCHEMA_V10_TEMPLATE.replace("l3_irreversible", "none_readonly");
        assert!(with_none_text.contains("none_readonly"));
        let with_none = serde_json_lite::parse(&with_none_text).expect("fixture must parse");
        let finding = check_tool_schema_v10(&with_none).expect("1.1 field must fail");
        assert!(finding.contains("reversibility enum"), "{finding}");

        let missing_required =
            serde_json_lite::parse(r#"{"properties":{"reversibility":{"enum":[]}}}"#)
                .expect("fixture must parse");
        assert!(check_tool_schema_v10(&missing_required).is_some());

        let wrong_type = serde_json_lite::parse(&TOOL_SCHEMA_V10_TEMPLATE.replace(
            r#""version":{"type":"string""#,
            r#""version":{"type":"number""#,
        ))
        .expect("fixture must parse");
        let finding = check_tool_schema_v10(&wrong_type).expect("wrong property type must fail");
        assert!(finding.contains("property version type"), "{finding}");

        let open_object = serde_json_lite::parse(&TOOL_SCHEMA_V10_TEMPLATE.replace(
            r#""additionalProperties":false"#,
            r#""additionalProperties":true"#,
        ))
        .expect("fixture must parse");
        let finding = check_tool_schema_v10(&open_object).expect("open property map must fail");
        assert!(finding.contains("additionalProperties"), "{finding}");
    }

    /// 负向（真仓库形态）：仓库根指向一个不存在的目录 → 6 份 schema 全读不到
    /// → 必须是阻塞级退出码 1，而不是 0（铁律 1：读不到 ≠ 通过）。
    #[test]
    fn test_run_returns_blocking_exit_code_when_schemas_unreadable() {
        let missing = Path::new("Z:/definitely-not-a-directory-verify-schemas");
        let mut output: Vec<u8> = Vec::new();
        let code = run(missing, &mut output).expect("读不到 schema 是校验结论，不是 IO 故障");
        assert_eq!(code, 1, "读不到 schema 必须返回阻塞级退出码 1");
        let text = String::from_utf8(output).expect("输出必须是 UTF-8");
        assert!(
            text.contains("verdict: FAILED"),
            "必须显式打印 FAILED，实际输出：{text}"
        );
    }
}
