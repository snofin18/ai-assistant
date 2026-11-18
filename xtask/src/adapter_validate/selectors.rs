//! D3 selector model validation.

use std::collections::BTreeMap;

use super::{check_keys, push_error};
use crate::report::Report;
use crate::serde_json_lite::Value;

const ROLES: [&str; 22] = [
    "Button",
    "CheckBox",
    "ComboBox",
    "Dialog",
    "Document",
    "Edit",
    "Group",
    "List",
    "Menu",
    "MenuBar",
    "MenuItem",
    "Pane",
    "RadioButton",
    "ScrollBar",
    "Slider",
    "SplitButton",
    "Tab",
    "Text",
    "ToolBar",
    "Tree",
    "TreeItem",
    "Window",
];
const SUPPORTS: [&str; 3] = ["foreground", "inspect", "read_text"];
const PROBES: [&str; 3] = ["observed", "required", "provisional"];

pub(super) fn validate(value: &Value, base: &str, report: &mut Report) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/selectors",
            format!("{base}/selectors/targets.json"),
            "must be object",
        );
        return;
    };
    match object.get("selector_model").and_then(Value::as_str) {
        Some("uia_candidate_chain_v1") => validate_uia(object, base, report),
        Some("shell_target_v1") => validate_shell(object, base, report),
        _ => push_error(
            report,
            "adapter/selectors",
            format!("{base}/selectors/targets.json"),
            "unknown selector_model",
        ),
    }
}

fn validate_uia(object: &BTreeMap<String, Value>, base: &str, report: &mut Report) {
    let path = format!("{base}/selectors/targets.json");
    check_keys(
        report,
        "adapter/selectors",
        &path,
        object,
        &[
            "schema_version",
            "app_id",
            "selector_model",
            "app_version_range",
            "probe_status",
            "policy",
            "targets",
        ],
    );
    validate_probe(object.get("probe_status"), &path, report);
    validate_policy(object.get("policy"), base, report);
    validate_targets(object.get("targets"), base, report, true);
}

fn validate_policy(value: Option<&Value>, base: &str, report: &mut Report) {
    let pointer = format!("{base}/selectors/targets.json#/policy");
    let Some(policy) = value.and_then(Value::as_object) else {
        push_error(report, "adapter/selectors", pointer, "policy missing");
        return;
    };
    check_keys(
        report,
        "adapter/selectors",
        &pointer,
        policy,
        &[
            "on_ambiguous",
            "on_not_found",
            "min_score_to_try",
            "max_resolve_ms",
        ],
    );
    if policy.get("on_ambiguous").and_then(Value::as_str) != Some("error_and_ask") {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/on_ambiguous"),
            "must be error_and_ask",
        );
    }
    let Some(not_found) = policy.get("on_not_found").and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/on_not_found"),
            "must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/selectors",
        &format!("{pointer}/on_not_found"),
        not_found,
        &["retry_after_ms", "then_escalate"],
    );
    validate_retry_after(not_found.get("retry_after_ms"), &pointer, report);
    if not_found.get("then_escalate").and_then(Value::as_bool) != Some(true) {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/on_not_found/then_escalate"),
            "must be true",
        );
    }
    if !policy
        .get("min_score_to_try")
        .and_then(Value::as_f64)
        .is_some_and(|score| score.is_finite() && (0.0..=1.0).contains(&score))
    {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/min_score_to_try"),
            "must be within [0,1]",
        );
    }
    if policy
        .get("max_resolve_ms")
        .and_then(Value::as_integer)
        .is_none_or(|value| value <= 0)
    {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/max_resolve_ms"),
            "must be positive integer",
        );
    }
}

fn validate_retry_after(value: Option<&Value>, pointer: &str, report: &mut Report) {
    let Some(retries) = value.and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/on_not_found/retry_after_ms"),
            "must be array",
        );
        return;
    };
    if retries.len() > 5 {
        push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/on_not_found/retry_after_ms"),
            "must contain at most 5 entries",
        );
    }
    let mut previous = 0_i64;
    for retry in retries {
        let Some(delay) = retry.as_integer() else {
            push_error(
                report,
                "adapter/selectors",
                format!("{pointer}/on_not_found/retry_after_ms"),
                "entries must be integers",
            );
            continue;
        };
        if delay <= 0 || delay <= previous {
            push_error(
                report,
                "adapter/selectors",
                format!("{pointer}/on_not_found/retry_after_ms"),
                "entries must be positive and strictly increasing",
            );
        }
        previous = delay;
    }
}

fn validate_shell(object: &BTreeMap<String, Value>, base: &str, report: &mut Report) {
    let path = format!("{base}/selectors/targets.json");
    check_keys(
        report,
        "adapter/selectors",
        &path,
        object,
        &[
            "schema_version",
            "app_id",
            "selector_model",
            "probe_status",
            "targets",
        ],
    );
    validate_probe(object.get("probe_status"), &path, report);
    validate_targets(object.get("targets"), base, report, false);
}

fn validate_targets(value: Option<&Value>, base: &str, report: &mut Report, is_uia: bool) {
    let Some(targets) = value.and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/selectors",
            format!("{base}/selectors/targets.json"),
            "targets must be array",
        );
        return;
    };
    let mut ids = std::collections::BTreeSet::new();
    for (index, target) in targets.iter().enumerate() {
        let pointer = format!("{base}/selectors/targets.json#/targets/{index}");
        let Some(target) = target.as_object() else {
            push_error(
                report,
                "adapter/selectors",
                pointer,
                "target must be object",
            );
            continue;
        };
        if is_uia {
            validate_uia_target(target, &pointer, report);
        } else {
            validate_shell_target(target, &pointer, report);
        }
        validate_unique_id(target, &pointer, &mut ids, report);
        validate_probe(target.get("probe_status"), &pointer, report);
    }
}

fn validate_uia_target(target: &BTreeMap<String, Value>, pointer: &str, report: &mut Report) {
    check_keys(
        report,
        "adapter/selectors",
        pointer,
        target,
        &[
            "id",
            "scope",
            "probe_status",
            "allow_fallback_entry",
            "candidates",
        ],
    );
    if !matches!(
        target.get("scope").and_then(Value::as_str),
        Some("window" | "element")
    ) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "invalid scope",
        );
    }
    if target
        .get("allow_fallback_entry")
        .and_then(Value::as_bool)
        .is_none()
    {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "allow_fallback_entry must be explicit bool",
        );
    }
    validate_uia_candidates(target, pointer, report);
}

fn validate_shell_target(target: &BTreeMap<String, Value>, pointer: &str, report: &mut Report) {
    check_keys(
        report,
        "adapter/selectors",
        pointer,
        target,
        &[
            "id",
            "role",
            "class_name",
            "automation_id",
            "supports",
            "probe_status",
        ],
    );
    validate_role(target.get("role"), pointer, report);
    let supports = target.get("supports").and_then(Value::as_array);
    if supports.is_none_or(Vec::is_empty) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "supports must be non-empty",
        );
    }
    let mut previous: Option<&str> = None;
    for supported in supports.into_iter().flatten() {
        let Some(supported_text) = supported.as_str() else {
            push_error(
                report,
                "adapter/selectors",
                format!("{pointer}/supports"),
                "entries must be strings",
            );
            continue;
        };
        if !SUPPORTS.contains(&supported_text) {
            push_error(
                report,
                "adapter/selectors",
                format!("{pointer}/supports"),
                "unknown shell support",
            );
        }
        if previous.is_some_and(|value| value >= supported_text) {
            push_error(
                report,
                "adapter/selectors",
                format!("{pointer}/supports"),
                "supports must be unique and byte-order sorted",
            );
        }
        previous = Some(supported_text);
    }
    if target.get("class_name").and_then(Value::as_str).is_none()
        && target
            .get("automation_id")
            .and_then(Value::as_str)
            .is_none()
    {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "shell target requires non-text identity",
        );
    }
}

fn validate_unique_id(
    target: &BTreeMap<String, Value>,
    pointer: &str,
    ids: &mut std::collections::BTreeSet<String>,
    report: &mut Report,
) {
    let Some(id) = target.get("id").and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "target id missing",
        );
        return;
    };
    if !ids.insert(id.to_string()) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "duplicate target id",
        );
    }
    validate_identifier(id, pointer, report);
}

fn validate_uia_candidates(target: &BTreeMap<String, Value>, pointer: &str, report: &mut Report) {
    let Some(candidates) = target.get("candidates").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "candidates must be array",
        );
        return;
    };
    let mut ids = std::collections::BTreeSet::new();
    let mut has_primary = false;
    for (index, candidate) in candidates.iter().enumerate() {
        let path = format!("{pointer}/candidates/{index}");
        let Some(candidate) = candidate.as_object() else {
            push_error(
                report,
                "adapter/selectors",
                path,
                "candidate must be object",
            );
            continue;
        };
        validate_candidate(candidate, &path, &mut ids, &mut has_primary, report);
    }
    let allow = target.get("allow_fallback_entry").and_then(Value::as_bool) == Some(true);
    if (allow && has_primary) || (!allow && !has_primary) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "fallback group contract violated",
        );
    }
}

fn validate_candidate(
    candidate: &BTreeMap<String, Value>,
    path: &str,
    ids: &mut std::collections::BTreeSet<String>,
    has_primary: &mut bool,
    report: &mut Report,
) {
    check_keys(
        report,
        "adapter/selectors",
        path,
        candidate,
        &[
            "id",
            "kind",
            "value",
            "score",
            "locale_dependent",
            "fallback_only",
            "ttl_ms",
        ],
    );
    let Some(id) = candidate.get("id").and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/selectors",
            path.to_string(),
            "candidate id missing",
        );
        return;
    };
    if ids.contains(id) {
        push_error(
            report,
            "adapter/selectors",
            path.to_string(),
            "duplicate candidate id",
        );
    }
    let kind = candidate
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    validate_kind_value(kind, candidate.get("value"), id, ids, path, report);
    ids.insert(id.to_string());
    if !candidate
        .get("score")
        .and_then(Value::as_f64)
        .is_some_and(|score| score.is_finite() && (0.0..=1.0).contains(&score))
    {
        push_error(
            report,
            "adapter/selectors",
            path.to_string(),
            "score must be within [0,1]",
        );
    }
    let locale = candidate.get("locale_dependent").and_then(Value::as_bool);
    let fallback = candidate
        .get("fallback_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if locale.is_none()
        || candidate
            .get("fallback_only")
            .and_then(Value::as_bool)
            .is_none()
    {
        push_error(
            report,
            "adapter/selectors",
            path.to_string(),
            "fallback fields must be explicit",
        );
    }
    if is_text_kind(kind) && (!locale.unwrap_or(false) || !fallback) {
        push_error(
            report,
            "adapter/selectors",
            path.to_string(),
            "text/path selectors must be locale fallback",
        );
    }
    *has_primary |= !fallback;
    validate_ttl(candidate.get("ttl_ms"), kind, path, report);
}

fn validate_kind_value(
    kind: &str,
    value: Option<&Value>,
    candidate_id: &str,
    existing_ids: &std::collections::BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(value) = value.and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "candidate value must be object",
        );
        return;
    };
    let expected = match kind {
        "class_and_role" => "class_and_role",
        "exact_name_and_role" => "exact_name_and_role",
        "role_and_parent" => "role_and_parent",
        "a11y_path" => "path",
        "visual_anchor" => "visual_anchor",
        "runtime_id" | "automation_id" | "ax_identifier" | "title_regex" | "name_regex"
        | "exact_name" => "text",
        _ => {
            push_error(
                report,
                "adapter/selectors",
                pointer.to_string(),
                "unknown selector kind",
            );
            return;
        }
    };
    if value.len() != 1 || !value.contains_key(expected) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "kind/value discriminator mismatch",
        );
        return;
    }
    let Some(discriminator) = value.get(expected) else {
        return;
    };
    match expected {
        "text" => require_nonempty_string(
            discriminator,
            &format!("{pointer}/value/{expected}"),
            report,
        ),
        "class_and_role" => validate_class_and_role(discriminator, pointer, report),
        "exact_name_and_role" => validate_exact_name_and_role(discriminator, pointer, report),
        "role_and_parent" => {
            validate_role_and_parent(discriminator, candidate_id, existing_ids, pointer, report);
        }
        "path" => validate_a11y_path(discriminator, pointer, report),
        "visual_anchor" => validate_visual_anchor(discriminator, pointer, report),
        _ => {}
    }
}

fn validate_class_and_role(value: &Value, pointer: &str, report: &mut Report) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "class_and_role must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/selectors",
        &format!("{pointer}/value/class_and_role"),
        object,
        &["class", "role"],
    );
    if object
        .get("class")
        .and_then(Value::as_str)
        .is_none_or(|text| text.trim().is_empty())
    {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "class must be non-empty",
        );
    }
    validate_role(object.get("role"), pointer, report);
}

fn validate_exact_name_and_role(value: &Value, pointer: &str, report: &mut Report) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "exact_name_and_role must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/selectors",
        &format!("{pointer}/value/exact_name_and_role"),
        object,
        &["name", "role"],
    );
    if object
        .get("name")
        .and_then(Value::as_str)
        .is_none_or(|text| text.trim().is_empty())
    {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "name must be non-empty",
        );
    }
    validate_role(object.get("role"), pointer, report);
}

fn validate_role_and_parent(
    value: &Value,
    candidate_id: &str,
    existing_ids: &std::collections::BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "role_and_parent must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/selectors",
        &format!("{pointer}/value/role_and_parent"),
        object,
        &["role", "parent_id"],
    );
    validate_role(object.get("role"), pointer, report);
    let parent = object.get("parent_id").and_then(Value::as_str);
    if parent.is_none_or(|parent| parent == candidate_id || !existing_ids.contains(parent)) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "parent_id must reference an earlier candidate",
        );
    }
}

fn validate_a11y_path(value: &Value, pointer: &str, report: &mut Report) {
    let Some(path) = value.as_array() else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "path must be array",
        );
        return;
    };
    if path.is_empty()
        || path
            .iter()
            .any(|segment| segment.as_str().is_none_or(|text| text.trim().is_empty()))
    {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "path entries must be non-empty strings",
        );
    }
}

fn validate_visual_anchor(value: &Value, pointer: &str, report: &mut Report) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "visual_anchor must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/selectors",
        &format!("{pointer}/value/visual_anchor"),
        object,
        &["ocr_text", "region"],
    );
    for field in ["ocr_text", "region"] {
        if object
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|text| text.trim().is_empty())
        {
            push_error(
                report,
                "adapter/selectors",
                pointer.to_string(),
                format!("{field} must be non-empty"),
            );
        }
    }
}

fn require_nonempty_string(value: &Value, pointer: &str, report: &mut Report) {
    if value.as_str().is_none_or(|text| text.trim().is_empty()) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "must be non-empty string",
        );
    }
}

fn validate_ttl(value: Option<&Value>, kind: &str, pointer: &str, report: &mut Report) {
    match (kind, value.and_then(Value::as_integer)) {
        ("runtime_id", Some(0)) => {}
        ("runtime_id", _) => push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/ttl_ms"),
            "runtime_id requires ttl_ms = 0",
        ),
        (_, None) if value.is_none() => {}
        (_, Some(ttl)) if ttl > 0 => {}
        _ => push_error(
            report,
            "adapter/selectors",
            format!("{pointer}/ttl_ms"),
            "ttl_ms must be positive integer when present",
        ),
    }
}

fn validate_identifier(value: &str, pointer: &str, report: &mut Report) {
    let mut characters = value.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
    if !valid {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "id must match ^[a-z][a-z0-9_]*$",
        );
    }
}

fn validate_role(value: Option<&Value>, pointer: &str, report: &mut Report) {
    let Some(role) = value.and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "role missing",
        );
        return;
    };
    if !ROLES.contains(&role) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "unknown role",
        );
    }
}

fn validate_probe(value: Option<&Value>, pointer: &str, report: &mut Report) {
    if !PROBES.contains(&value.and_then(Value::as_str).unwrap_or_default()) {
        push_error(
            report,
            "adapter/selectors",
            pointer.to_string(),
            "invalid probe_status",
        );
    }
}

fn is_text_kind(kind: &str) -> bool {
    matches!(
        kind,
        "exact_name_and_role"
            | "a11y_path"
            | "title_regex"
            | "name_regex"
            | "exact_name"
            | "visual_anchor"
    )
}
