//! Schema → Rust code rendering helpers for codegen.
//! Per ADR-0035: per-line allows with rationale comments (no sledgehammer).
//!
//! ## Lint policy (per ADR-0035 §决策 2)
//! Per-line allows below target specific lints known to fire from string-builder code:
//! - format_push_string: out.push_str(&format!(...)) is idiomatic vs write!(out, ...)
//! - too_many_lines: each render_* function is one big per-field writer
//! - unnecessary_wraps: render_* never actually fails
//! - missing_const_for_fn: codegen cannot know const-safety
//! - doc_markdown: Chinese doc strings use unbackticked identifiers (ADR-0021)
#![allow(clippy::format_push_string, clippy::too_many_lines, clippy::unnecessary_wraps, clippy::missing_const_for_fn, clippy::doc_markdown)]

use crate::serde_json_lite::Value;

/// Required location of each schema file relative to repo root + target output path.
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

/// Generated-file header. Single-line to keep rustfmt stable.
pub const HEADER: &str = "// GENERATED — DO NOT EDIT. Source: `protocol/**/*.json`. Regenerate via `cargo run -p xtask -- codegen`.\n// Any manual edit here will be detected by `codegen --check` (CI gate).\n";

/// Normalize whitespace: trim trailing whitespace from each line + ensure single trailing newline.
/// Allows edits that only change trailing whitespace to NOT count as drift.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        out.push_str(line.trim_end());
        out.push_str("`n");
    }
    out
}

/// Render one generated file content from a schema file content.
pub fn render(schema_rel: &str, schema_text: &str) -> Result<String, String> {
    let name = std::path::Path::new(schema_rel)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("bad schema path: {schema_rel}"))?;
    let val: Value = crate::serde_json_lite::parse(schema_text)
        .map_err(|e| format!("parse {schema_rel}: {e}"))?;
    let title = val.get("title").and_then(Value::as_str).unwrap_or("");
    let version = val.get("version").and_then(Value::as_str).unwrap_or("");
    match name {
        "error-codes-1.0.json" => render_error_code(&val, title, version),
        "envelope-1.0.json" => render_envelope(&val, title, version),
        "tool-schema-1.0.json" => render_tool_schema(&val, title, version),
        "capability-1.0.json" => render_capability(&val),
        "audit-event-1.0.json" => render_audit_event(&val),
        other => Err(format!("unknown schema file: {other}")),
    }
}

/// Convert snake_case to PascalCase for enum variant names.
/// Convert snake_case to PascalCase, preserving dots (for audit event types like tool.called).
pub fn pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_boundary = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            at_boundary = true;
        } else if ch == '.' {
            at_boundary = true;
            out.push(ch);
        } else if at_boundary {
            out.extend(ch.to_uppercase());
            at_boundary = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Extract an enum variant list from properties.X.enum (a JSON array of strings).
fn extract_enum(val: &Value, parent: &str, prop: &str) -> Result<Vec<String>, String> {
    let props = val
        .get("properties")
        .ok_or_else(|| String::from("schema missing 'properties'"))?;
    let node = if parent.is_empty() {
        props.get(prop)
    } else {
        props
            .get(parent)
            .and_then(Value::as_object)
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.get(prop))
    };
    let node = node.ok_or_else(|| {
        format!(
            "schema missing properties.{}.enum",
            if parent.is_empty() {
                prop.to_string()
            } else {
                format!("{parent}.{prop}")
            },
        )
    })?;
    let enum_arr = node.get("enum").and_then(Value::as_array).ok_or_else(|| {
        format!(
            "properties.{}.enum is not an array",
            if parent.is_empty() {
                prop.to_string()
            } else {
                format!("{parent}.{prop}")
            },
        )
    })?;
    enum_arr
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| String::from("enum value is not a string"))
        })
        .collect()
}

/// Render `generated/error_code.rs` from `protocol/error-codes/error-codes-1.0.json`.
///
/// Schema contract (validated by `verify-schemas`):
/// - top-level `version` (string)
/// - `categories` array with exactly 13 items
/// - each item: `category` (PascalCase enum-name candidate), `retryable` (bool),
///   `message_for_model`, `message_for_user`, `hint`, `evidence_ref` (all non-empty strings)
fn render_error_code(val: &Value, title: &str, version: &str) -> Result<String, String> {
    let categories = val
        .get("categories")
        .and_then(Value::as_array)
        .ok_or_else(|| String::from("missing 'categories' array"))?;
    if categories.len() != 13 {
        return Err(format!(
            "categories count = {} (expected 13)",
            categories.len()
        ));
    }

    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&format!(
        "//! Generated by `xtask codegen` from `protocol/error-codes/error-codes-1.0.json`.\n         //! Schema: {title} v{version}. Per arch v2 section 8.7: 13 error categories. Slashed rows (RateLimited/Timeout, UserCancelled/TookOver) unified into single ErrorCategory variant per row to keep count at 13; the underlying distinction is preserved via evidence_ref and message_for_model content.\n         //! Regenerate via `cargo run -p xtask -- codegen`.
//!
//! ## Lint policy (per ADR-0035)
//! Per-line allows below target specific lints known to fire from string-builder code:
//! - format_push_string: out.push_str(&format!(...)) is idiomatic here vs write!(out, ...)
//! - too_many_lines: each render_* function is one big per-field writer (deliberately dense)
//! - unnecessary_wraps: render_* never actually fails
//! - missing_const_for_fn: codegen cannot know if each emit helper is safe as const fn
//! - doc_markdown: Chinese doc comments use unbackticked identifiers (ADR-0021)
#![allow(clippy::format_push_string, clippy::too_many_lines, clippy::unnecessary_wraps, clippy::missing_const_for_fn, clippy::doc_markdown)]\n\n"
    ));
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str(
        "/// Stable identifier of one of 13 error categories (per arch v2 section 8.7).\n",
    );
    out.push_str("/// New codes require ADR (schema change).\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"PascalCase\")]\n");
    out.push_str("pub enum ErrorCategory {\n");
    for c in categories {
        let name = c
            .get("category")
            .and_then(Value::as_str)
            .ok_or_else(|| String::from("category missing 'category' field"))?;
        out.push_str(&format!("    {name},\n"));
    }
    out.push_str("}\n\n");

    out.push_str("impl ErrorCategory {\n");
    out.push_str("    /// All 13 categories in canonical order (matches schema file).\n");
    out.push_str("    pub const ALL: [ErrorCategory; 13] = [\n");
    for c in categories {
        let name = c.get("category").and_then(Value::as_str).unwrap_or("");
        out.push_str(&format!("        ErrorCategory::{name},\n"));
    }
    out.push_str("    ];\n\n");

    out.push_str("    /// Whether the agent may safely retry this error (per v2 section 8.7 default strategy column).\n");
    out.push_str("    #[must_use]\n");
    out.push_str("    pub const fn retryable(self) -> bool {\n");
    out.push_str("        match self {\n");
    for c in categories {
        let name = c.get("category").and_then(Value::as_str).unwrap_or("");
        let retryable = c
            .get("retryable")
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("category {name} missing 'retryable'"))?;
        out.push_str(&format!(
            "            ErrorCategory::{name} => {retryable},\n"
        ));
    }
    out.push_str("        }\n    }\n}\n\n");

    out.push_str("/// Alias for clarity in envelope errors.\n");
    out.push_str("pub type ErrorCode = ErrorCategory;\n\n");
    out.push_str("/// Error definition with all fields per v2 section 8.7.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct ErrorDefinition {\n");
    out.push_str("    pub category: ErrorCategory,\n");
    out.push_str("    pub retryable: bool,\n");
    out.push_str("    pub message_for_model: String,\n");
    out.push_str("    pub message_for_user: String,\n");
    out.push_str("    pub hint: String,\n");
    out.push_str("    pub evidence_ref: String,\n");
    out.push_str("}\n\n");
    out.push_str("impl ErrorDefinition {\n");
    out.push_str("    #[must_use]\n");
    out.push_str("    pub fn for_category(category: ErrorCategory) -> Self {\n");
    out.push_str("        let (message_for_model, message_for_user, hint, evidence_ref) = match category {\n");
    for c in categories {
        let name = c.get("category").and_then(Value::as_str).unwrap_or("");
        let m_model = c
            .get("message_for_model")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("category {name} missing message_for_model"))?;
        let m_user = c
            .get("message_for_user")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("category {name} missing message_for_user"))?;
        let hint = c
            .get("hint")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("category {name} missing hint"))?;
        let ev = c
            .get("evidence_ref")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("category {name} missing evidence_ref"))?;
        out.push_str(&format!(
            "            ErrorCategory::{name} => (\n                \"{m_model}\",\n                \"{m_user}\",\n                \"{hint}\",\n                \"{ev}\",\n            ),\n"
        ));
    }
    out.push_str("        };\n");
    out.push_str("        Self {\n");
    out.push_str("            retryable: category.retryable(),\n");
    out.push_str("            category,\n");
    out.push_str("            message_for_model: message_for_model.to_string(),\n");
    out.push_str("            message_for_user: message_for_user.to_string(),\n");
    out.push_str("            hint: hint.to_string(),\n");
    out.push_str("            evidence_ref: evidence_ref.to_string(),\n");
    out.push_str("        }\n    }\n}\n");

    Ok(out)
}

/// Render `generated/envelope.rs` from `protocol/envelope/envelope-1.0.json`.
fn render_envelope(val: &Value, title: &str, version: &str) -> Result<String, String> {
    let source_kind = extract_enum(val, "source", "kind")?;
    let truncation_reason = extract_enum(val, "truncated", "reason")?;

    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&format!(
        "//! Generated by `xtask codegen` from `protocol/envelope/envelope-1.0.json`.\n\
         //! Schema: {title} v{version}. Per arch v2 section 5.3: Unified tool return envelope.\n\
         //! Regenerate via `cargo run -p xtask -- codegen`.
//!
//! ## Lint policy (per ADR-0035)
//! Per-line allows below target specific lints known to fire from string-builder code:
//! - format_push_string: out.push_str(&format!(...)) is idiomatic here vs write!(out, ...)
//! - too_many_lines: each render_* function is one big per-field writer (deliberately dense)
//! - unnecessary_wraps: render_* never actually fails
//! - missing_const_for_fn: codegen cannot know if each emit helper is safe as const fn
//! - doc_markdown: Chinese doc comments use unbackticked identifiers (ADR-0021)
#![allow(clippy::format_push_string, clippy::too_many_lines, clippy::unnecessary_wraps, clippy::missing_const_for_fn, clippy::doc_markdown)]\n\n"
    ));
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("use super::error_code::{ErrorCode, ErrorDefinition};\n\n");

    // SourceKind
    out.push_str("/// Source provenance kind. Required when untrusted=true.\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum SourceKind {\n");
    for k in &source_kind {
        let v = pascal_case(k);
        out.push_str(&format!("    {v},\n"));
    }
    out.push_str("}\n\n");
    out.push_str("/// Provenance of the data field.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct Source {\n");
    out.push_str("    pub kind: SourceKind,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub app_id: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub target: Option<String>,\n");
    out.push_str("}\n\n");
    // Truncation
    out.push_str("/// Truncation reason. Required iff data was truncated.\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum TruncationReason {\n");
    for r in &truncation_reason {
        let v = pascal_case(r);
        out.push_str(&format!("    {v},\n"));
    }
    out.push_str("}\n\n");
    out.push_str("/// Truncation metadata.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct Truncation {\n");
    out.push_str("    pub occurred: bool,\n");
    out.push_str("    pub reason: TruncationReason,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub original_bytes: Option<u64>,\n");
    out.push_str("}\n\n");
    out.push_str("/// Evidence per v2 section 7.5: tree snapshot id, screenshot, or any artifact that lets undo/verify/replay reference the tool's post-state.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct Evidence {\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub snapshot_id: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub screenshot: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub extra: Option<serde_json::Value>,\n");
    out.push_str("}\n\n");
    out.push_str("/// Per-tool operational metrics (v2 section 5.3).\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct Metrics {\n");
    out.push_str("    pub duration_ms: u64,\n");
    out.push_str("    #[serde(default = \"default_attempts\")]\n");
    out.push_str("    pub attempts: u32,\n");
    out.push_str("}\n\n");
    out.push_str("fn default_attempts() -> u32 { 1 }\n\n");
    // Error
    out.push_str("/// error field shape.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct EnvelopeError {\n");
    out.push_str("    pub code: ErrorCode,\n");
    out.push_str("    pub message: String,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub evidence_ref: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub details: Option<serde_json::Value>,\n");
    out.push_str("}\n\n");
    // ToolEnvelope
    out.push_str("/// Unified tool return shape (v2 section 5.3).\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct ToolEnvelope {\n");
    out.push_str("    pub version: String,\n");
    out.push_str("    pub tool: String,\n");
    out.push_str("    pub task_id: String,\n");
    out.push_str("    pub step_id: String,\n");
    out.push_str("    pub ok: bool,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub data: Option<EnvelopeData>,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub untrusted: bool,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub source: Option<Source>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub truncated: Option<Truncation>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub evidence: Option<Evidence>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub metrics: Option<Metrics>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub error: Option<EnvelopeError>,\n");
    out.push_str("}\n\n");
    out.push_str("/// data field wrapper.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(transparent)]\n");
    out.push_str("pub struct EnvelopeData(pub serde_json::Value);\n\n");
    // impl
    out.push_str("impl ToolEnvelope {\n");
    out.push_str("    /// Build an ok=true envelope.\n");
    out.push_str("    #[must_use]\n");
    out.push_str("    pub fn ok(tool: String, task_id: String, step_id: String, data: serde_json::Value) -> Self {\n");
    out.push_str("        Self {\n");
    out.push_str("            version: \"1.0\".to_string(),\n");
    out.push_str("            tool,\n");
    out.push_str("            task_id,\n");
    out.push_str("            step_id,\n");
    out.push_str("            ok: true,\n");
    out.push_str("            data: Some(EnvelopeData(data)),\n");
    out.push_str("            untrusted: false,\n");
    out.push_str("            source: None,\n");
    out.push_str("            truncated: None,\n");
    out.push_str("            evidence: None,\n");
    out.push_str("            metrics: None,\n");
    out.push_str("            error: None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n\n");
    out.push_str(
        "    /// Build an ok=false envelope. evidence_ref auto-injected from ErrorDefinition.\n",
    );
    out.push_str("    #[must_use]\n");
    out.push_str("    pub fn error(tool: String, task_id: String, step_id: String, code: ErrorCode, message: impl Into<String>) -> Self {\n");
    out.push_str("        let def = ErrorDefinition::for_category(code);\n");
    out.push_str("        Self {\n");
    out.push_str("            version: \"1.0\".to_string(),\n");
    out.push_str("            tool,\n");
    out.push_str("            task_id,\n");
    out.push_str("            step_id,\n");
    out.push_str("            ok: false,\n");
    out.push_str("            data: None,\n");
    out.push_str("            untrusted: false,\n");
    out.push_str("            source: None,\n");
    out.push_str("            truncated: None,\n");
    out.push_str("            evidence: None,\n");
    out.push_str("            metrics: None,\n");
    out.push_str("            error: Some(EnvelopeError {\n");
    out.push_str("                code,\n");
    out.push_str("                message: message.into(),\n");
    out.push_str("                evidence_ref: Some(def.evidence_ref),\n");
    out.push_str("                details: None,\n");
    out.push_str("            }),\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}

/// Render `generated/tool_schema.rs` from `protocol/tool-schema/tool-schema-1.0.json`.
fn render_tool_schema(val: &Value, title: &str, version: &str) -> Result<String, String> {
    let risk_levels = extract_enum(val, "", "risk_level")?;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&format!(
        "//! Generated by `xtask codegen` from `protocol/tool-schema/tool-schema-1.0.json`.\n\
         //! Schema: {title} v{version}. Per arch v2 section 5.1: Meta-schema for individual tool schemas.\n\n"
    ));
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("/// Per v2 section 10 + ADR-0021: risk level drives policy gating.\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum RiskLevel {\n");
    for r in &risk_levels {
        let v = pascal_case(r);
        out.push_str(&format!("    {v},\n"));
    }
    out.push_str("}\n\n");
    out.push_str("/// Meta-schema for an individual tool.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct ToolSchema {\n");
    out.push_str("    pub version: String,\n");
    out.push_str("    pub name: String,\n");
    out.push_str("    pub description: String,\n");
    out.push_str("    pub input: serde_json::Value,\n");
    out.push_str("    pub output: serde_json::Value,\n");
    out.push_str("    pub risk_level: RiskLevel,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub requires_approval: bool,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub idempotent: bool,\n");
    out.push_str("    #[serde(default, skip_serializing_if = \"Vec::is_empty\")]\n");
    out.push_str("    pub tags: Vec<String>,\n");
    out.push_str("}\n");
    Ok(out)
}

/// Render `generated/capability.rs` from `protocol/capability-matrix/capability-1.0.json`.
fn render_capability(_val: &Value) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(
        "//! Generated by `xtask codegen` from `protocol/capability-matrix/capability-1.0.json`.\n\
         //! Per arch v2 section 13.1.2: Capability identifier catalog.\n\n",
    );
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("/// Stability level.\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum CapabilityStability {\n");
    out.push_str("    Stable,\n    Experimental,\n    Deprecated,\n}\n\n");
    out.push_str("/// Stable capability descriptor.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct Capability {\n");
    out.push_str("    pub id: String,\n");
    out.push_str("    pub description: String,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub stability: Option<CapabilityStability>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub version: Option<String>,\n");
    out.push_str("}\n");
    Ok(out)
}

/// Render `generated/audit_event.rs` from `protocol/audit-event/audit-event-1.0.json`.
fn render_audit_event(val: &Value) -> Result<String, String> {
    let _event_types = extract_enum(val, "", "event_type")?;  // reserved for future per-event-type filtering
    let actors = extract_enum(val, "", "actor")?;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(
        "//! Generated by `xtask codegen` from `protocol/audit-event/audit-event-1.0.json`.\n\
         //! Per arch v2 section 8.x: tamper-evident audit log entry.\n\
         //! Event type naming per naming.md section 7 = noun.past_verb snake_case.\n\
         //! Hash chain uses SHA-256 hex (64 chars).\n\n",
    );
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("use super::envelope::ToolEnvelope;\n\n");
    // Audit event type is a label/category (per naming.md section 7 = noun.past_verb snake_case like tool.called).
    // Stored as String (not enum) so adding new event types in the schema does NOT require
    // a Rust recompile. Schema-level enum validation (via verify-schemas) ensures only known labels.
    out.push_str("/// Audit event type label per naming.md section 7: noun.past_verb snake_case (e.g. \"tool.called\").\n");
    out.push_str("///\n");
    out.push_str(
        "/// Stored as String (not enum) so adding new event types in the schema does NOT\n",
    );
    out.push_str(
        "/// require a Rust recompile. Schema-level enum validation (via verify-schemas) ensures\n",
    );
    out.push_str("/// only known labels are accepted.\n");
    out.push_str("pub type AuditEventType = String;\n\n");
    out.push_str("/// Per arch v2 section 8.x: who triggered this event.\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum AuditActor {\n");
    for a in &actors {
        let v = pascal_case(a);
        out.push_str(&format!("    {v},\n"));
    }
    out.push_str("}\n\n");
    out.push_str("/// Per v2 section 8.x.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct PolicyDecision {\n");
    out.push_str("    pub allow: bool,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub rule_id: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub reason: Option<String>,\n");
    out.push_str("}\n\n");
    out.push_str("/// Cost in tokens + USD. omits Eq (usd is f64, NaN != NaN).\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n");
    out.push_str("pub struct Cost {\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub tokens_in: Option<u64>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub tokens_out: Option<u64>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub usd: Option<f64>,\n");
    out.push_str("}\n\n");
    out.push_str("/// Audit event. omits Eq because nested Cost contains f64.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct AuditEvent {\n");
    out.push_str("    pub version: String,\n");
    out.push_str("    pub event_type: AuditEventType,\n");
    out.push_str("    pub ts: String,\n");
    out.push_str("    pub session_id: String,\n");
    out.push_str("    pub actor: AuditActor,\n");
    out.push_str("    pub action: String,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub target: Option<serde_json::Value>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub args: Option<serde_json::Value>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub result: Option<ToolEnvelope>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub policy_decision: Option<PolicyDecision>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub duration_ms: Option<u64>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub cost: Option<Cost>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub prev_hash: Option<String>,\n");
    out.push_str("    pub self_hash: String,\n");
    out.push_str("}\n");
    Ok(out)
}
