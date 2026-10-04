//! # replay 序列解析与树级 diff
//!
//! 职责：读取 Recording v2 的 UIA 树快照序列，校验有界资源与树不变量，逐步计算
//! 节点 / 属性 / 文本差异，并把事实与 fixture 中的 expected diff 对照。
//!
//! ## 边界（不做什么）
//! - 不采集真实桌面、不驱动应用、不读取商业应用数据。
//! - 不重新定义平台公共类型，也不改变 `assistant-replay` 的 Recording v1 provider。
//! - 只消费仓库内 fixture；所有输入都是不可信输入，先解析、校验，再 diff。
//!
//! ## 不变量
//! 1. 单文件、步数、每步节点数与 expected change 数都有硬上限（ADR-0063）。
//! 2. 同一输入必须产生同一 diff 顺序；输出不依赖 `HashMap` 遍历顺序。
//! 3. 未知版本、孤儿父节点、环、重复 handle、悬空文本引用均显式失败。
//! 4. expected diff 与实际 diff 不一致时返回退出码 1，并输出两侧可定位项。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::serde_json_lite::{Value, parse};

/// Recording v2 sequence version.
pub const SEQUENCE_VERSION: u32 = 2;
/// 单份录制文件的硬上限。
pub const MAX_RECORDING_BYTES: u64 = 1_048_576;
/// 单份序列的步数上限。
pub const MAX_STEPS: usize = 128;
/// 单快照节点数上限。
pub const MAX_NODES_PER_SNAPSHOT: usize = 4_096;
/// 单步 expected / actual diff 项上限。
pub const MAX_CHANGES_PER_STEP: usize = 4_096;

#[derive(Debug, Clone)]
struct RecordingSequence {
    schema_version: u32,
    recording_id: String,
    application: String,
    suite: String,
    baseline: Snapshot,
    steps: Vec<ReplayStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    nodes: BTreeMap<u64, Node>,
    read_text: BTreeMap<u64, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    parent_id: Option<u64>,
    role: String,
    automation_id: Option<String>,
    class_name: Option<String>,
    name: String,
    bounds: [i64; 4],
    is_offscreen: bool,
    is_enabled: bool,
    patterns: Vec<String>,
}

#[derive(Debug, Clone)]
struct ReplayStep {
    step_id: String,
    expected_changes: Vec<TreeChange>,
    after: Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeChange {
    kind: TreeChangeKind,
    node_handle_id: u64,
    field: Option<String>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeChangeKind {
    NodeRemoved,
    NodeAdded,
    PropertyChanged,
    TextChanged,
}

impl TreeChangeKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "node_removed" => Ok(Self::NodeRemoved),
            "node_added" => Ok(Self::NodeAdded),
            "property_changed" => Ok(Self::PropertyChanged),
            "text_changed" => Ok(Self::TextChanged),
            other => Err(format!("unknown tree change kind `{other}`")),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::NodeRemoved => "node_removed",
            Self::NodeAdded => "node_added",
            Self::PropertyChanged => "property_changed",
            Self::TextChanged => "text_changed",
        }
    }

    const fn order(self) -> u8 {
        match self {
            Self::NodeRemoved => 0,
            Self::NodeAdded => 1,
            Self::PropertyChanged => 2,
            Self::TextChanged => 3,
        }
    }
}

/// 回放一个 suite：只接受显式白名单 suite，避免路径穿越与未知配置静默通过。
pub fn run_suite(
    repo_root: &Path,
    suite: &str,
    output: &mut dyn std::io::Write,
) -> Result<u8, String> {
    if suite != "core" {
        return Err(format!(
            "unknown replay suite `{suite}`; supported suite = core"
        ));
    }
    let suite_dir = repo_root.join("fixtures").join("recordings").join(suite);
    if !suite_dir.is_dir() {
        return Err(format!(
            "missing replay fixture suite directory: {}",
            suite_dir.display()
        ));
    }
    let mut fixtures = Vec::new();
    let entries = std::fs::read_dir(&suite_dir)
        .map_err(|error| format!("read replay suite {} failed: {error}", suite_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read replay suite entry failed: {error}"))?;
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "stat replay fixture {} failed: {error}",
                entry.path().display()
            )
        })?;
        if file_type.is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .ends_with(".sequence.json")
        {
            fixtures.push(entry.path());
        }
    }
    fixtures.sort();
    if fixtures.is_empty() {
        return Err(format!(
            "missing replay fixture: no `*.sequence.json` under {}",
            suite_dir.display()
        ));
    }

    write_line(output, &format!("== replay suite: {suite} =="))?;
    let mut failed = false;
    for fixture in fixtures {
        write_line(output, &format!("fixture: {}", fixture.display()))?;
        let code = run_sequence(&fixture, output)?;
        failed |= code != crate::EXIT_OK;
    }
    write_line(
        output,
        if failed {
            "-- verdict: FAILED"
        } else {
            "-- verdict: PASSED"
        },
    )?;
    Ok(if failed {
        crate::EXIT_FINDINGS
    } else {
        crate::EXIT_OK
    })
}

/// 回放单份 Recording v2 序列文件。
pub fn run_sequence(path: &Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    let content = read_bounded(path)?;
    let sequence = match parse_sequence(&content) {
        Ok(sequence) => sequence,
        Err(error) => {
            write_line(output, "== replay ==")?;
            write_line(output, &format!("file: {}", path.display()))?;
            write_line(output, &format!("parse error: {error}"))?;
            write_line(output, "-- verdict: FAILED")?;
            return Ok(crate::EXIT_FINDINGS);
        }
    };
    replay_sequence(path, &sequence, output)
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("stat replay fixture {} failed: {error}", path.display()))?;
    if metadata.len() > MAX_RECORDING_BYTES {
        return Err(format!(
            "replay fixture {} exceeds size cap {} bytes (actual {})",
            path.display(),
            MAX_RECORDING_BYTES,
            metadata.len()
        ));
    }
    std::fs::read_to_string(path)
        .map_err(|error| format!("read replay fixture {} failed: {error}", path.display()))
}

fn parse_sequence(content: &str) -> Result<RecordingSequence, String> {
    let value = parse(content).map_err(|error| format!("invalid JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "top-level recording must be an object".to_string())?;
    let schema_version = required_u64(object, "schema_version")?;
    if schema_version != u64::from(SEQUENCE_VERSION) {
        return Err(format!(
            "unsupported recording version {schema_version}; expected {SEQUENCE_VERSION}"
        ));
    }
    let recording_id = required_string(object, "recording_id")?;
    let application = required_string(object, "application")?;
    let suite = required_string(object, "suite")?;
    if suite.trim().is_empty() {
        return Err("suite must not be blank".to_string());
    }
    let baseline = parse_snapshot(required_object(object, "baseline")?)?;
    let step_values = required_array(object, "steps")?;
    if step_values.is_empty() {
        return Err("steps must not be empty".to_string());
    }
    if step_values.len() > MAX_STEPS {
        return Err(format!("steps exceeds cap {MAX_STEPS}"));
    }
    let mut steps = Vec::with_capacity(step_values.len());
    let mut step_ids = BTreeSet::new();
    for step_value in step_values {
        let step = parse_step(step_value)?;
        if !step_ids.insert(step.step_id.clone()) {
            return Err(format!("duplicate step_id `{}`", step.step_id));
        }
        steps.push(step);
    }
    Ok(RecordingSequence {
        schema_version: u32::try_from(schema_version)
            .map_err(|_| "schema_version does not fit in u32".to_string())?,
        recording_id,
        application,
        suite,
        baseline,
        steps,
    })
}

/// Read the declared `schema_version` without committing to a recording dialect.
///
/// Legacy v1 fixtures omit `schema_version` and use `version`; returning `0` lets
/// the caller keep the old dry-run path without guessing from raw text.
pub fn declared_sequence_version(content: &str) -> Result<u32, String> {
    let value = parse(content).map_err(|error| format!("invalid JSON: {error}"))?;
    let Some(object) = value.as_object() else {
        return Ok(0);
    };
    if !object.contains_key("schema_version") {
        return Ok(0);
    }
    let version = required_u64(object, "schema_version")?;
    u32::try_from(version).map_err(|_| "schema_version does not fit in u32".to_string())
}

fn parse_snapshot(object: &BTreeMap<String, Value>) -> Result<Snapshot, String> {
    if let Some(window) = object.get("window") {
        let window = window
            .as_object()
            .ok_or_else(|| "snapshot.window must be an object".to_string())?;
        let window_handle = required_u64(window, "local_handle_id")?;
        if window_handle == 0 {
            return Err("snapshot.window.local_handle_id must be > 0".to_string());
        }
    }
    let node_values = required_array(object, "nodes")?;
    if node_values.is_empty() {
        return Err("snapshot.nodes must not be empty".to_string());
    }
    if node_values.len() > MAX_NODES_PER_SNAPSHOT {
        return Err(format!(
            "snapshot.nodes exceeds cap {MAX_NODES_PER_SNAPSHOT}"
        ));
    }
    let mut nodes = BTreeMap::new();
    for node_value in node_values {
        let node_object = node_value
            .as_object()
            .ok_or_else(|| "snapshot node must be an object".to_string())?;
        let handle = required_u64(node_object, "local_handle_id")?;
        if handle == 0 {
            return Err("snapshot node local_handle_id must be > 0".to_string());
        }
        let node = parse_node(node_object)?;
        if nodes.insert(handle, node).is_some() {
            return Err(format!("duplicate node handle {handle}"));
        }
    }
    validate_parent_graph(&nodes)?;
    let read_text = parse_read_text(object, &nodes)?;
    Ok(Snapshot { nodes, read_text })
}

fn parse_node(object: &BTreeMap<String, Value>) -> Result<Node, String> {
    let patterns = required_array(object, "patterns")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| "node.patterns entries must be strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Node {
        parent_id: optional_u64(object, "parent_handle_id")?,
        role: required_string(object, "role")?,
        automation_id: optional_string(object, "automation_id")?,
        class_name: optional_string(object, "class_name")?,
        name: required_string(object, "name")?,
        bounds: parse_bounds(required_array(object, "bounds")?)?,
        is_offscreen: required_bool(object, "is_offscreen")?,
        is_enabled: required_bool(object, "is_enabled")?,
        patterns,
    })
}

fn parse_bounds(values: &[Value]) -> Result<[i64; 4], String> {
    if values.len() != 4 {
        return Err("node.bounds must contain 4 integers".to_string());
    }
    let mut bounds = [0_i64; 4];
    for (index, value) in values.iter().enumerate() {
        let Value::Number(number) = value else {
            return Err("node.bounds entries must be numbers".to_string());
        };
        if !number.is_finite() || number.fract() != 0.0 {
            return Err("node.bounds entries must be integers".to_string());
        }
        let converted = number
            .to_string()
            .parse::<i64>()
            .map_err(|_| "node.bounds entry does not fit in i64".to_string())?;
        if let Some(slot) = bounds.get_mut(index) {
            *slot = converted;
        }
    }
    Ok(bounds)
}

fn parse_read_text(
    object: &BTreeMap<String, Value>,
    nodes: &BTreeMap<u64, Node>,
) -> Result<BTreeMap<u64, String>, String> {
    let mut read_text = BTreeMap::new();
    for value in required_array(object, "read_text")? {
        let entry = value
            .as_object()
            .ok_or_else(|| "read_text entry must be an object".to_string())?;
        let handle = required_u64(entry, "element_handle_id")?;
        if !nodes.contains_key(&handle) {
            return Err(format!("read_text references missing node {handle}"));
        }
        if read_text
            .insert(handle, required_string(entry, "text")?)
            .is_some()
        {
            return Err(format!("duplicate read_text outcome for node {handle}"));
        }
    }
    Ok(read_text)
}

fn validate_parent_graph(nodes: &BTreeMap<u64, Node>) -> Result<(), String> {
    for (&handle, node) in nodes {
        let Some(parent) = node.parent_id else {
            continue;
        };
        if !nodes.contains_key(&parent) {
            return Err(format!("node {handle} references missing parent {parent}"));
        }
        let mut seen = BTreeSet::new();
        let mut current = Some(handle);
        while let Some(candidate) = current {
            if !seen.insert(candidate) {
                return Err(format!("parent cycle at node {handle}"));
            }
            current = nodes.get(&candidate).and_then(|node| node.parent_id);
        }
    }
    Ok(())
}

fn parse_step(value: &Value) -> Result<ReplayStep, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "step must be an object".to_string())?;
    let changes = required_array(object, "expected_changes")?;
    if changes.len() > MAX_CHANGES_PER_STEP {
        return Err(format!(
            "expected_changes exceeds cap {MAX_CHANGES_PER_STEP}"
        ));
    }
    Ok(ReplayStep {
        step_id: required_string(object, "step_id")?,
        expected_changes: changes
            .iter()
            .map(parse_change)
            .collect::<Result<Vec<_>, _>>()?,
        after: parse_snapshot(required_object(object, "after")?)?,
    })
}

fn parse_change(value: &Value) -> Result<TreeChange, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "expected change must be an object".to_string())?;
    Ok(TreeChange {
        kind: TreeChangeKind::parse(&required_string(object, "kind")?)?,
        node_handle_id: required_u64(object, "node_handle_id")?,
        field: optional_string(object, "field")?,
        before: optional_string(object, "before")?,
        after: optional_string(object, "after")?,
    })
}

fn replay_sequence(
    path: &Path,
    sequence: &RecordingSequence,
    output: &mut dyn std::io::Write,
) -> Result<u8, String> {
    write_line(output, "== replay ==")?;
    write_line(output, &format!("file: {}", path.display()))?;
    write_line(
        output,
        &format!("schema_version: {}", sequence.schema_version),
    )?;
    write_line(output, &format!("recording_id: {}", sequence.recording_id))?;
    write_line(output, &format!("application: {}", sequence.application))?;
    write_line(output, &format!("suite: {}", sequence.suite))?;
    write_line(output, &format!("steps: {}", sequence.steps.len()))?;
    let mut previous = sequence.baseline.clone();
    let mut total_changes = 0_usize;
    for step in &sequence.steps {
        let mut actual = diff_snapshots(&previous, &step.after);
        let mut expected = step.expected_changes.clone();
        sort_changes(&mut actual);
        sort_changes(&mut expected);
        if actual != expected {
            write_line(output, &format!("step: {} MISMATCH", step.step_id))?;
            for change in &expected {
                write_line(
                    output,
                    &format!(
                        "  expected: {}",
                        render_change(change, &previous, &step.after)
                    ),
                )?;
            }
            for change in &actual {
                write_line(
                    output,
                    &format!(
                        "  actual:   {}",
                        render_change(change, &previous, &step.after)
                    ),
                )?;
            }
            write_line(output, "-- verdict: FAILED")?;
            return Ok(crate::EXIT_FINDINGS);
        }
        total_changes = total_changes.saturating_add(actual.len());
        write_line(
            output,
            &format!("step: {} OK ({} change(s))", step.step_id, actual.len()),
        )?;
        for change in &actual {
            write_line(
                output,
                &format!("  {}", render_change(change, &previous, &step.after)),
            )?;
        }
        previous = step.after.clone();
    }
    write_line(
        output,
        &format!(
            "-- summary: {} step(s), {total_changes} change(s), 0 mismatch",
            sequence.steps.len()
        ),
    )?;
    write_line(output, "-- verdict: PASSED")?;
    Ok(crate::EXIT_OK)
}

fn diff_snapshots(before: &Snapshot, after: &Snapshot) -> Vec<TreeChange> {
    let mut changes = Vec::new();
    let handles: BTreeSet<u64> = before
        .nodes
        .keys()
        .chain(after.nodes.keys())
        .copied()
        .collect();
    for handle in handles {
        match (before.nodes.get(&handle), after.nodes.get(&handle)) {
            (Some(_), None) => changes.push(TreeChange {
                kind: TreeChangeKind::NodeRemoved,
                node_handle_id: handle,
                field: None,
                before: None,
                after: None,
            }),
            (None, Some(_)) => changes.push(TreeChange {
                kind: TreeChangeKind::NodeAdded,
                node_handle_id: handle,
                field: None,
                before: None,
                after: None,
            }),
            (Some(before_node), Some(after_node)) => {
                diff_node_properties(handle, before_node, after_node, &mut changes);
            }
            (None, None) => {}
        }
    }
    let text_handles: BTreeSet<u64> = before
        .read_text
        .keys()
        .chain(after.read_text.keys())
        .copied()
        .collect();
    for handle in text_handles {
        let before_text = before.read_text.get(&handle);
        let after_text = after.read_text.get(&handle);
        if before_text == after_text {
            continue;
        }
        let kind = if before_text.is_some() && after_text.is_some() {
            TreeChangeKind::TextChanged
        } else {
            TreeChangeKind::PropertyChanged
        };
        changes.push(TreeChange {
            kind,
            node_handle_id: handle,
            field: Some("read_text".to_string()),
            before: before_text.cloned(),
            after: after_text.cloned(),
        });
    }
    changes
}

fn diff_node_properties(handle: u64, before: &Node, after: &Node, changes: &mut Vec<TreeChange>) {
    let before_properties = node_properties(before);
    let after_properties = node_properties(after);
    for (field, before_value) in before_properties {
        let Some(after_value) = after_properties.get(field) else {
            continue;
        };
        if before_value != *after_value {
            changes.push(TreeChange {
                kind: TreeChangeKind::PropertyChanged,
                node_handle_id: handle,
                field: Some(field.to_string()),
                before: Some(before_value),
                after: Some(after_value.clone()),
            });
        }
    }
}

fn node_properties(node: &Node) -> BTreeMap<&'static str, String> {
    let mut properties = BTreeMap::new();
    properties.insert("parent_handle_id", option_number(node.parent_id));
    properties.insert("role", node.role.clone());
    properties.insert(
        "automation_id",
        node.automation_id.clone().unwrap_or_default(),
    );
    properties.insert("class_name", node.class_name.clone().unwrap_or_default());
    properties.insert("name", node.name.clone());
    properties.insert(
        "bounds",
        format!(
            "{},{},{},{}",
            node.bounds.first().copied().unwrap_or_default(),
            node.bounds.get(1).copied().unwrap_or_default(),
            node.bounds.get(2).copied().unwrap_or_default(),
            node.bounds.get(3).copied().unwrap_or_default()
        ),
    );
    properties.insert("is_offscreen", node.is_offscreen.to_string());
    properties.insert("is_enabled", node.is_enabled.to_string());
    properties.insert("patterns", node.patterns.join("|"));
    properties
}

fn option_number(value: Option<u64>) -> String {
    value.map_or_else(String::new, |number| number.to_string())
}

fn sort_changes(changes: &mut [TreeChange]) {
    changes.sort_by(|left, right| {
        (
            left.kind.order(),
            left.node_handle_id,
            left.field.as_deref().unwrap_or_default(),
            left.before.as_deref().unwrap_or_default(),
            left.after.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                right.kind.order(),
                right.node_handle_id,
                right.field.as_deref().unwrap_or_default(),
                right.before.as_deref().unwrap_or_default(),
                right.after.as_deref().unwrap_or_default(),
            ))
    });
}

fn render_change(change: &TreeChange, before: &Snapshot, after: &Snapshot) -> String {
    let label = after
        .nodes
        .get(&change.node_handle_id)
        .or_else(|| before.nodes.get(&change.node_handle_id))
        .and_then(|node| node.automation_id.as_deref())
        .map_or_else(|| "(no automation_id)".to_string(), ToString::to_string);
    let field = change.field.as_deref().unwrap_or("-");
    let before_value = change.before.as_deref().unwrap_or("-");
    let after_value = change.after.as_deref().unwrap_or("-");
    format!(
        "{} node={} automation_id={} field={} before={:?} after={:?}",
        change.kind.as_str(),
        change.node_handle_id,
        label,
        field,
        before_value,
        after_value
    )
}

fn required_object<'a>(
    object: &'a BTreeMap<String, Value>,
    key: &str,
) -> Result<&'a BTreeMap<String, Value>, String> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("`{key}` must be an object"))
}

fn required_array<'a>(
    object: &'a BTreeMap<String, Value>,
    key: &str,
) -> Result<&'a [Value], String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("`{key}` must be an array"))
}

fn required_string(object: &BTreeMap<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("`{key}` must be a string"))
}

fn optional_string(object: &BTreeMap<String, Value>, key: &str) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("`{key}` must be a string or null")),
    }
}

fn required_bool(object: &BTreeMap<String, Value>, key: &str) -> Result<bool, String> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("`{key}` must be a boolean"))
}

fn required_u64(object: &BTreeMap<String, Value>, key: &str) -> Result<u64, String> {
    let Some(value) = object.get(key) else {
        return Err(format!("`{key}` is required"));
    };
    number_to_u64(value, key)
}

fn optional_u64(object: &BTreeMap<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => number_to_u64(value, key).map(Some),
    }
}

fn number_to_u64(value: &Value, field: &str) -> Result<u64, String> {
    let Value::Number(number) = value else {
        return Err(format!("`{field}` must be an unsigned integer"));
    };
    if !number.is_finite() || number.fract() != 0.0 || *number < 0.0 {
        return Err(format!("`{field}` must be an unsigned integer"));
    }
    number
        .to_string()
        .parse::<u64>()
        .map_err(|_| format!("`{field}` does not fit in u64"))
}

fn write_line(output: &mut dyn std::io::Write, text: &str) -> Result<(), String> {
    writeln!(output, "{text}").map_err(|error| format!("write replay output failed: {error}"))
}
