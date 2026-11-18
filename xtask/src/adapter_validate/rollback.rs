//! D5 rollback coverage validation.

use std::collections::{BTreeMap, BTreeSet};

use super::{ToolInfo, check_keys, push_error};
use crate::report::Report;
use crate::serde_json_lite::Value;

pub(super) fn validate(
    value: &Value,
    tools: &[ToolInfo],
    catalog: &Value,
    base: &str,
    report: &mut Report,
) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/rollback",
            format!("{base}/rollback/recipes.json"),
            "must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/rollback",
        &format!("{base}/rollback/recipes.json"),
        object,
        &["schema_version", "app_id", "recipes"],
    );
    let Some(recipes) = object.get("recipes").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/rollback",
            format!("{base}/rollback/recipes.json"),
            "recipes must be array",
        );
        return;
    };
    let writes: BTreeSet<&str> = tools
        .iter()
        .filter(|tool| tool.effect == "write")
        .map(|tool| tool.name.as_str())
        .collect();
    let counts = validate_rows(recipes, tools, catalog, &writes, base, report);
    for name in writes {
        if counts.get(name).copied() != Some(1) {
            push_error(
                report,
                "adapter/rollback",
                format!("{base}/rollback/recipes.json"),
                format!("write tool `{name}` must have exactly one coverage"),
            );
        }
    }
}

fn validate_rows(
    recipes: &[Value],
    tools: &[ToolInfo],
    catalog: &Value,
    writes: &BTreeSet<&str>,
    base: &str,
    report: &mut Report,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (index, recipe) in recipes.iter().enumerate() {
        let pointer = format!("{base}/rollback/recipes.json#/recipes/{index}");
        let Some(recipe) = recipe.as_object() else {
            push_error(
                report,
                "adapter/rollback",
                pointer,
                "coverage row must be object",
            );
            continue;
        };
        let tool = recipe
            .get("tool")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !writes.contains(tool) {
            push_error(
                report,
                "adapter/rollback",
                pointer.clone(),
                "coverage tool is not a write tool",
            );
        }
        *counts.entry(tool.to_string()).or_insert(0) += 1;
        validate_failure_policy(recipe, &pointer, report);
        validate_row(recipe, tools, catalog, tool, &pointer, report);
    }
    counts
}

fn validate_failure_policy(recipe: &BTreeMap<String, Value>, pointer: &str, report: &mut Report) {
    if recipe.get("on_rollback_failure").and_then(Value::as_str)
        != Some("publish_incident_and_require_human")
    {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "invalid on_rollback_failure",
        );
    }
}

fn validate_row(
    recipe: &BTreeMap<String, Value>,
    tools: &[ToolInfo],
    catalog: &Value,
    tool: &str,
    pointer: &str,
    report: &mut Report,
) {
    match recipe.get("coverage").and_then(Value::as_str) {
        Some("no_recovery") => validate_no_recovery(recipe, tools, tool, pointer, report),
        Some("recipe") => validate_recipe(recipe, tools, catalog, tool, pointer, report),
        _ => push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "coverage must be recipe|no_recovery",
        ),
    }
}

fn validate_no_recovery(
    recipe: &BTreeMap<String, Value>,
    tools: &[ToolInfo],
    tool: &str,
    pointer: &str,
    report: &mut Report,
) {
    check_keys(
        report,
        "adapter/rollback",
        pointer,
        recipe,
        &["tool", "coverage", "reason", "on_rollback_failure"],
    );
    if recipe
        .get("reason")
        .and_then(Value::as_str)
        .is_none_or(|text| text.trim().is_empty())
    {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "no_recovery requires reason",
        );
    }
    if let Some(tool) = tools.iter().find(|info| info.name == tool)
        && (tool.reversibility != "l3_irreversible"
            || !matches!(tool.risk_level.as_str(), "high" | "critical")
            || !tool.requires_approval)
    {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "invalid no_recovery tool metadata",
        );
    }
}

fn validate_recipe(
    recipe: &BTreeMap<String, Value>,
    tools: &[ToolInfo],
    catalog: &Value,
    tool_name: &str,
    pointer: &str,
    report: &mut Report,
) {
    check_keys(
        report,
        "adapter/rollback",
        pointer,
        recipe,
        &["tool", "coverage", "anchor_strategy", "on_rollback_failure"],
    );
    let Some(anchors) = recipe.get("anchor_strategy").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "anchor_strategy must be array",
        );
        return;
    };
    if anchors.is_empty() {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "anchor_strategy must be non-empty",
        );
    }
    for (index, anchor) in anchors.iter().enumerate() {
        validate_anchor(
            anchor,
            tools,
            catalog,
            &format!("{pointer}/anchor_strategy/{index}"),
            report,
        );
    }
    if let Some(tool) = tools.iter().find(|info| info.name == tool_name) {
        let levels: Vec<&str> = anchors
            .iter()
            .filter_map(|anchor| anchor.get("level").and_then(Value::as_str))
            .collect();
        let valid = match tool.reversibility.as_str() {
            "l0_undo_stack" => levels.first() == Some(&"l0"),
            "l1_snapshot" => levels.contains(&"l1"),
            "l2_compensation" => levels.len() == 1 && levels.first() == Some(&"l2"),
            _ => false,
        };
        if !valid {
            push_error(
                report,
                "adapter/rollback",
                pointer.to_string(),
                "anchor levels do not match reversibility",
            );
        }
    }
}

fn validate_anchor(
    value: &Value,
    tools: &[ToolInfo],
    catalog: &Value,
    pointer: &str,
    report: &mut Report,
) {
    let Some(anchor) = value.as_object() else {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "anchor must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/rollback",
        pointer,
        anchor,
        &["level", "mechanism", "capture", "restore", "post_verify"],
    );
    let level = anchor
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mechanism = anchor
        .get("mechanism")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(
        (level, mechanism),
        ("l0", "undo_stack")
            | ("l1", "content_snapshot" | "shadow_copy")
            | ("l2", "compensating_action")
    ) {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "level/mechanism mismatch",
        );
    }
    let Some((capture_kind, restore_kind, expected_source)) = anchor_kinds(level, mechanism) else {
        return;
    };
    let observations = validate_capture(
        anchor.get("capture"),
        tools,
        capture_kind,
        expected_source,
        pointer,
        report,
    );
    validate_restore(anchor.get("restore"), tools, restore_kind, pointer, report);
    validate_post_verify(
        anchor.get("post_verify"),
        catalog,
        &observations,
        pointer,
        report,
    );
}

fn anchor_kinds(
    level: &str,
    mechanism: &str,
) -> Option<(&'static str, &'static str, Option<&'static str>)> {
    match (level, mechanism) {
        ("l0", "undo_stack") => Some(("undo_budget", "undo_stack", Some("builtin:app.undo_stack"))),
        ("l1", "content_snapshot") => Some((
            "content_snapshot",
            "restore_content_snapshot",
            Some("builtin:storage.content_snapshot"),
        )),
        ("l1", "shadow_copy") => Some((
            "file_shadow_copy",
            "restore_file_shadow_copy",
            Some("builtin:filesystem.shadow_copy"),
        )),
        ("l2", "compensating_action") => Some(("compensating_state", "invoke_tool", None)),
        _ => None,
    }
}

fn validate_capture(
    value: Option<&Value>,
    tools: &[ToolInfo],
    expected_kind: &str,
    expected_source: Option<&str>,
    pointer: &str,
    report: &mut Report,
) -> BTreeSet<String> {
    let capture_pointer = format!("{pointer}/capture");
    let Some(capture) = value.and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/rollback",
            capture_pointer,
            "must be object",
        );
        return BTreeSet::new();
    };
    check_keys(
        report,
        "adapter/rollback",
        &capture_pointer,
        capture,
        &["kind", "source", "max_steps", "valid_until", "observations"],
    );
    if capture.get("kind").and_then(Value::as_str) != Some(expected_kind) {
        push_error(
            report,
            "adapter/rollback",
            format!("{capture_pointer}/kind"),
            format!("must be {expected_kind}"),
        );
    }
    validate_capture_source(
        capture.get("source"),
        tools,
        expected_source,
        &capture_pointer,
        report,
    );
    if expected_kind == "undo_budget" {
        if capture
            .get("max_steps")
            .and_then(Value::as_integer)
            .is_none_or(|steps| steps <= 0)
        {
            push_error(
                report,
                "adapter/rollback",
                format!("{capture_pointer}/max_steps"),
                "must be positive integer",
            );
        }
        if capture
            .get("valid_until")
            .and_then(Value::as_str)
            .is_none_or(|value| value != "document_close")
        {
            push_error(
                report,
                "adapter/rollback",
                format!("{capture_pointer}/valid_until"),
                "must be document_close",
            );
        }
    }
    let Some(observations) = capture.get("observations").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/rollback",
            format!("{capture_pointer}/observations"),
            "must be array",
        );
        return BTreeSet::new();
    };
    validate_sorted_nonempty_strings(
        observations,
        &format!("{capture_pointer}/observations"),
        report,
    )
}

fn validate_capture_source(
    value: Option<&Value>,
    tools: &[ToolInfo],
    expected_builtin: Option<&str>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(source) = value.and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/rollback",
            format!("{pointer}/source"),
            "must be string",
        );
        return;
    };
    if let Some(expected) = expected_builtin {
        if source != expected {
            push_error(
                report,
                "adapter/rollback",
                format!("{pointer}/source"),
                format!("must be {expected}"),
            );
        }
        return;
    }
    let Some(tool_name) = source.strip_prefix("tool:") else {
        push_error(
            report,
            "adapter/rollback",
            format!("{pointer}/source"),
            "must reference tool:<read-tool>",
        );
        return;
    };
    if !tools
        .iter()
        .any(|tool| tool.name == tool_name && tool.effect == "read")
    {
        push_error(
            report,
            "adapter/rollback",
            format!("{pointer}/source"),
            "source tool must be declared read tool",
        );
    }
}

fn validate_restore(
    value: Option<&Value>,
    tools: &[ToolInfo],
    expected_kind: &str,
    pointer: &str,
    report: &mut Report,
) {
    let restore_pointer = format!("{pointer}/restore");
    let Some(restore) = value.and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/rollback",
            restore_pointer,
            "must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/rollback",
        &restore_pointer,
        restore,
        &["kind", "tool", "times", "until", "arguments"],
    );
    if restore.get("kind").and_then(Value::as_str) != Some(expected_kind) {
        push_error(
            report,
            "adapter/rollback",
            format!("{restore_pointer}/kind"),
            format!("must be {expected_kind}"),
        );
    }
    if let Some(tool) = restore.get("tool").and_then(Value::as_str)
        && !tools.iter().any(|info| info.name == tool)
    {
        push_error(
            report,
            "adapter/rollback",
            format!("{restore_pointer}/tool"),
            "restore tool is not declared",
        );
    }
    if expected_kind == "invoke_tool" && restore.get("tool").and_then(Value::as_str).is_none() {
        push_error(
            report,
            "adapter/rollback",
            format!("{restore_pointer}/tool"),
            "invoke_tool requires tool",
        );
    }
    if let Some(times) = restore.get("times")
        && times.as_integer().is_none_or(|value| value <= 0)
    {
        push_error(
            report,
            "adapter/rollback",
            format!("{restore_pointer}/times"),
            "times must be positive integer",
        );
    }
    if restore
        .get("arguments")
        .is_some_and(|arguments| arguments.as_object().is_none())
    {
        push_error(
            report,
            "adapter/rollback",
            format!("{restore_pointer}/arguments"),
            "arguments must be object",
        );
    }
}

fn validate_post_verify(
    value: Option<&Value>,
    catalog: &Value,
    observations: &BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let verify_pointer = format!("{pointer}/post_verify");
    let Some(post_verify) = value.and_then(Value::as_object) else {
        push_error(report, "adapter/rollback", verify_pointer, "must be object");
        return;
    };
    match post_verify.get("kind").and_then(Value::as_str) {
        Some("capture_equals") => {
            check_keys(
                report,
                "adapter/rollback",
                &verify_pointer,
                post_verify,
                &["kind", "observation"],
            );
            if !post_verify
                .get("observation")
                .and_then(Value::as_str)
                .is_some_and(|observation| observations.contains(observation))
            {
                push_error(
                    report,
                    "adapter/rollback",
                    format!("{verify_pointer}/observation"),
                    "must reference capture observation",
                );
            }
        }
        Some("postcondition") => {
            check_keys(
                report,
                "adapter/rollback",
                &verify_pointer,
                post_verify,
                &["kind", "assertion"],
            );
            if let Some(assertion) = post_verify.get("assertion") {
                let _ = super::tools::validate_postcondition(
                    assertion,
                    "write",
                    catalog,
                    &format!("{verify_pointer}/assertion"),
                    report,
                );
            } else {
                push_error(
                    report,
                    "adapter/rollback",
                    format!("{verify_pointer}/assertion"),
                    "postcondition assertion missing",
                );
            }
        }
        _ => push_error(
            report,
            "adapter/rollback",
            format!("{verify_pointer}/kind"),
            "must be capture_equals|postcondition",
        ),
    }
}

fn validate_sorted_nonempty_strings(
    values: &[Value],
    pointer: &str,
    report: &mut Report,
) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut previous: Option<&str> = None;
    if values.is_empty() {
        push_error(
            report,
            "adapter/rollback",
            pointer.to_string(),
            "must be non-empty",
        );
    }
    for value in values {
        let Some(text) = value.as_str() else {
            push_error(
                report,
                "adapter/rollback",
                pointer.to_string(),
                "entries must be strings",
            );
            continue;
        };
        if text.trim().is_empty() {
            push_error(
                report,
                "adapter/rollback",
                pointer.to_string(),
                "entries must be non-empty",
            );
        }
        if previous.is_some_and(|previous| previous >= text) {
            push_error(
                report,
                "adapter/rollback",
                pointer.to_string(),
                "entries must be unique and byte-order sorted",
            );
        }
        previous = Some(text);
        result.insert(text.to_string());
    }
    result
}
