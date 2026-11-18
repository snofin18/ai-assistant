//! D2 App Map projection validation.

use std::collections::BTreeSet;

use super::{check_keys, push_error, string_field};
use crate::report::Report;
use crate::serde_json_lite::Value;

#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    documents: &'a std::collections::BTreeMap<String, Value>,
    repo_root: &'a std::path::Path,
    base: &'a str,
}

impl<'a> Context<'a> {
    pub(super) const fn new(
        documents: &'a std::collections::BTreeMap<String, Value>,
        repo_root: &'a std::path::Path,
        base: &'a str,
    ) -> Self {
        Self {
            documents,
            repo_root,
            base,
        }
    }
}

#[derive(Default)]
struct EntryState {
    ids: BTreeSet<String>,
    refs: Vec<String>,
}

pub(super) fn validate(
    value: &Value,
    author_text: Option<&String>,
    projection_text: &str,
    projection_context: Context<'_>,
    report: &mut Report,
) {
    let base = projection_context.base;
    let Some(map) = value.as_object() else {
        push_error(
            report,
            "adapter/app-map",
            format!("{base}/app_map.json"),
            "must be object",
        );
        return;
    };
    let Some(manifest) = map.get("projection_manifest").and_then(Value::as_object) else {
        push_error(
            report,
            "adapter/app-map-manifest",
            format!("{base}/app_map.json"),
            "projection_manifest missing",
        );
        return;
    };
    if manifest.get("model").and_then(Value::as_str) != Some("core_app_map_v1") {
        push_error(
            report,
            "adapter/app-map-manifest",
            format!("{base}/app_map.json#/projection_manifest/model"),
            "must be core_app_map_v1",
        );
    }
    check_keys(
        report,
        "adapter/app-map-manifest",
        &format!("{base}/app_map.json#/projection_manifest"),
        manifest,
        &["model", "entries", "omissions"],
    );
    let Some(entries) = manifest.get("entries").and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/app-map-manifest",
            format!("{base}/app_map.json#/projection_manifest/entries"),
            "entries must be array",
        );
        return;
    };
    let mut state = EntryState::default();
    validate_entries(entries, value, projection_context, report, &mut state);
    validate_omissions(
        manifest.get("omissions"),
        value,
        projection_context,
        report,
        &mut state,
    );
    validate_coverage(map, &state.refs, base, report);
    validate_projection_bytes(value, author_text, projection_text, entries, base, report);
}

fn validate_entries(
    entries: &[Value],
    root: &Value,
    context: Context<'_>,
    report: &mut Report,
    state: &mut EntryState,
) {
    let base = context.base;
    if entries.is_empty() {
        push_error(
            report,
            "adapter/app-map-manifest",
            format!("{base}/app_map.json#/projection_manifest/entries"),
            "entries must not be empty",
        );
    }
    for (index, entry) in entries.iter().enumerate() {
        let pointer = format!("{base}/app_map.json#/projection_manifest/entries/{index}");
        let Some(object) = entry.as_object() else {
            push_error(
                report,
                "adapter/app-map-entry",
                pointer,
                "entry must be object",
            );
            continue;
        };
        check_keys(
            report,
            "adapter/app-map-entry",
            &pointer,
            object,
            &[
                "id",
                "title",
                "content",
                "source_reference",
                "token_estimate",
            ],
        );
        validate_entry_fields(object, &pointer, report, state, root, context);
    }
}

fn validate_entry_fields(
    object: &std::collections::BTreeMap<String, Value>,
    pointer: &str,
    report: &mut Report,
    state: &mut EntryState,
    root: &Value,
    entry_context: Context<'_>,
) {
    if let Some(id) = string_field(report, object, pointer, "id") {
        if !is_snake_id(id) {
            push_error(
                report,
                "adapter/app-map-entry",
                pointer.to_string(),
                "id must be snake_case",
            );
        }
        if !state.ids.insert(id.to_string()) {
            push_error(
                report,
                "adapter/app-map-entry",
                pointer.to_string(),
                "duplicate entry id",
            );
        }
    }
    let title = string_field(report, object, pointer, "title");
    let content = string_field(report, object, pointer, "content");
    if let (Some(title), Some(content)) = (title, content) {
        if title.lines().count() != 1 || content.lines().count() != 1 {
            push_error(
                report,
                "adapter/app-map-entry",
                pointer.to_string(),
                "title/content must be single-line",
            );
        }
        let expected = content.chars().count().div_ceil(4).max(1);
        if token_estimate(object) != Some(expected) {
            push_error(
                report,
                "adapter/app-map-token-estimate",
                pointer.to_string(),
                format!("token_estimate must be {expected}"),
            );
        }
    }
    if let Some(source) = string_field(report, object, pointer, "source_reference") {
        if !valid_source_reference(source, root, entry_context) {
            push_error(
                report,
                "adapter/app-map-source",
                pointer.to_string(),
                "source_reference unresolved",
            );
        }
        state.refs.push(source.to_string());
    }
}

fn token_estimate(object: &std::collections::BTreeMap<String, Value>) -> Option<usize> {
    let integer = object.get("token_estimate").and_then(Value::as_integer)?;
    if !(0..=1_000_000).contains(&integer) {
        return None;
    }
    usize::try_from(integer).ok()
}

fn validate_omissions(
    omissions: Option<&Value>,
    root: &Value,
    context: Context<'_>,
    report: &mut Report,
    state: &mut EntryState,
) {
    let base = context.base;
    let Some(omissions) = omissions.and_then(Value::as_array) else {
        push_error(
            report,
            "adapter/app-map-manifest",
            format!("{base}/app_map.json#/projection_manifest/omissions"),
            "omissions must be array",
        );
        return;
    };
    for (index, omission) in omissions.iter().enumerate() {
        let pointer = format!("{base}/app_map.json#/projection_manifest/omissions/{index}");
        let Some(object) = omission.as_object() else {
            push_error(
                report,
                "adapter/app-map-omission",
                pointer,
                "omission must be object",
            );
            continue;
        };
        check_keys(
            report,
            "adapter/app-map-omission",
            &pointer,
            object,
            &["source_reference", "reason"],
        );
        if let Some(source) = string_field(report, object, &pointer, "source_reference") {
            if !valid_source_reference(source, root, context) {
                push_error(
                    report,
                    "adapter/app-map-source",
                    pointer.clone(),
                    "source_reference unresolved",
                );
            }
            state.refs.push(source.to_string());
        }
        if string_field(report, object, &pointer, "reason").is_none() {
            push_error(
                report,
                "adapter/app-map-omission",
                pointer,
                "reason must be non-empty",
            );
        }
    }
}

fn validate_coverage(
    map: &std::collections::BTreeMap<String, Value>,
    refs: &[String],
    base: &str,
    report: &mut Report,
) {
    for key in map.keys() {
        if matches!(
            key.as_str(),
            "schema_version" | "app_id" | "version_range" | "projection_manifest"
        ) {
            continue;
        }
        if !refs
            .iter()
            .any(|reference| reference.contains(&format!("#/{key}")))
        {
            push_error(
                report,
                "adapter/app-map-coverage",
                format!("{base}/app_map.json#/{key}"),
                "knowledge domain is neither projected nor omitted",
            );
        }
    }
}

fn validate_projection_bytes(
    root: &Value,
    author_text: Option<&String>,
    projection_text: &str,
    entries: &[Value],
    base: &str,
    report: &mut Report,
) {
    let app_id = root
        .get("app_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected = canonical_projection_manifest(entries, app_id);
    if expected.as_bytes() != projection_text.as_bytes() {
        push_error(
            report,
            "adapter/projection-byte-drift",
            format!("{base}/memory/app_map.v1.json"),
            "projection does not match canonical manifest bytes",
        );
    }
    if author_text.is_some_and(|text| !text.ends_with('\n')) {
        push_error(
            report,
            "adapter/app-map",
            format!("{base}/app_map.json"),
            "file must end with LF",
        );
    }
}

fn valid_source_reference(reference: &str, root: &Value, context: Context<'_>) -> bool {
    if let Some(pointer) = reference.strip_prefix("bundle:app_map#") {
        return resolve_pointer(root, pointer).is_some();
    }
    for slot in ["selectors", "tools", "rollback", "interrupts"] {
        let Some(pointer) = reference.strip_prefix(&format!("bundle:{slot}#")) else {
            continue;
        };
        return context
            .documents
            .get(slot)
            .is_some_and(|document| resolve_pointer(document, pointer).is_some());
    }
    let Some(memory_reference) = reference.strip_prefix("memory:") else {
        return false;
    };
    let Some((path, anchor)) = memory_reference.split_once('#') else {
        return false;
    };
    if path.is_empty()
        || anchor.is_empty()
        || !path.starts_with("docs/memory/apps/")
        || path.contains("..")
        || path.contains('\\')
    {
        return false;
    }
    context.repo_root.join(path).is_file()
}

fn resolve_pointer<'a>(root: &'a Value, pointer: &str) -> Option<&'a Value> {
    if pointer.is_empty() {
        return Some(root);
    }
    let mut current = root;
    for token in pointer.strip_prefix('/')?.split('/') {
        let token = token.replace("~1", "/").replace("~0", "~");
        current = if let Some(object) = current.as_object() {
            object.get(&token)?
        } else {
            let array = current.as_array()?;
            array.get(token.parse::<usize>().ok()?)?
        };
    }
    Some(current)
}

pub(super) fn canonical_projection_manifest(entries: &[Value], app_id: &str) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|entry| entry_id(entry).to_string());
    let mut output = format!(
        "{{\n  \"version\": 1,\n  \"app_id\": \"{}\",\n  \"entries\": [\n",
        json_escape(app_id)
    );
    for (index, entry) in sorted.iter().enumerate() {
        output.push_str("    {\n");
        for field in ["id", "title", "content", "source_reference"] {
            output.push_str("      \"");
            output.push_str(field);
            output.push_str("\": \"");
            output.push_str(&json_escape(
                entry.get(field).and_then(Value::as_str).unwrap_or_default(),
            ));
            output.push_str("\",\n");
        }
        let estimate = entry.as_object().and_then(token_estimate).unwrap_or(0);
        output.push_str("      \"token_estimate\": ");
        output.push_str(&estimate.to_string());
        output.push_str("\n    }");
        if index + 1 != sorted.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("  ]\n}\n");
    output
}

fn entry_id(entry: &Value) -> &str {
    entry.get("id").and_then(Value::as_str).unwrap_or_default()
}

fn is_snake_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
