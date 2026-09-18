//! `adr_registry.rs` 的单元测试：ADR 登记表 / ADR 文件状态行 / 待建号解析（ADR-0030 D3）的白盒测试。
//!
//! **为什么单独一个文件**：`adr_registry.rs` 连同测试会超过单文件行数阈值
//! （AGENTS.md §5.3 的 600 行硬上限 / gov §5.4 的 > 600 警告）。用 `#[path]` 把 `mod tests`
//! 外置后两侧都回到阈值内，而测试**仍然是本模块的私有单元测试** —— `use super::*` 照旧能
//! 访问私有项，这一点与 `tests/` 目录下的集成测试有本质区别，不能混为一谈。
//! 声明处在 `adr_registry.rs` 末尾：`#[cfg(test)] #[path = "adr_registry_tests.rs"] mod tests;`。
//!
//! ## 组织方式
//! 按被测函数分组，每组前有一行 `// --- 函数名 ---` 分隔注释；命名遵循
//! `test_<被测单元>_<条件>_<期望>`（AGENTS.md §5.1）。

// 测试里允许 unwrap/expect/panic：断言失败就该立刻炸出来，包装成 Result 只会掩盖问题
// （AGENTS.md §5.5「tests/ 内可 allow」）。
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

/// 一份结构完整的登记表样本（含 §1/§2/§3、退役清单、下一个可用编号）。
const SAMPLE_REGISTRY: &str = "\
# docs/adr/ — ADR 编号登记表

## 1. 已存在的 ADR 文件（编号已被占用，不得复用）

| 编号 | 文件 | 状态 | 主题 |
|---|---|---|---|
| 0018 | `0018-nightly.md` | Accepted → **Superseded**（by ADR-0029） | 夜间自动化 |
| 0029 | `0029-nightly-v2.md` | Accepted | 夜间自动化改回 Codex |

**下一个可用编号：0031**

## 2. 已决定、但尚未写成 ADR 文件的编号（`[ADR:待建 NNNN]`）

| 编号 | 决定 |
|---|---|
| 0016 | 全仓库统一 LF |
| 0027 | `#[allow]` 的唯一合法位置 |

## 3. 已知编号事故

已退役编号：0019（原「#[allow] 位置」条目，已由 0027 取代）

## 4. 引用规范
";

// --- file_number ---

#[test]
fn test_file_number_extracts_four_digit_prefix() {
    assert_eq!(file_number("0018-nightly.md"), Some(18));
    assert_eq!(file_number("0030-x.md"), Some(30));
}

#[test]
fn test_file_number_rejects_non_adr_files() {
    for name in [
        "README.md",
        ".gitkeep",
        "18-nightly.md",
        "0018x-nightly.md",
        "adr-0018.md",
    ] {
        assert_eq!(file_number(name), None, "不该被当成 ADR 文件：{name}");
    }
}

// --- parse_status_line / summarize_adr_file ---

#[test]
fn test_parse_status_line_takes_earliest_keyword_not_superseded_field() {
    let content = "# ADR-0018　标题\n\n状态：**Accepted**（2026-09-18）　Supersedes：—　Superseded by：ADR-0029\n\n正文\n";
    let (status, line, superseded_by) = parse_status_line(content);
    assert_eq!(
        status.as_deref(),
        Some("Accepted"),
        "不能被后面的 Superseded by 顶掉"
    );
    assert_eq!(line, 3);
    assert_eq!(superseded_by, Some(29));
}

#[test]
fn test_parse_status_line_dash_means_not_superseded() {
    let content = "状态：**Proposed**（待确认）　Supersedes：—　Superseded by：—\n";
    let (status, _, superseded_by) = parse_status_line(content);
    assert_eq!(status.as_deref(), Some("Proposed"));
    assert_eq!(superseded_by, None, "写 — 就是没有被取代");
}

#[test]
fn test_parse_status_line_missing_returns_none_and_zero() {
    let (status, line, superseded_by) = parse_status_line("# ADR-0099\n\n没有状态行\n");
    assert_eq!(status, None);
    assert_eq!(line, 0);
    assert_eq!(superseded_by, None);
}

#[test]
fn test_summarize_adr_file_returns_none_for_non_adr_name() {
    assert_eq!(
        summarize_adr_file("README.md", "状态：**Accepted**\n"),
        None
    );
}

// --- parse_registry ---

#[test]
fn test_retired_numbers_ignores_replacement_number_in_parenthesis() {
    // 负向：括号里是说明文字，"取代者 0027" 不是退役号
    assert_eq!(
        retired_numbers("已退役编号：0019（原「#[allow] 位置」条目，已由 0027 取代）"),
        vec![19]
    );
    // 半角括号同样要截断
    assert_eq!(
        retired_numbers("已退役编号：0019 (replaced by 0027)"),
        vec![19]
    );
    // 多个退役号：无括号时用顿号/逗号分隔都能取全
    assert_eq!(retired_numbers("已退役编号：0019、0023"), vec![19, 23]);
    // "（无）" 必须解析成空表，而不是把别的数字抓进来
    assert_eq!(retired_numbers("已退役编号：（无）"), Vec::<u32>::new());
}

#[test]
fn test_parse_registry_reads_all_four_anchors() {
    let registry = parse_registry(SAMPLE_REGISTRY);
    assert!(
        registry.missing_anchors.is_empty(),
        "不该缺锚点：{:?}",
        registry.missing_anchors
    );
    let existing: Vec<u32> = registry.existing.iter().map(|row| row.number).collect();
    assert_eq!(existing, vec![18, 29]);
    assert_eq!(registry.pending, vec![16, 27]);
    assert_eq!(registry.retired, vec![19]);
    assert_eq!(registry.next_available, Some(31));
}

#[test]
fn test_parse_registry_keeps_status_cell_verbatim() {
    let registry = parse_registry(SAMPLE_REGISTRY);
    let first = registry.existing.first().expect("应有一行");
    assert!(
        first.status_cell.contains("Superseded"),
        "状态列原文要留着，供 superseded-not-marked 规则判断：{}",
        first.status_cell
    );
}

#[test]
fn test_parse_registry_records_missing_anchors_instead_of_passing_silently() {
    let registry = parse_registry("# 空登记表\n\n什么都没有\n");
    assert!(
        registry.missing_anchors.len() >= 5,
        "四个章节 + 两个单行标记都必须被记为缺失，实际：{:?}",
        registry.missing_anchors
    );
    assert!(registry.existing.is_empty());
    assert_eq!(registry.next_available, None);
}

#[test]
fn test_parse_registry_retired_none_marker_yields_empty_list() {
    let content = SAMPLE_REGISTRY.replace(
        "已退役编号：0019（原「#[allow] 位置」条目，已由 0027 取代）",
        "已退役编号：（无）",
    );
    let registry = parse_registry(&content);
    assert!(registry.retired.is_empty());
    assert!(
        !registry
            .missing_anchors
            .iter()
            .any(|anchor| anchor.contains(RETIRED_MARKER)),
        "写了「（无）」就不算缺锚点"
    );
}

#[test]
fn test_parse_registry_ignores_non_number_first_column() {
    // 表头行与分隔行的第 1 列不是 4 位数字，必须被跳过而不是报成编号
    let registry = parse_registry(SAMPLE_REGISTRY);
    assert!(
        !registry.existing.iter().any(|row| row.number == 0),
        "表头/分隔行不该混进来"
    );
}

// --- all_adr_numbers / collect_pending_numbers ---

#[test]
fn test_all_adr_numbers_requires_clean_four_digit_boundary() {
    assert_eq!(all_adr_numbers("见 ADR-0019 与 0027"), vec![19, 27]);
    assert_eq!(
        all_adr_numbers("编号 00190 不是 ADR 号"),
        vec![],
        "5 位数字里不许截前 4 位"
    );
    assert_eq!(all_adr_numbers("已退役编号：（无）"), vec![]);
}

#[test]
fn test_collect_pending_numbers_finds_markers_and_dedups() {
    let content = "\
- [2026-09-16][DECISION][ADR:待建 0016] 全仓库统一 LF
- [2026-09-16][DECISION][ADR:待建 0018 补充] 三级回退
- [2026-09-18][DECISION][ADR:待建 0027][supersedes: 2026-09-16 的 [ADR:待建 0019] 条目] 改号
";
    assert_eq!(collect_pending_numbers(content), vec![16, 18, 19, 27]);
}

#[test]
fn test_collect_pending_numbers_ignores_bare_adr_references() {
    // 裸写 `ADR-0016`（不带「待建」）不算预留条目 —— 那意味着文件已存在
    assert_eq!(collect_pending_numbers("见 ADR-0016 与 ADR-0023\n"), vec![]);
}

// --- 4 位编号扫描的顺序语义（2026-09-18 的实测回归）---

#[test]
fn test_first_adr_number_returns_text_order_not_smallest() {
    // 回归：登记表写「下一个可用编号：0031（= 已用最大号 0030 + 1）」时必须读出 0031。
    // 若复用 all_adr_numbers（升序去重）会读到 0030 —— 2026-09-18 实测踩过。
    assert_eq!(
        first_adr_number("**下一个可用编号：0031**（= 0030 + 1）"),
        Some(31)
    );
}

#[test]
fn test_first_adr_number_reads_superseded_by_target_not_smaller_number() {
    // 取代者是 0029，不是同一行里更小的 0018
    assert_eq!(
        first_adr_number("Superseded by：ADR-0029（另见 ADR-0018 的纪律性内容）"),
        Some(29)
    );
}

#[test]
fn test_four_digit_numbers_in_text_order_keeps_duplicates_and_order() {
    assert_eq!(
        four_digit_numbers_in_text_order("0031 与 0030，再说一次 0031"),
        vec![31, 30, 31]
    );
}

#[test]
fn test_all_adr_numbers_is_sorted_and_deduplicated() {
    // 集合语义：与顺序语义互补，退役清单依赖它
    assert_eq!(all_adr_numbers("0031 与 0030，再说一次 0031"), vec![30, 31]);
}
