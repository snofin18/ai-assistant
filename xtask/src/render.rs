//! Schema → Rust 代码渲染（`xtask codegen` 的纯逻辑部分，TASK-011）。
//!
//! 职责：把 `protocol/**/*.json`（协议**单一事实源**）渲染成 Rust 源码字符串。
//!
//! ## 边界（不做什么）
//! - 不做 JSON 解析（见 [`crate::serde_json_lite`]）与文件 IO（见 [`crate::codegen`]）
//! - **不跑 rustfmt**：输出必须天生满足 `rustfmt.toml`（`max_width=100` / LF / 4 空格），
//!   否则 `cargo fmt --check` 会在生成物上报红。改本文件时务必按顺序跑：
//!   `cargo run -p xtask -- codegen` → `cargo fmt --all --check` → `cargo run -p xtask -- codegen --check`
//!
//! ## 不变量
//! 1. 每个生成文件首行都是 [`HEADER`]（`GENERATED — DO NOT EDIT`），手改会被 `codegen --check` 抓到
//! 2. 渲染是纯函数：同输入 → 同字节输出（`--check` 才能当硬门禁）
//! 3. 每个生成的公开类型都带 `#[non_exhaustive]`（TASK-011 §必须遵守）
//! 4. 渲染失败一律返回 `Err`，绝不产出半成品（铁律 1：无静默失败）
//!
//! ## Lint 政策（ADR-0035 §替代路径：首选改代码）
//! 本文件**不使用任何** `#[allow]`：插值走 [`emit_line`] 传播 `Result`，
//! 长函数按"一个类型一个 emitter"拆开。模板行刻意打包成少数几个字符串字面量
//! （rustfmt 不会拆字符串，故打包形态是稳定的）。

use std::fmt::Write as _;

use crate::serde_json_lite::Value;

/// 每份 schema 的（仓库根相对路径，生成物仓库根相对路径）。
pub const SCHEMAS: &[(&str, &str)] = &[
    (
        "protocol/error-codes/error-codes-1.0.json",
        "crates/protocol/src/generated/error_code.rs",
    ),
    (
        "protocol/envelope/envelope-1.0.json",
        "crates/protocol/src/generated/envelope.rs",
    ),
    (
        "protocol/tool-schema/tool-schema-1.0.json",
        "crates/protocol/src/generated/tool_schema.rs",
    ),
    (
        "protocol/capability-matrix/capability-1.0.json",
        "crates/protocol/src/generated/capability.rs",
    ),
    (
        "protocol/audit-event/audit-event-1.0.json",
        "crates/protocol/src/generated/audit_event.rs",
    ),
];

/// 生成物文件头。刻意保持单行，避免 rustfmt 重新折行造成假 drift。
pub const HEADER: &str = "// GENERATED — DO NOT EDIT. Source: `protocol/**/*.json`. Regenerate via `cargo run -p xtask -- codegen`.\n// Any manual edit here will be detected by `codegen --check` (CI gate).\n";

/// 写 String 失败时的错误文案。
///
/// `fmt::Write for String` 恒返回 `Ok`，所以这条路径实际不可达；保留它是为了满足
/// 铁律 1 —— 不允许用 `let _ =` 丢弃 `Result`，必须显式传播。
fn write_error() -> String {
    String::from("render: 写入 String 失败（fmt::Error）")
}

/// 写一段**带换行**的文本（插值走 `format_args!`，避免 `push_str(&format!(..))`）。
fn emit_line(out: &mut String, arguments: std::fmt::Arguments<'_>) -> Result<(), String> {
    writeln!(out, "{arguments}").map_err(|_| write_error())
}

/// 归一化：逐行去掉行尾空白，并用 `\n` 重新拼接。
///
/// 用途：`codegen --check` 比对前把两侧都过一遍，于是"只改了尾随空白"不算 drift。
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// 渲染一份 schema 对应的生成物内容。
///
/// # 错误
/// 入参路径无法识别、JSON 解析失败、或缺必需字段时返回 `Err`（带人可读原因）。
pub fn render(schema_relative_path: &str, schema_text: &str) -> Result<String, String> {
    let file_name = std::path::Path::new(schema_relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("bad schema path: {schema_relative_path}"))?;
    let value: Value = crate::serde_json_lite::parse(schema_text)
        .map_err(|error| format!("parse {schema_relative_path}: {error}"))?;
    let title = value.get("title").and_then(Value::as_str).unwrap_or("");
    let version = value.get("version").and_then(Value::as_str).unwrap_or("");
    match file_name {
        "error-codes-1.0.json" => render_error_code(&value, title, version),
        "envelope-1.0.json" => render_envelope(&value, title, version),
        "tool-schema-1.0.json" => render_tool_schema(&value, title, version),
        "capability-1.0.json" => Ok(render_capability()),
        "audit-event-1.0.json" => render_audit_event(&value),
        other => Err(format!("unknown schema file: {other}")),
    }
}

/// `snake_case` / `kebab-case` → `PascalCase`；`.` 视为分段边界并保留（审计事件类型用）。
pub fn pascal_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_boundary = true;
    for character in text.chars() {
        if character == '_' || character == '-' {
            at_boundary = true;
        } else if character == '.' {
            at_boundary = true;
            out.push(character);
        } else if at_boundary {
            out.extend(character.to_uppercase());
            at_boundary = false;
        } else {
            out.push(character);
        }
    }
    out
}

/// 从 `properties.<parent>.properties.<property>.enum` 取字符串枚举值。
///
/// `parent` 为空串表示取值于顶层 `properties`。
fn extract_enum(value: &Value, parent: &str, property: &str) -> Result<Vec<String>, String> {
    let qualified = if parent.is_empty() {
        property.to_string()
    } else {
        format!("{parent}.{property}")
    };
    let properties = value
        .get("properties")
        .ok_or_else(|| String::from("schema missing 'properties'"))?;
    let node = if parent.is_empty() {
        properties.get(property)
    } else {
        properties
            .get(parent)
            .and_then(Value::as_object)
            .and_then(|object| object.get("properties"))
            .and_then(|properties| properties.get(property))
    }
    .ok_or_else(|| format!("schema missing properties.{qualified}"))?;
    let values = node
        .get("enum")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("properties.{qualified}.enum is not an array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| String::from("enum value is not a string"))
        })
        .collect()
}

/// 取顶层数据数组（`categories` / `capabilities` 这类"schema 自带取值表"）。
///
/// `expected` 非空时同时校验元素个数（不变量：错误分类恰好 13 项）。
fn data_array<'a>(
    value: &'a Value,
    key: &str,
    expected: Option<usize>,
) -> Result<&'a Vec<Value>, String> {
    let entries = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing '{key}' array"))?;
    if let Some(expected) = expected
        && entries.len() != expected
    {
        return Err(format!(
            "{key} count = {} (expected {expected})",
            entries.len()
        ));
    }
    Ok(entries)
}

/// 取数组第 `index` 项的字符串字段。
fn entry_str(entries: &[Value], index: usize, key: &str) -> Result<String, String> {
    entries
        .get(index)
        .and_then(|entry| entry.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("entry #{index} missing '{key}'"))
}

/// 取数组第 `index` 项的布尔字段。
fn entry_bool(entries: &[Value], index: usize, key: &str) -> Result<bool, String> {
    entries
        .get(index)
        .and_then(|entry| entry.get(key))
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("entry #{index} missing '{key}'"))
}
/// 渲染 `generated/error_code.rs`（schema：`protocol/error-codes/error-codes-1.0.json`）。
///
/// schema 契约（由 `verify-schemas` 不变量 2/3 守门）：`categories` 恰好 13 项，
/// 且与 `properties.categories.items.properties.category.enum` 同序逐项一致。
fn render_error_code(value: &Value, title: &str, version: &str) -> Result<String, String> {
    let categories = data_array(value, "categories", Some(13))?;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(
        "//! Generated by `xtask codegen` from `protocol/error-codes/error-codes-1.0.json`.\n",
    );
    emit_line(
        &mut out,
        format_args!(
            "//! Schema: {title} v{version}. Per arch v2 section 8.7: 13 error categories."
        ),
    )?;
    out.push_str("//! Slashed rows from v2 (RateLimited/Timeout, UserCancelled/TookOver) are unified into one\n//! ErrorCategory variant per row to keep the count at 13; the underlying distinction is\n//! preserved via evidence_ref and message_for_model content.\n//! Regenerate via `cargo run -p xtask -- codegen`.\n\nuse serde::{Deserialize, Serialize};\n\n");
    emit_error_category(&mut out, categories)?;
    emit_error_definition(&mut out, categories)?;
    Ok(out)
}

/// 渲染 `ErrorCategory` 枚举 + `ALL` 常量 + `retryable()`。
fn emit_error_category(out: &mut String, categories: &[Value]) -> Result<(), String> {
    out.push_str("/// Stable identifier of one of 13 error categories (per arch v2 section 8.7).\n/// New codes require ADR (schema change).\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]\n#[serde(rename_all = \"PascalCase\")]\n#[non_exhaustive]\npub enum ErrorCategory {\n");
    for index in 0..categories.len() {
        emit_line(
            out,
            format_args!("    {},", entry_str(categories, index, "category")?),
        )?;
    }
    out.push_str("}\n\nimpl ErrorCategory {\n    /// All 13 categories in canonical order (matches schema file).\n    pub const ALL: [ErrorCategory; 13] = [\n");
    for index in 0..categories.len() {
        emit_line(
            out,
            format_args!(
                "        ErrorCategory::{},",
                entry_str(categories, index, "category")?
            ),
        )?;
    }
    out.push_str("    ];\n\n    /// Whether the agent may safely retry this error (per v2 section 8.7 default strategy column).\n    #[must_use]\n    pub const fn retryable(self) -> bool {\n        match self {\n");
    for index in 0..categories.len() {
        emit_line(
            out,
            format_args!(
                "            ErrorCategory::{} => {},",
                entry_str(categories, index, "category")?,
                entry_bool(categories, index, "retryable")?
            ),
        )?;
    }
    out.push_str("        }\n    }\n}\n\n");
    Ok(())
}

/// 渲染 `ErrorCode` 别名 + `ErrorDefinition` 及其 `for_category()` 取值表。
fn emit_error_definition(out: &mut String, categories: &[Value]) -> Result<(), String> {
    out.push_str("/// Alias for clarity in envelope errors.\npub type ErrorCode = ErrorCategory;\n\n/// Error definition with all fields per v2 section 8.7.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct ErrorDefinition {\n    pub category: ErrorCategory,\n    pub retryable: bool,\n    pub message_for_model: String,\n    pub message_for_user: String,\n    pub hint: String,\n    pub evidence_ref: String,\n}\n\nimpl ErrorDefinition {\n    #[must_use]\n    pub fn for_category(category: ErrorCategory) -> Self {\n        let (message_for_model, message_for_user, hint, evidence_ref) = match category {\n");
    for index in 0..categories.len() {
        let name = entry_str(categories, index, "category")?;
        emit_line(out, format_args!("            ErrorCategory::{name} => ("))?;
        for key in [
            "message_for_model",
            "message_for_user",
            "hint",
            "evidence_ref",
        ] {
            emit_line(
                out,
                format_args!(
                    "                \"{}\",",
                    entry_str(categories, index, key)?
                ),
            )?;
        }
        out.push_str("            ),\n");
    }
    out.push_str("        };\n        Self {\n            retryable: category.retryable(),\n            category,\n            message_for_model: message_for_model.to_string(),\n            message_for_user: message_for_user.to_string(),\n            hint: hint.to_string(),\n            evidence_ref: evidence_ref.to_string(),\n        }\n    }\n}\n");
    Ok(())
}
/// 渲染 `generated/envelope.rs`（schema：`protocol/envelope/envelope-1.0.json`，v2 §5.3）。
fn render_envelope(value: &Value, title: &str, version: &str) -> Result<String, String> {
    let source_kinds = extract_enum(value, "source", "kind")?;
    let truncation_reasons = extract_enum(value, "truncated", "reason")?;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/envelope/envelope-1.0.json`.\n");
    emit_line(
        &mut out,
        format_args!(
            "//! Schema: {title} v{version}. Per arch v2 section 5.3: Unified tool return envelope."
        ),
    )?;
    out.push_str("//! Regenerate via `cargo run -p xtask -- codegen`.\n\nuse serde::{Deserialize, Serialize};\n\nuse super::error_code::{ErrorCode, ErrorDefinition};\n\n");
    emit_source_kind(&mut out, &source_kinds)?;
    emit_source(&mut out);
    emit_truncation_reason(&mut out, &truncation_reasons)?;
    emit_truncation(&mut out);
    emit_evidence(&mut out);
    emit_metrics(&mut out);
    emit_envelope_error(&mut out);
    emit_tool_envelope(&mut out);
    emit_envelope_data(&mut out);
    emit_tool_envelope_impl(&mut out);
    Ok(out)
}

/// 渲染 `SourceKind` 枚举。
fn emit_source_kind(out: &mut String, source_kinds: &[String]) -> Result<(), String> {
    out.push_str("/// Source provenance kind. Required when untrusted=true.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub enum SourceKind {\n");
    for kind in source_kinds {
        emit_line(out, format_args!("    {},", pascal_case(kind)))?;
    }
    out.push_str("}\n\n");
    Ok(())
}

/// 渲染 `Source` 结构体。
fn emit_source(out: &mut String) {
    out.push_str("/// Provenance of the data field.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct Source {\n    pub kind: SourceKind,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub app_id: Option<String>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub target: Option<String>,\n}\n\n");
}

/// 渲染 `TruncationReason` 枚举。
fn emit_truncation_reason(out: &mut String, reasons: &[String]) -> Result<(), String> {
    out.push_str("/// Truncation reason. Required iff data was truncated.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub enum TruncationReason {\n");
    for reason in reasons {
        emit_line(out, format_args!("    {},", pascal_case(reason)))?;
    }
    out.push_str("}\n\n");
    Ok(())
}

/// 渲染 `Truncation` 结构体。
fn emit_truncation(out: &mut String) {
    out.push_str("/// Truncation metadata.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct Truncation {\n    pub occurred: bool,\n    pub reason: TruncationReason,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub original_bytes: Option<u64>,\n}\n\n");
}

/// 渲染 `Evidence` 结构体（v2 §7.5）。
fn emit_evidence(out: &mut String) {
    out.push_str("/// Evidence per v2 section 7.5: tree snapshot id, screenshot, or any artifact that lets undo/verify/replay reference the tool's post-state.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct Evidence {\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub snapshot_id: Option<String>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub screenshot: Option<String>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub extra: Option<serde_json::Value>,\n}\n\n");
}

/// 渲染 `Metrics` 结构体 + `attempts` 的默认值函数。
///
/// `default_attempts` 必须写成展开形态：rustfmt 会把 `{ 1 }` 拆成多行块体。
fn emit_metrics(out: &mut String) {
    out.push_str("/// Per-tool operational metrics (v2 section 5.3).\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct Metrics {\n    pub duration_ms: u64,\n    #[serde(default = \"default_attempts\")]\n    pub attempts: u32,\n}\n\nfn default_attempts() -> u32 {\n    1\n}\n\n");
}

/// 渲染 `EnvelopeError` 结构体。
fn emit_envelope_error(out: &mut String) {
    out.push_str("/// error field shape.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub struct EnvelopeError {\n    pub code: ErrorCode,\n    pub message: String,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub evidence_ref: Option<String>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub details: Option<serde_json::Value>,\n}\n\n");
}

/// 渲染 `ToolEnvelope` 结构体。
///
/// `data` / `error` **不带** `skip_serializing_if`：v2 §5.3 的 `data` 是必填字段
/// （成功时为对象、失败时为 `null`），`error` 同理显式给 `null`，否则序列化结果
/// 不满足 `envelope-1.0.json` 的 `required` 列表。
fn emit_tool_envelope(out: &mut String) {
    out.push_str("/// Unified tool return shape (v2 section 5.3).\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub struct ToolEnvelope {\n    pub version: String,\n    pub tool: String,\n    pub task_id: String,\n    pub step_id: String,\n    pub ok: bool,\n    pub data: Option<EnvelopeData>,\n    #[serde(default)]\n    pub untrusted: bool,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub source: Option<Source>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub truncated: Option<Truncation>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub evidence: Option<Evidence>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub metrics: Option<Metrics>,\n    pub error: Option<EnvelopeError>,\n}\n\n");
}

/// 渲染 `EnvelopeData` 包装类型 + 构造器。
///
/// `#[non_exhaustive]` 会禁掉下游的元组构造语法，故必须提供 `new`。
fn emit_envelope_data(out: &mut String) {
    out.push_str("/// data field wrapper.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(transparent)]\n#[non_exhaustive]\npub struct EnvelopeData(pub serde_json::Value);\n\nimpl EnvelopeData {\n    /// Wrap a JSON payload for the v2 section 5.3 `data` field.\n    #[must_use]\n    pub fn new(value: serde_json::Value) -> Self {\n        Self(value)\n    }\n}\n\n");
}
/// 渲染 `ToolEnvelope::ok` / `ToolEnvelope::error` 两个构造器。
///
/// 两个签名必须与 rustfmt 稳定形态逐字节一致：`ok` 是 97 列（保持单行），
/// `error` 是 121 列（展开为每参数一行）。
fn emit_tool_envelope_impl(out: &mut String) {
    out.push_str("impl ToolEnvelope {\n    /// Build an ok=true envelope.\n    #[must_use]\n    pub fn ok(tool: String, task_id: String, step_id: String, data: serde_json::Value) -> Self {\n        Self {\n            version: \"1.0\".to_string(),\n            tool,\n            task_id,\n            step_id,\n            ok: true,\n            data: Some(EnvelopeData(data)),\n            untrusted: false,\n            source: None,\n            truncated: None,\n            evidence: None,\n            metrics: None,\n            error: None,\n        }\n    }\n\n    /// Build an ok=false envelope. evidence_ref auto-injected from ErrorDefinition.\n    #[must_use]\n    pub fn error(\n        tool: String,\n        task_id: String,\n        step_id: String,\n        code: ErrorCode,\n        message: impl Into<String>,\n    ) -> Self {\n        let def = ErrorDefinition::for_category(code);\n        Self {\n            version: \"1.0\".to_string(),\n            tool,\n            task_id,\n            step_id,\n            ok: false,\n            data: None,\n            untrusted: false,\n            source: None,\n            truncated: None,\n            evidence: None,\n            metrics: None,\n            error: Some(EnvelopeError {\n                code,\n                message: message.into(),\n                evidence_ref: Some(def.evidence_ref),\n                details: None,\n            }),\n        }\n    }\n}\n");
}

/// 渲染 `generated/tool_schema.rs`（schema：`protocol/tool-schema/tool-schema-1.0.json`）。
fn render_tool_schema(value: &Value, title: &str, version: &str) -> Result<String, String> {
    let risk_levels = extract_enum(value, "", "risk_level")?;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(
        "//! Generated by `xtask codegen` from `protocol/tool-schema/tool-schema-1.0.json`.\n",
    );
    emit_line(
        &mut out,
        format_args!(
            "//! Schema: {title} v{version}. Per arch v2 section 5.1: Meta-schema for individual tool schemas."
        ),
    )?;
    out.push_str("\nuse serde::{Deserialize, Serialize};\n\n/// Per v2 section 10 + ADR-0021: risk level drives policy gating.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub enum RiskLevel {\n");
    for risk_level in &risk_levels {
        emit_line(&mut out, format_args!("    {},", pascal_case(risk_level)))?;
    }
    out.push_str("}\n\n/// Meta-schema for an individual tool.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub struct ToolSchema {\n    pub version: String,\n    pub name: String,\n    pub description: String,\n    pub input: serde_json::Value,\n    pub output: serde_json::Value,\n    pub risk_level: RiskLevel,\n    #[serde(default)]\n    pub requires_approval: bool,\n    #[serde(default)]\n    pub idempotent: bool,\n    #[serde(default, skip_serializing_if = \"Vec::is_empty\")]\n    pub tags: Vec<String>,\n}\n");
    Ok(out)
}

/// 渲染 `generated/capability.rs`（schema：`protocol/capability-matrix/capability-1.0.json`）。
///
/// 能力目录（`capabilities` 数据数组）本身不生成 Rust 常量：它是**运行时数据**，
/// 由 TASK-016 的能力矩阵加载；生成期只负责类型形状。
fn render_capability() -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/capability-matrix/capability-1.0.json`.\n//! Per arch v2 section 13.1.2: Capability identifier catalog.\n\nuse serde::{Deserialize, Serialize};\n\n/// Stability level.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub enum CapabilityStability {\n    Stable,\n    Experimental,\n    Deprecated,\n}\n\n/// Stable capability descriptor.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub struct Capability {\n    pub id: String,\n    pub description: String,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub stability: Option<CapabilityStability>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub version: Option<String>,\n}\n");
    out
}

/// 渲染 `generated/audit_event.rs`（schema：`protocol/audit-event/audit-event-1.0.json`）。
fn render_audit_event(value: &Value) -> Result<String, String> {
    // 事件类型在 schema 里是点分标签（如 `tool.called`），不是合法 Rust 标识符，
    // 故只做存在性校验、不生成枚举；合法标签由 schema 的 enum 约束（verify-schemas 守门）。
    let _event_types = extract_enum(value, "", "event_type")?;
    let actors = extract_enum(value, "", "actor")?;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/audit-event/audit-event-1.0.json`.\n//! Per arch v2 section 8.x: tamper-evident audit log entry.\n//! Event type naming per naming.md section 7 = noun.past_verb snake_case.\n//! Hash chain uses SHA-256 hex (64 chars).\n\nuse serde::{Deserialize, Serialize};\n\nuse super::envelope::ToolEnvelope;\n\n/// Audit event type label per naming.md section 7: noun.past_verb snake_case (e.g. \"tool.called\").\n///\n/// Stored as String (not enum) so adding new event types in the schema does NOT\n/// require a Rust recompile. Schema-level enum validation (via verify-schemas) ensures\n/// only known labels are accepted.\npub type AuditEventType = String;\n\n/// Per arch v2 section 8.x: who triggered this event.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub enum AuditActor {\n");
    for actor in &actors {
        emit_line(&mut out, format_args!("    {},", pascal_case(actor)))?;
    }
    out.push_str("}\n\n/// Per v2 section 8.x.\n#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct PolicyDecision {\n    pub allow: bool,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub rule_id: Option<String>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub reason: Option<String>,\n}\n\n/// Cost in tokens + USD. omits Eq (usd is f64, NaN != NaN).\n#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[non_exhaustive]\npub struct Cost {\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub tokens_in: Option<u64>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub tokens_out: Option<u64>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub usd: Option<f64>,\n}\n\n/// Audit event. omits Eq because nested Cost contains f64.\n#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\n#[non_exhaustive]\npub struct AuditEvent {\n    pub version: String,\n    pub event_type: AuditEventType,\n    pub ts: String,\n    pub session_id: String,\n    pub actor: AuditActor,\n    pub action: String,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub target: Option<serde_json::Value>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub args: Option<serde_json::Value>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub result: Option<ToolEnvelope>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub policy_decision: Option<PolicyDecision>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub duration_ms: Option<u64>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub cost: Option<Cost>,\n    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub prev_hash: Option<String>,\n    pub self_hash: String,\n}\n");
    Ok(out)
}
