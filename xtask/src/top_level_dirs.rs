//! # 顶层目录 ADR 白名单规则（gov §5.4 / ADR-0069）
//!
//! 职责：解析 `docs/adr/top-level-directories.md` 的目录表，并比对仓库根的一级目录。
//! 未登记目录是 Error；白名单存在但磁盘缺失的条目是 Warning。
//!
//! ## 边界（不做什么）
//! - 不做文件系统遍历：实际目录由 `repowalk` 收集，文件由 `main.rs` 读取。
//! - 不自动扩表：规则只报错，维护者必须通过 ADR / Orchestrator 更新白名单。
//! - 不把“解析失败”当成空白名单：调用方必须产生 unparsable Error。
//!
//! ## 不变量
//! 1. 解析与判定是纯函数，输出顺序确定。
//! 2. 固定标题 `## 允许的顶层目录` 是表格入口；标题缺失或零行都是 `None`。
//! 3. 第一列目录名统一去掉一个尾部 `/` 后比较。
//!
//! 相关：`docs/adr/0069-top-level-directory-adr-whitelist.md`

use std::collections::BTreeSet;

use crate::report::{Finding, Severity};

/// 白名单文件相对仓库根的路径。
pub const TOP_LEVEL_WHITELIST_PATH: &str = "docs/adr/top-level-directories.md";

/// 固定表格标题。
pub const TOP_LEVEL_WHITELIST_HEADING: &str = "## 允许的顶层目录";

/// 解析白名单表格；标题缺失或没有任何合法目录行时返回 `None`。
#[must_use]
pub fn parse_top_level_whitelist(source: &str) -> Option<BTreeSet<String>> {
    let mut saw_heading = false;
    let mut directories = BTreeSet::new();
    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            if saw_heading {
                break;
            }
            saw_heading = heading == TOP_LEVEL_WHITELIST_HEADING.trim_start_matches("## ");
            continue;
        }
        if !saw_heading || !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some(directory_cell) = cells.first().copied() else {
            continue;
        };
        let directory = directory_cell
            .trim_matches('`')
            .trim()
            .trim_end_matches('/');
        if directory.is_empty()
            || directory == "目录"
            || directory
                .chars()
                .all(|character| character == '-' || character == ':')
            || directory.contains('/')
            || directory.contains('\\')
        {
            continue;
        }
        directories.insert(directory.to_string());
    }
    (saw_heading && !directories.is_empty()).then_some(directories)
}

/// 比对实际顶层目录与白名单。
#[must_use]
pub fn check_top_level_directories(
    actual_directories: &[String],
    whitelist: &BTreeSet<String>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for directory in actual_directories {
        let normalized = directory.trim_end_matches('/');
        if !whitelist.contains(normalized) {
            findings.push(Finding::new(
                "hygiene/unregistered-top-level-directory",
                Severity::Error,
                format!("{normalized}/"),
                0,
                format!(
                    "顶层目录未登记在 {TOP_LEVEL_WHITELIST_PATH}；新增目录前必须先有 Accepted ADR 并登记白名单（ADR-0069）"
                ),
            ));
        }
    }
    for directory in whitelist {
        let normalized = directory.trim_end_matches('/');
        if !actual_directories
            .iter()
            .any(|actual| actual.trim_end_matches('/') == normalized)
        {
            findings.push(Finding::new(
                "hygiene/stale-top-level-directory",
                Severity::Warning,
                TOP_LEVEL_WHITELIST_PATH,
                0,
                format!(
                    "白名单登记了 {normalized}/，但仓库根下不存在该目录；请清理（ADR-0069 D5）"
                ),
            ));
        }
    }
    findings
}

/// 白名单缺失或不可解析时的 Error。
#[must_use]
pub fn unparsable_whitelist_finding() -> Finding {
    Finding::new(
        "hygiene/top-level-whitelist-unparsable",
        Severity::Error,
        TOP_LEVEL_WHITELIST_PATH,
        0,
        format!(
            "缺少固定标题 `{TOP_LEVEL_WHITELIST_HEADING}` 或没有可解析目录行；规则不会静默跳过（ADR-0069 D6）"
        ),
    )
}

#[cfg(test)]
#[path = "top_level_dirs_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;
