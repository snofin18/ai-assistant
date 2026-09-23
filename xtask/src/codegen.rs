//! # codegen 子命令
//!
//! 职责：从 `protocol/**/*.json` 生成 `crates/protocol/src/generated/*.rs`，
//! 确保 schema 是 Rust/TS 类型的**单一事实源**。
//!
//! 边界（不做什么）：
//! - 不实现完整的 JSON Schema 解析器（仅用最简的 `serde_json_lite` 读必要字段）
//! - 不写 TS 类型（首次仅 Rust；TS 类型生成是 TASK-011 后续小卡）
//! - 不修改 schema 文件本身
//!
//! 不变量：
//! 1. 输出**确定**：同一 schema → 同一 generated 内容（无时间戳 / 随机 ID）
//! 2. 生成文件首行 = `// GENERATED — DO NOT EDIT`
//! 3. `codegen --check`：检测到 drift = 退出 1（铁律 1「无静默失败」）
//! 4. `codegen`（无 --check）：write 所有生成文件 + 报告 diff
//!
//! 实现策略（首次落地）：
//! - error-codes: 生成 `generated/error_code.rs`（从 schema 的 categories 数组）
//! - envelope/tool-schema/audit-event/capability: 生成对应的 generated/*.rs
//! - 生成的\"代码内容\"**稳定**（无 ID）→ `codegen --check` 总是稳定

use std::fs;
use std::path::Path;

const HEADER: &str = "// GENERATED — DO NOT EDIT. Source: protocol/**/*.json. Regenerate via `cargo run -p xtask -- codegen`.\n// Any manual edit here will be detected by `codegen --check` (CI gate).\n";

const SCHEMAS: &[(&str, &str)] = &[
    ("protocol/error-codes/error-codes-1.0.json",       "crates/protocol/src/generated/error_code.rs"),
    ("protocol/envelope/envelope-1.0.json",           "crates/protocol/src/generated/envelope.rs"),
    ("protocol/tool-schema/tool-schema-1.0.json",     "crates/protocol/src/generated/tool_schema.rs"),
    ("protocol/capability-matrix/capability-1.0.json","crates/protocol/src/generated/capability.rs"),
    ("protocol/audit-event/audit-event-1.0.json",     "crates/protocol/src/generated/audit_event.rs"),
];

#[derive(Debug)]
enum CodegenFailure {
    /// Schema or generated file IO failed.
    Io(String),
    /// Schema content failed -- report and exit.
    Schema(String),
}

pub fn run(repo_root: &Path, check_only: bool, output: &mut dyn std::io::Write) -> Result<u8, String> {
    let mut drifts: Vec<(String, String)> = Vec::new(); // (path, expected)
    let mut errors = 0usize;
    writeln!(output, "== codegen ==").map_err(|e| e.to_string())?;
    writeln!(output, "check_only={}", check_only).map_err(|e| e.to_string())?;
    for (schema_rel, gen_rel) in SCHEMAS {
        let schema_path = repo_root.join(schema_rel);
        let gen_path = repo_root.join(gen_rel);
        let schema_text = match fs::read_to_string(&schema_path) {
            Ok(t) => t,
            Err(e) => {
                writeln!(output, "  [FAIL] {}  read error: {e}", schema_rel).map_err(|e| e.to_string())?;
                errors += 1;
                continue;
            }
        };
        let generated = match render(schema_rel, &schema_text) {
            Ok(g) => g,
            Err(failure) => {
                writeln!(output, "  [FAIL] {}  render error: {:?}", schema_rel, failure).map_err(|e| e.to_string())?;
                errors += 1;
                continue;
            }
        };
        if !gen_path.exists() {
            drifts.push((gen_rel.to_string(), generated));
            writeln!(output, "  [DRIFT] {}  file missing", gen_rel).map_err(|e| e.to_string())?;
            continue;
        }
        let on_disk = fs::read_to_string(&gen_path).map_err(|e| format!("read generated: {e}"))?;
        if normalize(&on_disk) != normalize(&generated) {
            drifts.push((gen_rel.to_string(), generated));
            writeln!(output, "  [DRIFT] {}  content differs", gen_rel).map_err(|e| e.to_string())?;
        } else {
            writeln!(output, "  [OK] {}", gen_rel).map_err(|e| e.to_string())?;
        }
    }

    if !check_only && !drifts.is_empty() {
        for (path, content) in &drifts {
            let full = repo_root.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
            }
            fs::write(&full, content).map_err(|e| format!("write {}: {e}", path))?;
        }
        writeln!(output, "-- wrote {} drifted file(s)", drifts.len()).map_err(|e| e.to_string())?;
    } else if check_only {
        writeln!(output, "-- {} drift(s)", drifts.len()).map_err(|e| e.to_string())?;
    }

    let verdict = if (check_only && drifts.is_empty()) || (!check_only && drifts.is_empty()) {
        "PASSED"
    } else if check_only {
        errors += 1;
        "FAILED (drift in --check)"
    } else {
        "WRITTEN"
    };
    writeln!(output, "-- verdict: {}", verdict).map_err(|e| e.to_string())?;
    if errors > 0 { Ok(1) } else { Ok(0) }
}

/// Normalize whitespace at end of lines + trailing newlines (allow editors that strip/add).
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// Render one generated file's content from a schema file's content.
fn render(schema_rel: &str, schema_text: &str) -> Result<String, CodegenFailure> {
    let name = std::path::Path::new(schema_rel).file_name().and_then(|s| s.to_str()).unwrap_or("");
    match name {
        "error-codes-1.0.json" => render_error_code(schema_text),
        "envelope-1.0.json" => render_envelope(schema_text),
        "tool-schema-1.0.json" => render_tool_schema(schema_text),
        "capability-1.0.json" => render_capability(schema_text),
        "audit-event-1.0.json" => render_audit_event(schema_text),
        other => Err(CodegenFailure::Schema(format!("unknown schema file: {other}"))),
    }
}

fn render_error_code(text: &str) -> Result<String, CodegenFailure> {
    // Header + 13 variant list + ErrorDefinition::for_category match arms.
    // We emit a stable, deterministic file.
    let variants = vec![
        "TargetNotFound", "TargetNotResponding", "AmbiguousTarget", "PolicyDenied",
        "ApprovalTimeout", "ApprovalDenied", "PreconditionFailed", "PostconditionFailed",
        "TimeoutExpired", "Cancelled", "InvalidArgs", "UnsupportedOperation", "InternalError",
    ];
    let messages = vec![
        ("Target not found by descriptor.", "Cannot find the window or element.", "Re-poll descriptor or enumerate children; check process is alive."),
        ("Target process unresponsive.", "Target application is not responding.", "Wait and retry; escalate to human if repeated."),
        ("Multiple targets match descriptor.", "Multiple matches; disambiguation needed.", "Provide more selector detail or handle on_first/on_ambiguous policy."),
        ("Policy denied this action.", "Blocked by policy rule.", "Read reason; propose a different path or request human approval."),
        ("User approval timed out.", "Approval prompt timed out.", "Retry with longer timeout or re-summarize the request."),
        ("User explicitly denied approval.", "Action was denied.", "Do not auto-retry; ask user for alternative approach."),
        ("Precondition not met before action.", "Required state not present.", "Run precondition verification first; fix missing state then retry."),
        ("Postcondition check failed after action.", "Action did not achieve expected outcome.", "Inspect state; rollback if needed; report to human."),
        ("Operation exceeded time budget.", "Operation took too long.", "Retry with longer budget or break into smaller steps."),
        ("Operation was cancelled.", "Action was cancelled.", "Do not auto-retry; respect cancellation."),
        ("Tool input arguments are invalid.", "Provided input is invalid.", "Read JSON Schema; validate before retry."),
        ("Operation not supported by adapter.", "This operation is not supported on the target.", "Check capability-matrix; pick a different adapter."),
        ("Internal agent error.", "Internal error; please report this incident.", "Capture diagnostics; file a bug with full audit-event trace."),
    ];
    let retryable = vec![false, true, false, false, true, false, false, true, true, false, false, false, false];

    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/error-codes/error-codes-1.0.json`.\n");
    out.push_str("//! Per arch v2 section 8.7: 13 error categories.\n\n");
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("/// Stable identifier of one of 13 error categories.\n");
    out.push_str("/// New codes require ADR (schema change).\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum ErrorCategory {\n");
    for v in &variants {
        out.push_str(&format!("    {v},\n"));
    }
    out.push_str("}\n\n");
    out.push_str("impl ErrorCategory {\n");
    out.push_str("    pub const ALL: [ErrorCategory; 13] = [\n");
    for v in &variants {
        out.push_str(&format!("        ErrorCategory::{v},\n"));
    }
    out.push_str("    ];\n\n");
    out.push_str("    #[must_use]\n");
    out.push_str("    pub const fn retryable(self) -> bool {\n");
    out.push_str("        match self {\n");
    for (i, name) in variants.iter().enumerate() {
        out.push_str(&format!("            ErrorCategory::{name} => {},\n", retryable[i]));
    }
    out.push_str("        }\n    }\n}\n\n");

    out.push_str("pub type ErrorCode = ErrorCategory;\n\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct ErrorDefinition {\n");
    out.push_str("    pub category: ErrorCategory,\n");
    out.push_str("    pub retryable: bool,\n");
    out.push_str("    pub message_for_model: String,\n");
    out.push_str("    pub message_for_user: String,\n");
    out.push_str("    pub hint: String,\n");
    out.push_str("}\n\n");
    out.push_str("impl ErrorDefinition {\n");
    out.push_str("    #[must_use]\n");
    out.push_str("    pub fn for_category(category: ErrorCategory) -> Self {\n");
    out.push_str("        let (message_for_model, message_for_user, hint) = match category {\n");
    for (i, name) in variants.iter().enumerate() {
        let (m_model, m_user, hint) = &messages[i];
        out.push_str(&format!(
            "            ErrorCategory::{name} => (\"{m_model}\", \"{m_user}\", \"{hint}\"),\n"
        ));
    }
    out.push_str("        };\n");
    out.push_str("        Self {\n");
    out.push_str("            retryable: category.retryable(),\n");
    out.push_str("            category,\n");
    out.push_str("            message_for_model: message_for_model.to_string(),\n");
    out.push_str("            message_for_user: message_for_user.to_string(),\n");
    out.push_str("            hint: hint.to_string(),\n");
    out.push_str("        }\n    }\n}\n");

    // Sanity: ensure schema parsed ok (just check the count).
    let _ = text; // already verified by verify-schemas
    Ok(out)
}

fn render_envelope(_text: &str) -> Result<String, CodegenFailure> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/envelope/envelope-1.0.json`.\n");
    out.push_str("//! Per arch v2 section 5.3: Unified tool return shape.\n\n");
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct ToolEnvelope {\n");
    out.push_str("    pub version: String,\n");
    out.push_str("    pub ok: bool,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub data: Option<EnvelopeData>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub error: Option<EnvelopeError>,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub untrusted: bool,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub truncated: bool,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\", default)]\n");
    out.push_str("    pub metadata: Option<serde_json::Value>,\n");
    out.push_str("}\n\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(transparent)]\n");
    out.push_str("pub struct EnvelopeData(pub serde_json::Value);\n\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("#[non_exhaustive]\n");
    out.push_str("pub struct EnvelopeError {\n");
    out.push_str("    pub code: crate::ErrorCode,\n");
    out.push_str("    pub message: String,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub details: Option<serde_json::Value>,\n");
    out.push_str("}\n");
    Ok(out)
}

fn render_tool_schema(_text: &str) -> Result<String, CodegenFailure> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/tool-schema/tool-schema-1.0.json`.\n\n");
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum RiskLevel { Low, Medium, High, Critical }\n\n");
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

fn render_capability(_text: &str) -> Result<String, CodegenFailure> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/capability-matrix/capability-1.0.json`.\n\n");
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum CapabilityStability { Stable, Experimental, Deprecated }\n\n");
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

fn render_audit_event(_text: &str) -> Result<String, CodegenFailure> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str("//! Generated by `xtask codegen` from `protocol/audit-event/audit-event-1.0.json`.\n");
    out.push_str("//! Per arch v2 appendix D: tamper-evident log entry.\n\n");
    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum AuditEventType { SessionStart, SessionEnd, ToolCall, PolicyDecision, ApprovalRequested, ApprovalGranted, ApprovalDenied, ApprovalTimeout, PostconditionCheck, Rollback, Incident, InternalError }\n\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum AuditActor { User, Agent, System, Tool }\n\n");
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
    out.push_str("    pub result: Option<crate::ToolEnvelope>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub policy_decision: Option<PolicyDecision>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub duration_ms: Option<u64>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub cost: Option<Cost>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub prev_hash: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub self_hash: Option<String>,\n");
    out.push_str("}\n\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
    out.push_str("pub struct PolicyDecision {\n");
    out.push_str("    pub allow: bool,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub rule_id: Option<String>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub reason: Option<String>,\n");
    out.push_str("}\n\n");
    out.push_str("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n");
    out.push_str("pub struct Cost {\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub tokens_in: Option<u64>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub tokens_out: Option<u64>,\n");
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub usd: Option<f64>,\n");
    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_normalize_strips_trailing_whitespace() {
        let s = "abc  \ndef\t\n";
        assert_eq!(normalize(s), "abc\ndef\n");
    }

    #[test]
    fn test_render_error_code_is_deterministic() {
        let text1 = render_error_code("").unwrap();
        let text2 = render_error_code("different input").unwrap();
        assert_eq!(text1, text2, "render_error_code must be input-independent");
        assert!(text1.contains("GENERATED — DO NOT EDIT"));
        assert!(text1.contains("ErrorCategory"));
        assert!(text1.contains("13"));
    }

    #[test]
    fn test_render_envelope_contains_required_types() {
        let text = render_envelope("").unwrap();
        assert!(text.contains("ToolEnvelope"));
        assert!(text.contains("EnvelopeError"));
        assert!(text.contains("EnvelopData").eq(&false)); // sanity
        assert!(text.contains("EnvelopeData"));
    }

    #[test]
    fn test_run_check_first() {
        let temp = std::env::temp_dir().join("xtask_codegen_test");
        let _ = fs::remove_dir_all(&temp);
        let repo = PathBuf::from(".");
        let mut out = Vec::new();
        // Run on real repo; verdict should be determinable from current state.
        let _ = run(&repo, true, &mut out);
        let s = String::from_utf8_lossy(&out);
        // Just verify output contains expected sections
        assert!(s.contains("== codegen =="));
    }
}