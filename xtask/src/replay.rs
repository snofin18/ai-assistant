//! # 树快照回放（replay skeleton）
//!
//! `xtask replay` 子命令（gov §5.1 门禁 #12 子编号 = replay 骨架；TASK-015 实现 = 阶段性）。
//!
//! 职责：加载录制的 UI 树快照（JSON），做**离线回放** = 校验 + 摘要，不实际驱动应用。
//!
//! ## 边界（不做什么）
//! - 不**真**驱动应用：replay 是 dry-run，仅校验快照可解析、记录数 / 关键字段齐全。
//! - 不做 diff：快照 diff（vs. 当前实树快照）是另一个子命令（= future）。
//!
//! ## 快照格式（stage-0 约定）
//! ```json
//! {
//!   "version": 1,
//!   "captured_at": "2026-09-19T12:34:56Z",
//!   "application": "notepad.exe",
//!   "nodes": [
//!     {"id": 1, "name": "文件", "automation_id": "File", "rect": [0, 0, 100, 30]},
//!     ...
//!   ]
//! }
//! ```
//!
//! ## 不变量
//! 1. 输入合法 = `version: 1` + `nodes: non-empty array` + 每个 node 有 `id` 字段。
//! 2. 输出确定性：同一输入必得同一 Finding 列表。
//! 3. 退出码：0 (PASS) / 1 (snapshot 错误) / 4 (IO 错误)。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.4、ADR-0019 元门禁（硬门禁必须配负向验证）

use crate::report::{Finding, Severity};

/// Replay 规则名常量。
const RULE_SNAPSHOT_EMPTY: &str = "replay/snapshot-empty";
const RULE_SNAPSHOT_NODE_MISSING_ID: &str = "replay/snapshot-node-missing-id";
const RULE_SNAPSHOT_DUPLICATE_ID: &str = "replay/snapshot-duplicate-id";

/// 简版 Snapshot 数据结构（手工解析，不引三方依赖）。
/// 实际 stage-1 落地时 = serde + 更完整结构。
#[derive(Debug)]
pub struct Snapshot {
    pub version: u32,
    #[allow(dead_code)]
    pub captured_at: String,
    #[allow(dead_code)]
    pub application: String,
    pub nodes: Vec<SnapshotNode>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SnapshotNode {
    pub id: u64,
    pub name: String,
    pub automation_id: String,
    pub rect: [i64; 4],
}

/// 解析 JSON 文本到 Snapshot（手工解析，不引 serde）。
/// 简化版 = 只看顶层字段 + nodes 数组。不完整但 stage-0 够用。
pub fn parse_snapshot(content: &str) -> Result<Snapshot, String> {
    // 提取 version 字段
    let version =
        extract_u32(content, "\"version\"").ok_or_else(|| "快照缺少 `version` 字段".to_string())?;
    if version != 1 {
        return Err(format!("快照 version = {version}，当前仅支持 1"));
    }
    let captured_at = extract_string(content, "\"captured_at\"").unwrap_or_default();
    let application = extract_string(content, "\"application\"").unwrap_or_default();
    // 提取 nodes 数组
    let nodes_str =
        extract_nodes_array(content).ok_or_else(|| "快照缺少 `nodes` 数组".to_string())?;
    let mut nodes = Vec::new();
    for node_str in split_objects(nodes_str) {
        let id = extract_u64(node_str, "\"id\"").unwrap_or(0);
        let name = extract_string(node_str, "\"name\"").unwrap_or_default();
        let automation_id = extract_string(node_str, "\"automation_id\"").unwrap_or_default();
        let rect = extract_rect(node_str).unwrap_or([0, 0, 0, 0]);
        nodes.push(SnapshotNode {
            id,
            name,
            automation_id,
            rect,
        });
    }
    Ok(Snapshot {
        version,
        captured_at,
        application,
        nodes,
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

fn extract_nodes_array(content: &str) -> Option<&str> {
    let idx = content.find("\"nodes\"")?;
    let after = &content[idx + "\"nodes\"".len()..];
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

fn extract_rect(node_str: &str) -> Option<[i64; 4]> {
    let idx = node_str.find("\"rect\"")?;
    let after = &node_str[idx + "\"rect\"".len()..];
    let after = after.trim_start_matches(':').trim_start();
    let start = after.find('[')?;
    let end = after.find(']')?;
    let inner = &after[start + 1..end];
    let parts: Vec<i64> = inner
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if parts.len() == 4 {
        match parts.as_slice() {
            [a, b, c, d, ..] => Some([*a, *b, *c, *d]),
            _ => None,
        }
    } else {
        None
    }
}

/// 校验 Snapshot 的不变量，返回 findings。
pub fn validate_snapshot(snap: &Snapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    if snap.nodes.is_empty() {
        findings.push(Finding::new(
            RULE_SNAPSHOT_EMPTY,
            Severity::Error,
            "<snapshot>",
            0,
            "快照 nodes 为空（无法做回放）".to_string(),
        ));
    }
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
    findings
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
        "version": 1,
        "captured_at": "2026-09-19T12:34:56Z",
        "application": "notepad.exe",
        "nodes": [
            {"id": 1, "name": "文件", "automation_id": "File", "rect": [0, 0, 100, 30]},
            {"id": 2, "name": "编辑", "automation_id": "Edit", "rect": [0, 0, 100, 30]},
            {"id": 3, "name": "Format", "automation_id": "MenuBar", "rect": [0, 0, 800, 20]}
        ]
    }"#;

    #[test]
    fn parse_valid_snapshot() {
        let snap = parse_snapshot(VALID_SNAPSHOT).expect("应解析");
        assert_eq!(snap.version, 1);
        assert_eq!(snap.application, "notepad.exe");
        assert_eq!(snap.nodes.len(), 3);
        assert_eq!(snap.nodes[0].id, 1);
        assert_eq!(snap.nodes[0].name, "文件");
    }

    #[test]
    fn parse_invalid_version() {
        let s = r#"{"version": 2, "nodes": [{"id": 1}]}"#;
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
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, RULE_SNAPSHOT_EMPTY);
    }

    #[test]
    fn validate_duplicate_id_errors() {
        let s = r#"{
            "version": 1,
            "nodes": [
                {"id": 1, "name": "a"},
                {"id": 1, "name": "b"}
            ]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, RULE_SNAPSHOT_DUPLICATE_ID);
    }

    #[test]
    fn validate_missing_id_errors() {
        let s = r#"{
            "version": 1,
            "nodes": [
                {"id": 0, "name": "no_id"}
            ]
        }"#;
        let snap = parse_snapshot(s).unwrap();
        let f = validate_snapshot(&snap);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, RULE_SNAPSHOT_NODE_MISSING_ID);
    }
}
