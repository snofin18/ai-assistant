//! `adapter-validate --report-only` entry, manifest parsing, and bundle containment.
//!
//! Responsibility: resolve one Adapter bundle, read all six JSON documents, and delegate
//! D2-D6 checks to focused modules. No code path writes to the Adapter tree.
//!
//! Boundaries: TOML parsing is limited to the fields ADR-0105 needs. JSON uses the in-repo
//! zero-dependency parser. Generated capability/postcondition metadata is consumed from
//! `protocol/adapter-catalog/adapter-catalog-1.0.json`.
//!
//! Invariants:
//! 1. bundle paths are relative, `/`-separated, canonical, contained, and unique;
//! 2. all six JSON files and `app_id` identity are checked before construct validation;
//! 3. findings are deterministic and report-only;
//! 4. construct validators never infer fields from current packs.

mod app_map;
mod interrupts;
mod rollback;
mod selectors;
mod tool_bounds;
mod tools;

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "../adapter_validate_excel_tests.rs"]
mod excel_tests;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::adapter_catalog;
use crate::report::{Finding, Report, Severity};
use crate::serde_json_lite::{self, Value};

const BUNDLE_KEYS: [&str; 7] = [
    "app_map",
    "app_map_projection",
    "app_map_projection_model",
    "selectors",
    "tools",
    "rollback",
    "interrupts",
];
const BUNDLE_SLOTS: [&str; 6] = [
    "app_map",
    "app_map_projection",
    "selectors",
    "tools",
    "rollback",
    "interrupts",
];

struct Manifest {
    app_id: String,
    bundle: BTreeMap<String, String>,
}

#[derive(Default)]
struct ManifestBuilder {
    schema_version: Option<u64>,
    app_id: Option<String>,
    bundle: BTreeMap<String, String>,
}

pub struct Bundle {
    pub adapter_relative: String,
    pub repo_root: PathBuf,
    pub values: BTreeMap<String, Value>,
    pub texts: BTreeMap<String, String>,
    pub catalog: Value,
}

#[derive(Clone, Debug)]
pub struct ToolInfo {
    pub name: String,
    pub effect: String,
    pub reversibility: String,
    pub risk_level: String,
    pub requires_approval: bool,
    pub tags: BTreeSet<String>,
}

/// Validate one Adapter bundle without modifying any file.
///
/// # Errors
///
/// Returns an error for command usage or repository-level IO failures. Invalid Adapter content
/// is rendered as findings and returns exit code 1.
pub fn run(
    repo_root: &Path,
    adapter_operand: &str,
    report_only: bool,
    output: &mut dyn Write,
) -> Result<u8, String> {
    if !report_only {
        return Err("adapter-validate requires explicit --report-only".to_string());
    }
    let adapter_root = repo_root
        .join(adapter_operand)
        .canonicalize()
        .map_err(|error| format!("adapter root `{adapter_operand}`: {error}"))?;
    let manifest_text = std::fs::read_to_string(adapter_root.join("adapter.toml"))
        .map_err(|error| format!("read {adapter_operand}/adapter.toml: {error}"))?;
    let catalog_text = std::fs::read_to_string(repo_root.join(adapter_catalog::CATALOG_PATH))
        .map_err(|error| format!("read {}: {error}", adapter_catalog::CATALOG_PATH))?;
    let catalog = serde_json_lite::parse(&catalog_text)
        .map_err(|error| format!("parse {}: {error}", adapter_catalog::CATALOG_PATH))?;
    let mut report = Report::new("adapter-validate");
    report.scanned_files = 1;
    let manifest = match parse_manifest(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            push_error(
                &mut report,
                "adapter/manifest",
                format!("{adapter_operand}/adapter.toml"),
                error,
            );
            return finish(report, output);
        }
    };
    let Some(bundle) = read_bundle(
        repo_root,
        &adapter_root,
        adapter_operand,
        &manifest,
        catalog,
        &mut report,
    ) else {
        return finish(report, output);
    };
    validate_constructs(&bundle, &mut report);
    finish(report, output)
}

fn finish(mut report: Report, output: &mut dyn Write) -> Result<u8, String> {
    sort_findings(&mut report);
    report.render(output).map_err(|error| error.to_string())?;
    Ok(u8::from(report.is_failure()))
}

fn validate_constructs(bundle: &Bundle, report: &mut Report) {
    if let (Some(app_map), Some(projection)) = (
        bundle.values.get("app_map"),
        bundle.texts.get("app_map_projection"),
    ) {
        app_map::validate(
            app_map,
            bundle.texts.get("app_map"),
            projection,
            app_map::Context::new(&bundle.values, &bundle.repo_root, &bundle.adapter_relative),
            report,
        );
    }
    if let Some(value) = bundle.values.get("selectors") {
        selectors::validate(value, &bundle.adapter_relative, report);
    }
    let tools = bundle.values.get("tools").map_or_else(Vec::new, |value| {
        tools::validate(value, &bundle.catalog, &bundle.adapter_relative, report)
    });
    if let Some(value) = bundle.values.get("rollback") {
        rollback::validate(
            value,
            &tools,
            &bundle.catalog,
            &bundle.adapter_relative,
            report,
        );
    }
    if let Some(value) = bundle.values.get("interrupts") {
        interrupts::validate(value, &tools, &bundle.adapter_relative, report);
    }
}

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let mut section = String::new();
    let mut builder = ManifestBuilder::default();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(next) = parse_section(line, line_index)? {
            section = next;
            continue;
        }
        let Some((key, value)) = parse_assignment(line) else {
            if section == "bundle" {
                return Err(format!("line {}: expected key = value", line_index + 1));
            }
            continue;
        };
        builder.push(&section, key, value, line_index)?;
    }
    builder.finish()
}

fn parse_section(line: &str, line_index: usize) -> Result<Option<String>, String> {
    if !line.starts_with('[') {
        return Ok(None);
    }
    if !line.ends_with(']') {
        return Err(format!("line {}: malformed section", line_index + 1));
    }
    Ok(Some(line.trim_matches(&['[', ']'][..]).trim().to_string()))
}

fn parse_assignment(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
        .map(|(key, value)| (key.trim(), value.trim()))
}

impl ManifestBuilder {
    fn push(
        &mut self,
        section: &str,
        key: &str,
        value: &str,
        line_index: usize,
    ) -> Result<(), String> {
        match (section, key) {
            ("", "schema_version") => {
                self.schema_version = Some(value.parse::<u64>().map_err(|_| {
                    format!("line {}: schema_version must be integer", line_index + 1)
                })?);
            }
            ("adapter", "app_id") => self.app_id = Some(parse_toml_string(value)?),
            ("bundle", _) => self.push_bundle(key, value, line_index)?,
            _ => {}
        }
        Ok(())
    }

    fn push_bundle(&mut self, key: &str, value: &str, line_index: usize) -> Result<(), String> {
        if !BUNDLE_KEYS.contains(&key) {
            return Err(format!(
                "line {}: unknown bundle key `{key}`",
                line_index + 1
            ));
        }
        if self
            .bundle
            .insert(key.to_string(), parse_toml_string(value)?)
            .is_some()
        {
            return Err(format!(
                "line {}: duplicate bundle key `{key}`",
                line_index + 1
            ));
        }
        Ok(())
    }

    fn finish(self) -> Result<Manifest, String> {
        if self.schema_version != Some(1) {
            return Err("schema_version must be 1".to_string());
        }
        let app_id = self
            .app_id
            .filter(|value| !value.trim().is_empty())
            .ok_or("adapter.app_id missing")?;
        let missing: Vec<&str> = BUNDLE_KEYS
            .iter()
            .copied()
            .filter(|key| !self.bundle.contains_key(*key))
            .collect();
        if !missing.is_empty() {
            return Err(format!("missing bundle keys: {}", missing.join(", ")));
        }
        if self
            .bundle
            .get("app_map_projection_model")
            .map(String::as_str)
            != Some("core_app_map_v1")
        {
            return Err("app_map_projection_model must be core_app_map_v1".to_string());
        }
        Ok(Manifest {
            app_id,
            bundle: self.bundle,
        })
    }
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in line.bytes().enumerate() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' && in_string {
            escaped = true;
        } else if byte == b'"' {
            in_string = !in_string;
        } else if byte == b'#' && !in_string {
            return line.get(..index).unwrap_or(line);
        }
    }
    line
}

fn parse_toml_string(value: &str) -> Result<String, String> {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(format!("expected quoted TOML string, got `{value}`"));
    };
    if inner.contains('\\') || inner.contains('"') {
        return Err("bundle path strings must not contain escapes or quotes".to_string());
    }
    if inner.trim().is_empty() {
        return Err("bundle path must not be empty".to_string());
    }
    Ok(inner.to_string())
}

fn read_bundle(
    repo_root: &Path,
    adapter_root: &Path,
    adapter_operand: &str,
    manifest: &Manifest,
    catalog: Value,
    report: &mut Report,
) -> Option<Bundle> {
    let mut values = BTreeMap::new();
    let mut texts = BTreeMap::new();
    let mut reader = BundleReader {
        adapter_root,
        adapter_operand,
        app_id: &manifest.app_id,
        canonical_targets: BTreeSet::new(),
        report,
    };
    for slot in BUNDLE_SLOTS {
        let Some(authored) = manifest.bundle.get(slot) else {
            continue;
        };
        let Some((value, text)) = reader.read_slot(slot, authored) else {
            continue;
        };
        values.insert(slot.to_string(), value);
        texts.insert(slot.to_string(), text);
    }
    if values.len() != BUNDLE_SLOTS.len() {
        return None;
    }
    Some(Bundle {
        adapter_relative: adapter_operand.trim_end_matches('/').to_string(),
        repo_root: repo_root.to_path_buf(),
        values,
        texts,
        catalog,
    })
}

struct BundleReader<'a, 'b> {
    adapter_root: &'a Path,
    adapter_operand: &'a str,
    app_id: &'a str,
    canonical_targets: BTreeSet<PathBuf>,
    report: &'b mut Report,
}

impl BundleReader<'_, '_> {
    fn read_slot(&mut self, slot: &str, authored: &str) -> Option<(Value, String)> {
        let relative = match normalize_bundle_path(authored) {
            Ok(path) => path,
            Err(error) => {
                push_error(
                    self.report,
                    "adapter/bundle-path",
                    format!("{}/adapter.toml", self.adapter_operand),
                    format!("{slot}: {error}"),
                );
                return None;
            }
        };
        let canonical = match self.adapter_root.join(&relative).canonicalize() {
            Ok(path) => path,
            Err(error) => {
                push_error(
                    self.report,
                    "adapter/bundle-read",
                    format!("{}/{}", self.adapter_operand, relative.display()),
                    error.to_string(),
                );
                return None;
            }
        };
        if !canonical.starts_with(self.adapter_root) {
            push_error(
                self.report,
                "adapter/bundle-contained",
                format!("{}/{}", self.adapter_operand, relative.display()),
                "canonical target escapes bundle root",
            );
            return None;
        }
        if !self.canonical_targets.insert(canonical.clone()) {
            push_error(
                self.report,
                "adapter/bundle-duplicate-target",
                format!("{}/{}", self.adapter_operand, relative.display()),
                "same canonical file occupies multiple bundle slots",
            );
            return None;
        }
        let text = match std::fs::read_to_string(&canonical) {
            Ok(text) => text,
            Err(error) => {
                push_error(
                    self.report,
                    "adapter/bundle-read",
                    format!("{}/{}", self.adapter_operand, relative.display()),
                    error.to_string(),
                );
                return None;
            }
        };
        let value = match serde_json_lite::parse(&text) {
            Ok(value) => value,
            Err(error) => {
                push_error(
                    self.report,
                    "adapter/json",
                    format!("{}/{}", self.adapter_operand, relative.display()),
                    error,
                );
                return None;
            }
        };
        self.report.scanned_files += 1;
        check_identity(
            slot,
            &value,
            self.app_id,
            self.adapter_operand,
            &relative,
            self.report,
        );
        Some((value, text))
    }
}

fn normalize_bundle_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.contains('\\') || raw.starts_with('/') {
        return Err("must be a relative `/`-separated path".to_string());
    }
    for component in Path::new(raw).components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("must not contain absolute, `.` or `..` segments".to_string());
        }
    }
    Ok(PathBuf::from(raw))
}

fn check_identity(
    slot: &str,
    value: &Value,
    app_id: &str,
    adapter_operand: &str,
    relative: &Path,
    report: &mut Report,
) {
    let path = format!("{adapter_operand}/{}", relative.display());
    if value.get("app_id").and_then(Value::as_str) != Some(app_id) {
        push_error(
            report,
            "adapter/app-id",
            path.clone(),
            format!("top-level app_id must equal `{app_id}`"),
        );
    }
    let version = if slot == "app_map_projection" {
        value.get("version")
    } else {
        value.get("schema_version")
    };
    if version.and_then(Value::as_integer) != Some(1) {
        push_error(
            report,
            "adapter/version",
            path,
            "document version must be 1",
        );
    }
}

pub fn check_keys(
    report: &mut Report,
    rule: &'static str,
    pointer: &str,
    object: &BTreeMap<String, Value>,
    allowed: &[&str],
) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            push_error(
                report,
                rule,
                pointer.to_string(),
                format!("unknown field `{key}`"),
            );
        }
    }
}

pub fn string_field<'a>(
    report: &mut Report,
    object: &'a BTreeMap<String, Value>,
    pointer: &str,
    field: &str,
) -> Option<&'a str> {
    let value = object.get(field).and_then(Value::as_str);
    if value.is_none_or(|text| text.trim().is_empty()) {
        push_error(
            report,
            "adapter/json",
            pointer.to_string(),
            format!("`{field}` must be non-empty string"),
        );
    }
    value
}

pub fn catalog_strings(catalog: &Value, field: &str) -> BTreeSet<String> {
    catalog
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

pub fn string_array(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

pub fn push_error(
    report: &mut Report,
    rule: &'static str,
    path: String,
    message: impl Into<String>,
) {
    report.push(Finding::new(rule, Severity::Error, path, 0, message));
}

fn validate_numeric_constraint(
    object: &BTreeMap<String, Value>,
    constraint: &Value,
    pointer: &str,
    report: &mut Report,
) {
    let left_field = constraint
        .get("left_field")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let right_field = constraint
        .get("right_field")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let left = object.get(left_field);
    let right = object.get(right_field);
    if !left
        .zip(right)
        .is_some_and(|(left, right)| catalog_number_less_than_or_equal(left, right))
    {
        push_error(
            report,
            "adapter/postcondition",
            pointer.to_string(),
            format!("violates catalog constraint `{left_field} <= {right_field}`"),
        );
    }
}

fn catalog_number_less_than_or_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left <= right,
        (Value::Integer(left), Value::Number(right)) => integer_less_equal_float(*left, *right),
        (Value::Number(left), Value::Integer(right)) => float_less_equal_integer(*left, *right),
        (Value::Number(left), Value::Number(right)) => left <= right,
        _ => false,
    }
}

fn integer_less_equal_float(integer: i64, float: f64) -> bool {
    const I64_UPPER_BOUND: f64 = 9_223_372_036_854_775_808.0;
    const I64_LOWER_BOUND: f64 = -9_223_372_036_854_775_808.0;
    if float >= I64_UPPER_BOUND {
        return true;
    }
    if float < I64_LOWER_BOUND {
        return false;
    }
    integral_float_to_i128(float.floor())
        .is_some_and(|float_floor| i128::from(integer) <= float_floor)
}

fn float_less_equal_integer(float: f64, integer: i64) -> bool {
    const I64_UPPER_BOUND: f64 = 9_223_372_036_854_775_808.0;
    const I64_LOWER_BOUND: f64 = -9_223_372_036_854_775_808.0;
    if float >= I64_UPPER_BOUND {
        return false;
    }
    if float < I64_LOWER_BOUND {
        return true;
    }
    integral_float_to_i128(float.ceil())
        .is_some_and(|float_ceiling| float_ceiling <= i128::from(integer))
}

fn integral_float_to_i128(value: f64) -> Option<i128> {
    if !value.is_finite() {
        return None;
    }
    format!("{value:.0}").parse::<i128>().ok()
}

fn catalog_value_matches(value: &Value, field_type: &str, catalog: &Value) -> bool {
    if field_type.contains('|') {
        return field_type
            .split('|')
            .any(|candidate| value.as_str() == Some(candidate));
    }
    match field_type {
        "non_empty_string" => value.as_str().is_some_and(|text| !text.trim().is_empty()),
        "optional_string" | "string" => value.as_str().is_some(),
        "scalar" => !matches!(value, Value::Array(_) | Value::Object(_)),
        "number" => matches!(value, Value::Integer(_) | Value::Number(_)),
        "positive_integer" => value.as_integer().is_some_and(|integer| integer > 0),
        "fingerprint" => value.as_str().is_some_and(is_valid_fingerprint),
        "ErrorCode" => value
            .as_str()
            .is_some_and(|code| catalog_strings(catalog, "error_codes").contains(code)),
        _ => false,
    }
}

fn is_valid_fingerprint(value: &str) -> bool {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn sort_findings(report: &mut Report) {
    report.findings.sort_by(|left, right| {
        left.rule
            .cmp(right.rule)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.message.cmp(&right.message))
    });
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]
#[path = "../adapter_validate_tests.rs"]
mod tests;
