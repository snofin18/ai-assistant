//! Shared Tool metadata projection and capability-specific risk floors.
//!
//! Responsibility: build immutable [`ToolInfo`] values and reject capability
//! combinations that could hide process spawn or spreadsheet writes behind a
//! lower-risk Tool envelope.
//!
//! Boundaries: this module does not parse `ToolSchema` shape, read Adapter files,
//! or mutate generated catalogs. Those responsibilities stay in `tools.rs`.
//!
//! Invariants:
//! 1. `com.workbook_open` is the sole capability on a non-idempotent,
//!    approval-gated, L3 Excel process Tool;
//! 2. Excel write capabilities require write effect, at least medium risk,
//!    explicit approval, and a sufficient recovery level;
//! 3. metadata extraction preserves the existing `ToolInfo` contract.

use std::collections::BTreeMap;

use super::{ToolInfo, push_error};
use crate::report::Report;
use crate::serde_json_lite::Value;

const COM_WRITE_CAPABILITIES: [&str; 4] = [
    "com.range_write",
    "com.workbook_save",
    "com.chart_create",
    "com.chart_export",
];

pub(super) fn tool_info(schema: &BTreeMap<String, Value>) -> ToolInfo {
    ToolInfo {
        name: schema
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        effect: schema
            .get("effect")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        reversibility: schema
            .get("reversibility")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        risk_level: schema
            .get("risk_level")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        requires_approval: schema
            .get("requires_approval")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tags: schema
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    }
}

pub(super) fn validate_capability_risk_floor(
    schema: &BTreeMap<String, Value>,
    value: Option<&Value>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(capabilities) = value.and_then(Value::as_array) else {
        return;
    };
    if has_capability(capabilities, "com.workbook_open") {
        validate_open_capability_floor(schema, capabilities, pointer, report);
    }
    for capability in COM_WRITE_CAPABILITIES {
        if has_capability(capabilities, capability) {
            validate_excel_write_floor(schema, capability, pointer, report);
        }
    }
}

fn validate_open_capability_floor(
    schema: &BTreeMap<String, Value>,
    capabilities: &[Value],
    pointer: &str,
    report: &mut Report,
) {
    if capabilities.len() != 1 {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/capabilities"),
            "com.workbook_open must be the sole capability",
        );
    }
    let effect = schema
        .get("effect")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if effect != "write" {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/effect"),
            "com.workbook_open requires effect=write",
        );
    }
    let reversibility = schema
        .get("reversibility")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if reversibility != "l3_irreversible" {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/reversibility"),
            "com.workbook_open requires l3_irreversible",
        );
    }
    let risk_level = schema
        .get("risk_level")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(risk_level, "high" | "critical") {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/risk_level"),
            "com.workbook_open requires high|critical risk",
        );
    }
    if schema.get("requires_approval").and_then(Value::as_bool) != Some(true) {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/requires_approval"),
            "com.workbook_open requires requires_approval=true",
        );
    }
    if schema.get("idempotent").and_then(Value::as_bool) != Some(false) {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/idempotent"),
            "com.workbook_open requires idempotent=false",
        );
    }
}

fn validate_excel_write_floor(
    schema: &BTreeMap<String, Value>,
    capability: &str,
    pointer: &str,
    report: &mut Report,
) {
    let effect = schema
        .get("effect")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if effect != "write" {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/effect"),
            format!("{capability} requires effect=write"),
        );
    }
    let risk_level = schema
        .get("risk_level")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(risk_level, "medium" | "high" | "critical") {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/risk_level"),
            format!("{capability} requires medium|high|critical risk"),
        );
    }
    if schema.get("requires_approval").and_then(Value::as_bool) != Some(true) {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/requires_approval"),
            format!("{capability} requires requires_approval=true"),
        );
    }
    let reversibility = schema
        .get("reversibility")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let has_required_recovery = match capability {
        "com.range_write" | "com.workbook_save" | "com.chart_create" => {
            reversibility == "l1_snapshot"
        }
        "com.chart_export" => {
            matches!(
                reversibility,
                "l1_snapshot" | "l2_compensation" | "l3_irreversible"
            )
        }
        _ => false,
    };
    if !has_required_recovery {
        push_error(
            report,
            "adapter/tools",
            format!("{pointer}/schema/reversibility"),
            format!("{capability} has an insufficient recovery level"),
        );
    }
}

fn has_capability(capabilities: &[Value], expected: &str) -> bool {
    capabilities
        .iter()
        .any(|capability| capability.as_str() == Some(expected))
}
