//! # 树快照回放（replay skeleton）
//!
//! `xtask replay` 子命令（gov §5.1 门禁 #12 子编号 = replay；TASK-034 扩展为 Recording v1）。
//!
//! 职责：加载录制的 UI 树快照（JSON），做**离线回放** = 校验 + 摘要，不实际驱动应用。
//!
//! ## 边界（不做什么）
//! - 不**真**驱动应用：replay 是 dry-run，仅校验快照可解析、记录数 / 关键字段齐全。
//! - 不做 diff：快照 diff（vs. 当前实树快照）是另一个子命令（= future）。
//!
//! ## 快照格式（Recording v1；兼容 TASK-015 的 legacy `version` 形状）
//! ```json
//! {
//!   "schema_version": 1,
//!   "recording_id": "rec-notepad-like-basic-001",
//!   "captured_at": "2026-09-19T12:34:56Z",
//!   "application": "notepad-like",
//!   "window": {
//!     "local_handle_id": 1,
//!     "display_label": "notepad-like:main",
//!     "title": "notepad-like",
//!     "fingerprint": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
//!     "minimized": false,
//!     "foreground": true,
//!     "occluded": false
//!   },
//!   "nodes": [
//!     {"local_handle_id": 100, "parent_handle_id": null, "automation_id": "MainWindow"},
//!     ...
//!   ],
//!   "read_text": [{"element_handle_id": 102, "text": "notepad-like fixture"}]
//! }
//! ```
//!
//! ## 不变量
//! 1. 输入合法 = `schema_version: 1`（或 legacy `version: 1`） + 非空 nodes + 唯一非零 handle。
//! 2. 输出确定性：同一输入必得同一 Finding 列表。
//! 3. 退出码：0 (PASS) / 1 (snapshot 错误) / 4 (IO 错误)。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.4、ADR-0019 元门禁（硬门禁必须配负向验证）

use crate::report::{Finding, Severity};

/// Replay 规则名常量。
const RULE_SNAPSHOT_EMPTY: &str = "replay/snapshot-empty";
const RULE_SNAPSHOT_NODE_MISSING_ID: &str = "replay/snapshot-node-missing-id";
const RULE_SNAPSHOT_DUPLICATE_ID: &str = "replay/snapshot-duplicate-id";
const RULE_SNAPSHOT_ORPHAN_PARENT: &str = "replay/snapshot-orphan-parent";
const RULE_SNAPSHOT_PARENT_CYCLE: &str = "replay/snapshot-parent-cycle";
const RULE_SNAPSHOT_DANGLING_TEXT: &str = "replay/snapshot-dangling-text-reference";
const RULE_SNAPSHOT_DUPLICATE_TEXT: &str = "replay/snapshot-duplicate-text-reference";
const RULE_SNAPSHOT_MISSING_APPLICATION: &str = "replay/snapshot-missing-application";
const RULE_SNAPSHOT_MISSING_CAPTURED_AT: &str = "replay/snapshot-missing-captured-at";

/// 简版 Snapshot 数据结构（手工解析，不引三方依赖）。
#[derive(Debug)]
pub struct Snapshot {
    pub version: u32,
    pub recording_id: Option<String>,
    pub captured_at: String,
    pub application: String,
    pub nodes: Vec<SnapshotNode>,
    pub read_text_handles: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct SnapshotNode {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub automation_id: Option<String>,
}

/// 解析 JSON 文本到 Snapshot（手工解析，不引 serde）。
/// 简化版 = 只看 replay 校验所需字段；完整 schema 校验由 `assistant-replay` 负责。
pub fn parse_snapshot(content: &str) -> Result<Snapshot, String> {
    let version = extract_u32(content, "\"schema_version\"")
        .or_else(|| extract_u32(content, "\"version\""))
        .ok_or_else(|| "快照缺少 `schema_version` 或 legacy `version` 字段".to_string())?;
    if version != 1 {
        return Err(format!("快照 version = {version}，当前仅支持 1"));
    }
    let recording_id = extract_string(content, "\"recording_id\"");
    let captured_at = extract_string(content, "\"captured_at\"").unwrap_or_default();
    let application = extract_string(content, "\"application\"").unwrap_or_default();
    let nodes_str =
        extract_array(content, "nodes").ok_or_else(|| "快照缺少 `nodes` 数组".to_string())?;
    let mut nodes = Vec::new();
    for node_str in split_objects(nodes_str) {
        let id = extract_u64(node_str, "\"local_handle_id\"")
            .or_else(|| extract_u64(node_str, "\"id\""))
            .unwrap_or(0);
        let parent_id = extract_u64(node_str, "\"parent_handle_id\"");
        let automation_id = extract_string(node_str, "\"automation_id\"");
        nodes.push(SnapshotNode {
            id,
            parent_id,
            automation_id,
        });
    }
    let read_text_handles = extract_array(content, "read_text")
        .map(|read_text| {
            split_objects(read_text)
                .into_iter()
                .filter_map(|entry| extract_u64(entry, "\"element_handle_id\""))
                .collect()
        })
        .unwrap_or_default();
    Ok(Snapshot {
        version,
        recording_id,
        captured_at,
        application,
        nodes,
        read_text_handles,
    })
}

fn extract_u32(s: &str, key: &str) -> Option<u32> {
    let idx = s.find(key)?;
    let after = &s[idx + key.len()..];
    let after = after.trim_start_matches(':').trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

fn extract_u64(s: &str, key: &str) -> Option<u64> {
    let idx = s.find(key)?;
    let after = &s[idx + key.len()..];
    let after = after.trim_start_matches(':').trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

fn extract_string(s: &str, key: &str) -> Option<String> {
    let idx = s.find(key)?;
    let after = &s[idx + key.len()..];
    let after = after.trim_start_matches(':').trim_start();
    let after = after.trim_start_matches('"');
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn extract_array<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let quoted_key = format!("\"{key}\"");
    let idx = content.find(&quoted_key)?;
    let after = &content[idx + quoted_key.len()..];
    let after = after.trim_start_matches(':').trim_start();
    let start = after.find('[')?;
    let end = find_matching_bracket(after, start)?;
    Some(&after[start + 1..end])
}

fn find_matching_bracket(s: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s[open_idx..].char_indices() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_objects(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = None;
    for (i, c) in s.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0
                    && let Some(s_idx) = start
                {
                    out.push(&s[s_idx..=i]);
                }
            }
            _ => {}
        }
    }
    out
}

/// 校验 Snapshot 的不变量，返回 findings。
pub fn validate_snapshot(snap: &Snapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    validate_snapshot_metadata(snap, &mut findings);
    if snap.nodes.is_empty() {
        findings.push(Finding::new(
            RULE_SNAPSHOT_EMPTY,
            Severity::Error,
            "<snapshot>",
            0,
            "快照 nodes 为空（无法做回放）".to_string(),
        ));
    }
    let seen_ids = validate_node_handles(snap, &mut findings);
    validate_parent_references(snap, &seen_ids, &mut findings);
    validate_parent_cycles(snap, &mut findings);
    validate_read_text_references(snap, &seen_ids, &mut findings);
    findings
}

fn validate_snapshot_metadata(snap: &Snapshot, findings: &mut Vec<Finding>) {
    if snap.application.trim().is_empty() {
        findings.push(Finding::new(
            RULE_SNAPSHOT_MISSING_APPLICATION,
            Severity::Error,
            "<snapshot>",
            0,
            "快照缺少 application".to_string(),
        ));
    }
    if snap.captured_at.trim().is_empty() {
        findings.push(Finding::new(
            RULE_SNAPSHOT_MISSING_CAPTURED_AT,
            Severity::Error,
            "<snapshot>",
            0,
            "快照缺少 captured_at".to_string(),
        ));
    }
}

fn validate_node_handles(
    snap: &Snapshot,
    findings: &mut Vec<Finding>,
) -> std::collections::HashSet<u64> {
    let mut seen_ids = std::collections::HashSet::new();
    for (i, node) in snap.nodes.iter().enumerate() {
        if node.id == 0 {
            findings.push(Finding::new(
                RULE_SNAPSHOT_NODE_MISSING_ID,
                Severity::Error,
                "<snapshot>",
                i,
                "节点缺少 `id` 字段或 id = 0".to_string(),
            ));
        }
        if !seen_ids.insert(node.id) {
            findings.push(Finding::new(
                RULE_SNAPSHOT_DUPLICATE_ID,
                Severity::Error,
                "<snapshot>",
                i,
                format!("节点 id = {} 重复", node.id),
            ));
        }
    }
    seen_ids
}

fn validate_parent_references(
    snap: &Snapshot,
    seen_ids: &std::collections::HashSet<u64>,
    findings: &mut Vec<Finding>,
) {
    for (i, node) in snap.nodes.iter().enumerate() {
        if let Some(parent) = node.parent_id
            && !seen_ids.contains(&parent)
        {
            findings.push(Finding::new(
                RULE_SNAPSHOT_ORPHAN_PARENT,
                Severity::Error,
                "<snapshot>",
                i,
                format!("节点 id = {} 引用不存在的父节点 {}", node.id, parent),
            ));
        }
    }
}

fn validate_parent_cycles(snap: &Snapshot, findings: &mut Vec<Finding>) {
    for (i, node) in snap.nodes.iter().enumerate() {
        let mut visited = std::collections::HashSet::new();
        let mut current = Some(node.id);
        while let Some(handle) = current {
            if !visited.insert(handle) {
                findings.push(Finding::new(
                    RULE_SNAPSHOT_PARENT_CYCLE,
                    Severity::Error,
                    "<snapshot>",
                    i,
                    format!("节点 id = {} 的父链存在环", node.id),
                ));
                break;
            }
            current = snap
                .nodes
                .iter()
                .find(|candidate| candidate.id == handle)
                .and_then(|candidate| candidate.parent_id);
        }
    }
}

fn validate_read_text_references(
    snap: &Snapshot,
    seen_ids: &std::collections::HashSet<u64>,
    findings: &mut Vec<Finding>,
) {
    let mut seen_text_handles = std::collections::HashSet::new();
    for (i, handle) in snap.read_text_handles.iter().enumerate() {
        if !seen_ids.contains(handle) {
            findings.push(Finding::new(
                RULE_SNAPSHOT_DANGLING_TEXT,
                Severity::Error,
                "<snapshot>",
                i,
                format!("read_text 引用不存在的节点 handle = {handle}"),
            ));
        } else if !seen_text_handles.insert(*handle) {
            findings.push(Finding::new(
                RULE_SNAPSHOT_DUPLICATE_TEXT,
                Severity::Error,
                "<snapshot>",
                i,
                format!("read_text 对节点 handle = {handle} 重复录制"),
            ));
        }
    }
}

/// 执行 replay 子命令：加载 + 校验 + 摘要（dry-run）。
pub fn run(snapshot_path: &std::path::Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::{EXIT_FINDINGS, EXIT_OK};
    let content = std::fs::read_to_string(snapshot_path).map_err(|e| format!("读快照失败：{e}"))?;
    let snap = match parse_snapshot(&content) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(output, "== replay ==");
            let _ = writeln!(output, "parse error: {e}");
            let _ = writeln!(output, "-- verdict: FAILED");
            return Ok(EXIT_FINDINGS);
        }
    };
    let findings = validate_snapshot(&snap);
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let _ = writeln!(output, "== replay ==");
    let _ = writeln!(output, "snapshot: {}", snapshot_path.display());
    let _ = writeln!(output, "version: {}", snap.version);
    let _ = writeln!(
        output,
        "recording_id: {}",
        snap.recording_id.as_deref().unwrap_or("(legacy)")
    );
    let _ = writeln!(
        output,
        "application: {}",
        if snap.application.is_empty() {
            "(未指定)".to_string()
        } else {
            snap.application.clone()
        }
    );
    let _ = writeln!(
        output,
        "captured_at: {}",
        if snap.captured_at.is_empty() {
            "(未指定)".to_string()
        } else {
            snap.captured_at.clone()
        }
    );
    let _ = writeln!(output, "nodes: {}", snap.nodes.len());
    for f in &findings {
        let _ = writeln!(output, "{} {}:{} {}", f.rule, f.path, f.line, f.message);
    }
    let verdict = if errors > 0 { "FAILED" } else { "PASSED" };
    let stable_automation_ids = snap
        .nodes
        .iter()
        .filter(|node| {
            node.automation_id
                .as_deref()
                .is_some_and(|automation_id| !automation_id.trim().is_empty())
        })
        .count();
    let _ = writeln!(output, "stable_automation_ids: {stable_automation_ids}");
    let _ = writeln!(
        output,
        "-- summary: {errors} error(s), {warnings} warning(s)"
    );
    let _ = writeln!(output, "-- verdict: {verdict}");
    if errors > 0 {
        Ok(EXIT_FINDINGS)
    } else {
        Ok(EXIT_OK)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;

    const VALID_SNAPSHOT: &str = r#"{
        "schema_version": 1,
        "recording_id": "rec-notepad-like-basic-001",
        "captured_at": "2026-09-19T12:34:56Z",
        "application": "notepad-like",
        "nodes": [
            {"local_handle_id": 100, "parent_handle_id": null, "automation_id": "MainWindow"},
            {"local_handle_id": 101, "parent_handle_id": 100, "automation_id": "EditorHost"},
            {"local_handle_id": 102, "parent_handle_id": 101, "automation_id": "EditorTextBox"}
        ],
        "read_text": [{"element_handle_id": 102, "text": "notepad-like fixture"}]
    }"#;

    #[test]
    fn parse_valid_snapshot() {
        let snap = parse_snapshot(VALID_SNAPSHOT).expect("应解析");
        assert_eq!(snap.version, 1);
        assert_eq!(snap.application, "notepad-like");
        assert_eq!(
            snap.recording_id.as_deref(),
            Some("rec-notepad-like-basic-001")
        );
        assert_eq!(snap.nodes.len(), 3);
        assert_eq!(snap.nodes[0].id, 100);
        assert_eq!(snap.nodes[1].parent_id, Some(100));
    }

    #[test]
    fn parse_invalid_version() {
        let s = r#"{"schema_version": 2, "nodes": [{"local_handle_id": 1}]}"#;
        let err = parse_snapshot(s).unwrap_err();
        assert!(err.contains("version"), "got: {err}");
    }

    #[test]
    fn parse_missing_version() {
        let s = r#"{"nodes": []}"#;
        assert!(parse_snapshot(s).is_err());
    }

    #[test]
    fn validate_clean_snapshot_passes() {
        let snap = parse_snapshot(VALID_SNAPSHOT).unwrap();
        let f = validate_snapshot(&snap);
        assert!(f.is_empty(), "got: {f:?}");
    }

    #[test]
    fn validate_empty_nodes_errors() {
        let s = r#"{"version": 1, "nodes": []}"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(f.iter().any(|finding| finding.rule == RULE_SNAPSHOT_EMPTY));
    }

    #[test]
    fn validate_duplicate_id_errors() {
        let s = r#"{
            "schema_version": 1,
            "nodes": [
                {"local_handle_id": 1},
                {"local_handle_id": 1}
            ]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(
            f.iter()
                .any(|finding| finding.rule == RULE_SNAPSHOT_DUPLICATE_ID)
        );
    }

    #[test]
    fn validate_missing_id_errors() {
        let s = r#"{
            "schema_version": 1,
            "nodes": [
                {"local_handle_id": 0}
            ]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(
            f.iter()
                .any(|finding| finding.rule == RULE_SNAPSHOT_NODE_MISSING_ID)
        );
    }

    #[test]
    fn parse_legacy_snapshot_shape() {
        let s = r#"{
            "version": 1,
            "captured_at": "2026-09-19T12:34:56Z",
            "application": "notepad.exe",
            "nodes": [{"id": 1, "name": "File", "automation_id": "File"}]
        }"#;
        let snap = parse_snapshot(s).expect("legacy shape remains readable");
        assert_eq!(snap.version, 1);
        assert_eq!(snap.nodes[0].id, 1);
        assert_eq!(snap.nodes[0].automation_id.as_deref(), Some("File"));
    }

    #[test]
    fn validate_orphan_parent_errors() {
        let s = r#"{
            "schema_version": 1,
            "nodes": [{"local_handle_id": 1, "parent_handle_id": 999}]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(
            f.iter()
                .any(|finding| finding.rule == RULE_SNAPSHOT_ORPHAN_PARENT)
        );
    }

    #[test]
    fn validate_parent_cycle_errors() {
        let s = r#"{
            "schema_version": 1,
            "nodes": [
                {"local_handle_id": 1, "parent_handle_id": 2},
                {"local_handle_id": 2, "parent_handle_id": 1}
            ]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(
            f.iter()
                .any(|finding| finding.rule == RULE_SNAPSHOT_PARENT_CYCLE)
        );
    }

    #[test]
    fn validate_dangling_text_reference_errors() {
        let s = r#"{
            "schema_version": 1,
            "nodes": [{"local_handle_id": 1}],
            "read_text": [{"element_handle_id": 999, "text": "x"}]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(
            f.iter()
                .any(|finding| finding.rule == RULE_SNAPSHOT_DANGLING_TEXT)
        );
    }

    #[test]
    fn validate_duplicate_text_reference_errors() {
        let s = r#"{
            "schema_version": 1,
            "nodes": [{"local_handle_id": 1}],
            "read_text": [
                {"element_handle_id": 1, "text": "x"},
                {"element_handle_id": 1, "text": "y"}
            ]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert!(
            f.iter()
                .any(|finding| finding.rule == RULE_SNAPSHOT_DUPLICATE_TEXT)
        );
    }
}
