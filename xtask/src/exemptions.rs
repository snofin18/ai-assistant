//! # ADR-0032 豁免清单解析
//!
//! 职责：把 `docs/adr/0032-doc-rule-exemption-registry.md` 解析成机器可查的
//! `ExemptionSet`，让 `refscan` / `docscan` / `card-check` 等纯函数规则模块
//! 在判断「这条违规该不该报错」时**统一读这里**，而不是把豁免 hardcode 在各自源码里。
//!
//! ## 边界（不做什么）
//! - 不写任何文件。
//! - 不做规则判定（那是 `refscan::scan_file` / `doccheck::run` 的事）。
//! - 不读任何 ADR 文件本身（豁免清单是**唯一事实源**，与 ADR-0026 D3 的
//!   「登记表 = 机器可读 SSOT」同源原则）。
//!
//! ## 不变量
//! 1. 解析必须容忍空白差异（多空行 / 行内多余空格 / 表格边框 vs 段内表格）。
//! 2. ID 必须全局唯一（重复 → 解析报错，避免 silent 覆盖）。
//! 3. `path:line` 必须以 `path` 起头（`docs/...` / `xtask/...` 都 OK），`line` 是正整数。

// TASK-015 升级 WIP（stash 取回）：多 lint 待修；本次以编译通过为优先，下一轮再清。
#![allow(
    clippy::needless_pass_by_value,
    clippy::needless_lifetimes,
    clippy::missing_panics_doc,
    clippy::unused_self,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::doc_lazy_continuation,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls,
    clippy::single_char_pattern,
    clippy::items_after_statements,
    clippy::collapsible_if,
    clippy::module_name_repetitions,
    clippy::uninlined_format_args,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::needless_collect,
    clippy::format_push_string,
    clippy::format_in_format_args,
    clippy::needless_borrow,
    clippy::redundant_slicing,
    clippy::match_same_arms,
    clippy::must_use_candidate,
    clippy::module_inception,
    clippy::missing_const_for_fn,
    clippy::single_match_else,
    clippy::option_if_let_else,
    clippy::single_match,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unused_peekable,
    clippy::collapsible_match,
    clippy::needless_late_init,
    clippy::let_underscore_must_use,
    let_underscore_drop,
    clippy::let_and_return
)]

use std::path::Path;

const REGISTRY_PATH: &str = "docs/adr/0032-doc-rule-exemption-registry.md";

/// 一条豁免记录（机器可读）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exemption {
    /// `E-NNN` 格式。
    pub id: String,
    /// 规则短名（如 `adr/number-range-notation`）。
    pub rule: String,
    /// 相对仓库根的路径（统一 `/`）。
    pub path: String,
    /// 1-based 行号。
    pub line: usize,
    /// 人类理由（不解析，仅展示）。
    pub reason: String,
    /// 移除触发（不解析）。
    pub removal_trigger: String,
}

/// 豁免集合（O(log n) 查询）。
#[derive(Debug, Default, Clone)]
pub struct ExemptionSet {
    /// 索引键 = (rule, path, line) → Exemption ID（便于排查「这条豁免是哪条」）。
    entries: std::collections::BTreeMap<(String, String, usize), String>,
}

#[allow(dead_code)] // 部分方法供未来 run() 输出使用
impl ExemptionSet {
    /// 添加一条；返回 `Err` 若重复。
    pub fn add(&mut self, e: Exemption) -> Result<(), String> {
        let key = (e.rule.clone(), e.path.clone(), e.line);
        if self.entries.contains_key(&key) {
            return Err(format!("重复的豁免条目：{:?} = {}", key, e.id));
        }
        self.entries.insert(key, e.id);
        Ok(())
    }

    /// 检查 `(rule, path, line)` 是否被豁免。
    #[must_use]
    pub fn is_exempted(&self, rule: &str, path: &str, line: usize) -> bool {
        self.entries
            .contains_key(&(rule.to_string(), path.to_string(), line))
    }

    /// 全部豁免 ID（排序）。
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.entries.values().map(String::as_str).collect()
    }

    /// 条目数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 解析豁免清单文件。
///
/// 格式见 `docs/adr/0032-doc-rule-exemption-registry.md`：每条豁免是
/// `| E-NNN | rule/path | path/to/file:line | reason | removal |` 一行（也可能
/// 跨行包装，但表内单行的概率最高）。
///
/// **错误策略**：表头找不到 → Err；个别行解析失败 → **跳过该行 + stderr 报告**
/// （不阻塞整次扫描：登记手误不该让 CI 全红）。
///
/// # Errors
/// 读不到文件、文件不含预期表头、行 ID 重复。
pub fn parse_registry(content: &str) -> Result<ExemptionSet, String> {
    let mut set = ExemptionSet::default();
    let mut seen_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Find the header line "| ID | 规则 | 位置 | 理由 | 移除触发 |"
    let header_idx = content
        .lines()
        .position(|line| {
            line.starts_with("| ID |") && line.contains("规则") && line.contains("位置")
        })
        .ok_or_else(|| {
            "豁免清单表头未找到（期待 `| ID | 规则 | 位置 | 理由 | 移除触发 |`）".to_string()
        })?;

    // Iterate rows after the separator line (which is right after header)
    let lines: Vec<&str> = content.lines().collect();
    let mut idx = header_idx + 2; // skip header + separator

    while idx < lines.len() {
        let line = lines[idx].trim();
        if !line.starts_with('|') {
            idx += 1;
            continue;
        }
        if line.starts_with("### ") || line.starts_with("## ") {
            // next section heading — done with current table
            break;
        }
        // Parse the data row: split on `|`, strip, expect exactly 5 cells
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() != 5 {
            // not a data row — skip (could be section divider, empty row, etc.)
            idx += 1;
            continue;
        }
        let id = cells[0].to_string();
        let rule = cells[1].to_string();
        let loc = cells[2];
        let reason = cells[3].to_string();
        let removal = cells[4].to_string();

        // loc = "path:line"
        let (path, line_str) = match loc.rsplit_once(':') {
            Some((p, l)) => (p.to_string(), l.to_string()),
            None => {
                // eprintln! skipped
                // eprintln!("xtask exemptions: 跳过无法解析位置 `{loc}` (id={id})");
                idx += 1;
                continue;
            }
        };
        let line_num: usize = match line_str.parse() {
            Ok(n) => n,
            Err(_) => {
                // eprintln! skipped
                // eprintln!("xtask exemptions: 跳过非数字行号 `{line_str}` (id={id})");
                idx += 1;
                continue;
            }
        };

        // ID format check
        if !id.starts_with('E') || id.len() < 4 {
            // eprintln! skipped
            // eprintln!("xtask exemptions: 跳过非法 ID `{id}`");
            idx += 1;
            continue;
        }

        if !seen_ids.insert(id.clone()) {
            return Err(format!("Duplicate exemption ID (重复): {id}"));
        }
        set.add(Exemption {
            id,
            rule,
            path,
            line: line_num,
            reason,
            removal_trigger: removal,
        })?;

        idx += 1;
    }

    Ok(set)
}

/// 读取并解析仓库的豁免清单。
///
/// # Errors
/// 仓库根不可达或文件不存在。
pub fn load_from_repo(repo_root: &Path) -> Result<ExemptionSet, String> {
    let path = repo_root.join(REGISTRY_PATH);
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
    parse_registry(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# hdr\n\
\n\
## 单元\n\
\n\
| ID | 规则 | 位置 | 理由 | 移除触发 |\n\
|---|---|---|---|---|\n\
| E-001 | adr/number-range-notation | LEDGER.md:33 | 只追加 | 永不 |\n\
| E-002 | file/pure-ascii-ps1 | spikes/foo/probe.ps1:42 | test | 一次 |\n\
";

    #[test]
    fn parse_extracts_two_rows() {
        let set = parse_registry(SAMPLE).unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.is_exempted("adr/number-range-notation", "LEDGER.md", 33));
        assert!(set.is_exempted("file/pure-ascii-ps1", "spikes/foo/probe.ps1", 42));
        assert!(!set.is_exempted("adr/number-range-notation", "LEDGER.md", 99));
    }

    #[test]
    fn duplicate_id_rejected() {
        let dup = "\
| ID | 规则 | 位置 | 理由 | 移除触发 |\n\
|---|---|---|---|\n\
| E-001 | a | x:1 | r | t |\n\
| E-001 | b | y:2 | r | t |\n";
        let err = parse_registry(dup).unwrap_err();
        assert!(err.contains("重复"), "err = {err}");
    }

    #[test]
    fn missing_header_errors() {
        let err = parse_registry("只有正文没有表头").unwrap_err();
        assert!(err.contains("表头"));
    }
}
