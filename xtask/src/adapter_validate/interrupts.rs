//! D6 interrupt grammar validation.

use std::collections::{BTreeMap, BTreeSet};

use super::{ToolInfo, check_keys, push_error};
use crate::report::Report;
use crate::serde_json_lite::Value;

const CLASSIFICATIONS: [&str; 9] = [
    "unsaved_changes_three_way",
    "cross_process_shell_dialog",
    "transient_popup",
    "overwrite_confirmation",
    "unsupported_network_feature",
    "modal_color_picker",
    "consent_dialog",
    "credential_boundary",
    "forbidden_challenge",
];
const HUMAN_REQUIRED: [&str; 6] = [
    "unsaved_changes_three_way",
    "overwrite_confirmation",
    "unsupported_network_feature",
    "consent_dialog",
    "credential_boundary",
    "forbidden_challenge",
];

pub(super) fn validate(value: &Value, tools: &[ToolInfo], base: &str, report: &mut Report) {
    let Some(object) = value.as_object() else {
        push_error(
            report,
            "adapter/interrupts",
            format!("{base}/interrupts/interrupts.json"),
            "must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/interrupts",
        &format!("{base}/interrupts/interrupts.json"),
        object,
        &["schema_version", "app_id", "default_action", "interrupts"],
    );
    if !matches!(
        object.get("default_action").and_then(Value::as_str),
        Some("cancel" | "dismiss_explicitly" | "ask_human")
    ) {
        push_error(
            report,
            "adapter/interrupts",
            format!("{base}/interrupts/interrupts.json#/default_action"),
            "unsafe default_action",
        );
    }
    let Some(interrupts) = object.get("interrupts").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/interrupts",
            format!("{base}/interrupts/interrupts.json"),
            "interrupts must be array",
        );
        return;
    };
    let names: BTreeSet<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    let mut priorities = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (index, interrupt) in interrupts.iter().enumerate() {
        validate_rule(
            interrupt,
            &names,
            &mut priorities,
            &mut ids,
            &format!("{base}/interrupts/interrupts.json#/interrupts/{index}"),
            report,
        );
    }
}

fn validate_rule(
    value: &Value,
    tools: &BTreeSet<&str>,
    priorities: &mut BTreeSet<u64>,
    ids: &mut BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(rule) = value.as_object() else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "interrupt must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/interrupts",
        pointer,
        rule,
        &[
            "id",
            "priority",
            "match",
            "action",
            "classification",
            "requires_human",
            "resume_after",
            "notes",
        ],
    );
    validate_rule_id(rule.get("id"), ids, pointer, report);
    validate_priority(rule.get("priority"), priorities, pointer, report);
    validate_match(rule.get("match"), pointer, report);
    validate_action(rule.get("action"), tools, pointer, report);
    validate_classification(rule, pointer, report);
    validate_notes(rule.get("notes"), pointer, report);
}

fn validate_rule_id(
    value: Option<&Value>,
    ids: &mut BTreeSet<String>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(id) = value.and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "id must be string",
        );
        return;
    };
    let mut characters = id.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        });
    if !valid {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "id must match ^[a-z][a-z0-9_-]*$",
        );
    }
    if !ids.insert(id.to_string()) {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "duplicate interrupt id",
        );
    }
}

fn validate_priority(
    value: Option<&Value>,
    priorities: &mut BTreeSet<u64>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(number) = value.and_then(Value::as_f64) else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "priority must be positive integer",
        );
        return;
    };
    let Some(integer) = number.to_string().parse::<u64>().ok() else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "priority must be positive integer",
        );
        return;
    };
    if number.fract() != 0.0 || integer == 0 {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "priority must be positive integer",
        );
    } else if !priorities.insert(integer) {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "duplicate priority",
        );
    }
}

fn validate_match(value: Option<&Value>, pointer: &str, report: &mut Report) {
    let Some(matches) = value.and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "match must be object",
        );
        return;
    };
    check_keys(
        report,
        "adapter/interrupts",
        pointer,
        matches,
        &[
            "class_name",
            "automation_id",
            "automation_id_candidates",
            "title_candidates",
            "name_candidates",
            "class_name_contains",
        ],
    );
    if matches.is_empty() {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "match must be non-empty",
        );
    }
    for field in ["class_name", "automation_id", "class_name_contains"] {
        if matches
            .get(field)
            .is_some_and(|value| value.as_str().is_none_or(|text| text.trim().is_empty()))
        {
            push_error(
                report,
                "adapter/interrupts",
                format!("{pointer}/match/{field}"),
                "must be non-empty string",
            );
        }
    }
    validate_match_candidates(
        matches.get("automation_id_candidates"),
        false,
        pointer,
        report,
    );
    let has_text =
        matches.contains_key("title_candidates") || matches.contains_key("name_candidates");
    let has_non_text = [
        "class_name",
        "automation_id",
        "automation_id_candidates",
        "class_name_contains",
    ]
    .iter()
    .any(|field| matches.contains_key(*field));
    if has_text && !has_non_text {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "text candidates cannot be sole match",
        );
    }
    for field in ["title_candidates", "name_candidates"] {
        validate_text_candidates(matches.get(field), pointer, report);
    }
}

fn validate_text_candidates(value: Option<&Value>, pointer: &str, report: &mut Report) {
    let Some(value) = value else {
        return;
    };
    let Some(candidates) = value.as_array() else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "text candidates must be array",
        );
        return;
    };
    if candidates.is_empty() {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "text candidates must be non-empty",
        );
    }
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        let Some(candidate) = candidate.as_object() else {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "text candidate must be object",
            );
            continue;
        };
        check_keys(
            report,
            "adapter/interrupts",
            pointer,
            candidate,
            &["value", "locale_dependent"],
        );
        if candidate.get("locale_dependent").and_then(Value::as_bool) != Some(true)
            || candidate
                .get("value")
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
        {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "text candidate requires value + locale_dependent=true",
            );
        }
        if let Some(text) = candidate.get("value").and_then(Value::as_str)
            && !seen.insert(text.to_string())
        {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "duplicate text candidate",
            );
        }
    }
}

fn validate_match_candidates(
    value: Option<&Value>,
    allow_empty: bool,
    pointer: &str,
    report: &mut Report,
) {
    let Some(value) = value else {
        return;
    };
    let Some(candidates) = value.as_array() else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "candidate list must be array",
        );
        return;
    };
    if !allow_empty && candidates.is_empty() {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "candidate list must be non-empty",
        );
    }
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        let Some(text) = candidate.as_str() else {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "candidate entries must be strings",
            );
            continue;
        };
        if text.trim().is_empty() {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "candidate entries must be non-empty",
            );
        }
        if !seen.insert(text.to_string()) {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "duplicate candidate",
            );
        }
    }
}

fn validate_action(
    value: Option<&Value>,
    tools: &BTreeSet<&str>,
    pointer: &str,
    report: &mut Report,
) {
    let Some(action) = value.and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "action must be object",
        );
        return;
    };
    match action
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "cancel" | "dismiss_explicitly" | "ask_human" => {
            if action.len() != 1 {
                push_error(
                    report,
                    "adapter/interrupts",
                    pointer.to_string(),
                    "simple action has extra fields",
                );
            }
        }
        "delegate_to_tool" => validate_delegate(action, tools, pointer, report),
        "cancel_unless_declared_by_tool" => validate_accepting(action, tools, pointer, report),
        _ => push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "unknown interrupt action",
        ),
    }
}

fn validate_delegate(
    action: &BTreeMap<String, Value>,
    tools: &BTreeSet<&str>,
    pointer: &str,
    report: &mut Report,
) {
    check_keys(
        report,
        "adapter/interrupts",
        pointer,
        action,
        &["kind", "tool"],
    );
    let Some(tool) = action.get("tool").and_then(Value::as_str) else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "delegate tool missing",
        );
        return;
    };
    if !tools.contains(tool) {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "delegate tool unresolved",
        );
    }
}

fn validate_accepting(
    action: &BTreeMap<String, Value>,
    tools: &BTreeSet<&str>,
    pointer: &str,
    report: &mut Report,
) {
    check_keys(
        report,
        "adapter/interrupts",
        pointer,
        action,
        &["kind", "accepting_tools"],
    );
    let Some(accepting) = action.get("accepting_tools").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "accepting_tools missing",
        );
        return;
    };
    if accepting.is_empty() {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "accepting_tools must be non-empty",
        );
    }
    for tool in accepting {
        if !tool.as_str().is_some_and(|name| tools.contains(name)) {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "accepting tool unresolved",
            );
        }
    }
}

fn validate_classification(rule: &BTreeMap<String, Value>, pointer: &str, report: &mut Report) {
    let classification = rule
        .get("classification")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !CLASSIFICATIONS.contains(&classification) {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "unknown classification",
        );
    }
    let requires_human = rule.get("requires_human").and_then(Value::as_bool);
    if requires_human.is_none() {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "requires_human must be explicit",
        );
    }
    if HUMAN_REQUIRED.contains(&classification) && requires_human != Some(true) {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "classification requires human",
        );
    }
    if rule
        .get("action")
        .and_then(|action| action.get("kind"))
        .and_then(Value::as_str)
        == Some("ask_human")
        && requires_human != Some(true)
    {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "ask_human requires requires_human=true",
        );
    }
    let resume = rule.get("resume_after").and_then(Value::as_str);
    if requires_human == Some(true)
        && !matches!(
            resume,
            Some("user_decision" | "interrupt_cleared" | "tool_resume")
        )
    {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "human interrupt requires valid resume_after",
        );
    }
    if requires_human == Some(false) && rule.contains_key("resume_after") {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "non-human interrupt must omit resume_after",
        );
    }
}

fn validate_notes(value: Option<&Value>, pointer: &str, report: &mut Report) {
    let Some(value) = value else {
        return;
    };
    let Some(notes) = value.as_array() else {
        push_error(
            report,
            "adapter/interrupts",
            pointer.to_string(),
            "notes must be array",
        );
        return;
    };
    for note in notes {
        if note.as_str().is_none_or(|text| text.trim().is_empty()) {
            push_error(
                report,
                "adapter/interrupts",
                pointer.to_string(),
                "notes entries must be non-empty strings",
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "../adapter_validate_interrupt_tests.rs"]
mod tests;
