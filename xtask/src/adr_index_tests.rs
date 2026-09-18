//! `adr_index.rs` 的单元测试：ADR 编号一致性 11 条规则（ADR-0030 D3）的白盒测试。
//!
//! **为什么单独一个文件**：`adr_index.rs` 连同测试会超过单文件行数阈值
//! （AGENTS.md §5.3 的 600 行硬上限 / gov §5.4 的 > 600 警告）。用 `#[path]` 把 `mod tests`
//! 外置后两侧都回到阈值内，而测试**仍然是本模块的私有单元测试** —— `use super::*` 照旧能
//! 访问私有项，这一点与 `tests/` 目录下的集成测试有本质区别，不能混为一谈。
//! 声明处在 `adr_index.rs` 末尾：`#[cfg(test)] #[path = "adr_index_tests.rs"] mod tests;`。
//!
//! ## 组织方式
//! 按被测函数分组，每组前有一行 `// --- 函数名 ---` 分隔注释；命名遵循
//! `test_<被测单元>_<条件>_<期望>`（AGENTS.md §5.1）。

// 测试里允许 unwrap/expect/panic：断言失败就该立刻炸出来，包装成 Result 只会掩盖问题
// （AGENTS.md §5.5「tests/ 内可 allow」）。
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

/// 一份**自洽**的登记表：§1 = {0029}，§2 = {0027}，退役 = {0019}，下一个 = 0030。
const CONSISTENT_REGISTRY: &str = "\
# docs/adr/ — ADR 编号登记表

## 1. 已存在的 ADR 文件

| 编号 | 文件 | 状态 | 主题 |
|---|---|---|---|
| 0029 | `0029-nightly.md` | Accepted | 夜间自动化改回 Codex |

**下一个可用编号：0030**

## 2. 已决定、但尚未写成 ADR 文件的编号

| 编号 | 决定 |
|---|---|
| 0027 | `#[allow]` 的唯一合法位置 |

## 3. 已知编号事故

已退役编号：0019（原「#[allow] 位置」条目，已由 0027 取代）

## 4. 引用规范
";

/// 与 [`CONSISTENT_REGISTRY`] 匹配的 ADR 文件集。
fn consistent_files() -> Vec<AdrFileSummary> {
    vec![AdrFileSummary {
        number: 29,
        file_name: "0029-nightly.md".to_string(),
        status: Some("Accepted".to_string()),
        status_line_number: 3,
        superseded_by: None,
    }]
}

/// 与 [`CONSISTENT_REGISTRY`] 匹配的 decisions.md（含 0027 与退役的 0019）。
const CONSISTENT_DECISIONS: &str = "\
- [2026-09-16][DECISION][ADR:待建 0027] `#[allow]` 的唯一合法位置
- [2026-09-18][DECISION][ADR:待建 0027][supersedes: 2026-09-16 的 [ADR:待建 0019] 条目] 改号
";

/// 只取规则标识符。
fn rules_of(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|finding| finding.rule).collect()
}

/// 取出命中某规则的那条发现项。
fn the_finding<'a>(findings: &'a [Finding], rule: &str) -> &'a Finding {
    findings
        .iter()
        .find(|finding| finding.rule == rule)
        .unwrap_or_else(|| panic!("应命中规则 {rule}，实际：{findings:?}"))
}

/// 用替换的方式造一个不一致的登记表，避免每个用例都抄一遍全文。
fn registry_with(replacement_from: &str, replacement_to: &str) -> String {
    CONSISTENT_REGISTRY.replace(replacement_from, replacement_to)
}

// --- 正向 ---

#[test]
fn test_check_consistent_triple_produces_no_findings() {
    let findings = check(
        CONSISTENT_REGISTRY,
        &consistent_files(),
        CONSISTENT_DECISIONS,
    );
    assert!(findings.is_empty(), "不该有发现项：{findings:?}");
}

#[test]
fn test_check_is_deterministic() {
    let files = consistent_files();
    assert_eq!(
        check(CONSISTENT_REGISTRY, &files, CONSISTENT_DECISIONS),
        check(CONSISTENT_REGISTRY, &files, CONSISTENT_DECISIONS),
        "不变量 1：纯函数"
    );
}

// --- 负向：编号分配 ---

#[test]
fn test_check_file_without_registry_row_reports_error() {
    let mut files = consistent_files();
    files.push(AdrFileSummary {
        number: 30,
        file_name: "0030-new.md".to_string(),
        status: Some("Accepted".to_string()),
        status_line_number: 3,
        superseded_by: None,
    });
    let findings = check(CONSISTENT_REGISTRY, &files, CONSISTENT_DECISIONS);
    let finding = the_finding(&findings, "adr/file-not-in-registry");
    assert_eq!(finding.path, "docs/adr/0030-new.md");
}

#[test]
fn test_check_registry_row_without_file_reports_error() {
    let registry = registry_with(
        "| 0029 | `0029-nightly.md` | Accepted | 夜间自动化改回 Codex |",
        "| 0029 | `0029-nightly.md` | Accepted | 夜间自动化改回 Codex |\n| 0031 | `0031-ghost.md` | Accepted | 不存在的文件 |",
    );
    let findings = check(&registry, &consistent_files(), CONSISTENT_DECISIONS);
    assert!(rules_of(&findings).contains(&"adr/registry-row-without-file"));
}

#[test]
fn test_check_same_number_in_both_tables_reports_collision() {
    // 这就是 0019 事故的复现：同一个号既在 §1 又在 §2
    let registry = registry_with(
        "| 编号 | 决定 |\n|---|---|\n| 0027 |",
        "| 编号 | 决定 |\n|---|---|\n| 0029 | 与文件同号的另一条决策 |\n| 0027 |",
    );
    let findings = check(&registry, &consistent_files(), CONSISTENT_DECISIONS);
    let finding = the_finding(&findings, "adr/number-collision");
    assert!(
        finding.message.contains("0029"),
        "应指出撞号的具体编号：{}",
        finding.message
    );
}

#[test]
fn test_check_retired_number_relisted_as_pending_reports_error() {
    let registry = registry_with(
        "| 0027 | `#[allow]` 的唯一合法位置 |",
        "| 0019 | 被重新分配的退役号 |\n| 0027 | `#[allow]` 的唯一合法位置 |",
    );
    let findings = check(&registry, &consistent_files(), CONSISTENT_DECISIONS);
    assert!(rules_of(&findings).contains(&"adr/retired-number-reallocated"));
}

// --- 负向：§2 与 decisions.md 互为镜像 ---

#[test]
fn test_check_pending_number_absent_from_decisions_reports_error() {
    let findings = check(
        CONSISTENT_REGISTRY,
        &consistent_files(),
        "- [2026-09-16][DECISION] 没有待建号\n",
    );
    assert!(rules_of(&findings).contains(&"adr/pending-not-in-decisions"));
}

#[test]
fn test_check_decisions_number_absent_from_pending_reports_error() {
    let decisions =
        format!("{CONSISTENT_DECISIONS}- [2026-09-18][DECISION][ADR:待建 0028] 新决策\n");
    let findings = check(CONSISTENT_REGISTRY, &consistent_files(), &decisions);
    assert!(rules_of(&findings).contains(&"adr/decisions-not-in-pending"));
}

#[test]
fn test_check_graduated_number_with_file_is_not_flagged() {
    // 0029 在 decisions.md 里仍有 `[ADR:待建 0029]`（只追加），但文件已建成 → 不该报错
    let decisions =
        format!("{CONSISTENT_DECISIONS}- [2026-09-16][DECISION][ADR:待建 0029] 老条目\n");
    let findings = check(CONSISTENT_REGISTRY, &consistent_files(), &decisions);
    assert!(
        !rules_of(&findings).contains(&"adr/decisions-not-in-pending"),
        "已毕业的号不需要留在 §2：{findings:?}"
    );
}

#[test]
fn test_check_retired_number_in_decisions_is_not_flagged() {
    // CONSISTENT_DECISIONS 里含 `[ADR:待建 0019]`（在 supersedes 括注中），0019 已退役 → 不该报错
    let findings = check(
        CONSISTENT_REGISTRY,
        &consistent_files(),
        CONSISTENT_DECISIONS,
    );
    assert!(!rules_of(&findings).contains(&"adr/decisions-not-in-pending"));
}

// --- 负向：下一个可用编号 ---

#[test]
fn test_check_next_number_too_small_reports_error_with_expected() {
    let registry = registry_with("下一个可用编号：0030", "下一个可用编号：0027");
    let findings = check(&registry, &consistent_files(), CONSISTENT_DECISIONS);
    let finding = the_finding(&findings, "adr/next-number-wrong");
    assert!(
        finding.message.contains("0030"),
        "应给出正确值：{}",
        finding.message
    );
}

#[test]
fn test_check_next_number_ignores_retired_and_graduated_numbers() {
    // 已用最大号 = max(§1=0029, §2=0027, 退役=0019, decisions={0019,0027}) = 0029 → 下一个 0030
    let findings = check(
        CONSISTENT_REGISTRY,
        &consistent_files(),
        CONSISTENT_DECISIONS,
    );
    assert!(!rules_of(&findings).contains(&"adr/next-number-wrong"));
}

// --- 负向：状态一致性 ---

#[test]
fn test_check_missing_status_line_reports_error() {
    let mut files = consistent_files();
    let only = files.first_mut().expect("应有一个文件");
    only.status = None;
    only.status_line_number = 0;
    assert!(
        rules_of(&check(CONSISTENT_REGISTRY, &files, CONSISTENT_DECISIONS))
            .contains(&"adr/missing-status-line")
    );
}

#[test]
fn test_check_status_mismatch_reports_error_and_points_to_file_as_truth() {
    let mut files = consistent_files();
    let only = files.first_mut().expect("应有一个文件");
    only.status = Some("Proposed".to_string());
    let findings = check(CONSISTENT_REGISTRY, &files, CONSISTENT_DECISIONS);
    let finding = the_finding(&findings, "adr/status-mismatch");
    assert!(
        finding.message.contains("以**文件**为准"),
        "必须说明谁说了算：{}",
        finding.message
    );
}

#[test]
fn test_check_superseded_file_not_marked_in_registry_reports_error() {
    let mut files = consistent_files();
    let only = files.first_mut().expect("应有一个文件");
    only.superseded_by = Some(31);
    let findings = check(CONSISTENT_REGISTRY, &files, CONSISTENT_DECISIONS);
    let finding = the_finding(&findings, "adr/superseded-not-marked");
    assert!(
        finding.message.contains("ADR-0031"),
        "应指出被谁取代：{}",
        finding.message
    );
}

#[test]
fn test_check_superseded_marked_in_registry_passes() {
    let registry = registry_with(
        "| 0029 | `0029-nightly.md` | Accepted | 夜间自动化改回 Codex |",
        "| 0029 | `0029-nightly.md` | Accepted → **Superseded**（by ADR-0031） | 夜间自动化改回 Codex |",
    );
    let mut files = consistent_files();
    let only = files.first_mut().expect("应有一个文件");
    only.superseded_by = Some(31);
    let findings = check(&registry, &files, CONSISTENT_DECISIONS);
    assert!(!rules_of(&findings).contains(&"adr/superseded-not-marked"));
    assert!(
        !rules_of(&findings).contains(&"adr/status-mismatch"),
        "状态列首个关键词仍是 Accepted，不该误报不符：{findings:?}"
    );
}

// --- 负向：锚点缺失（不变量 3）---

#[test]
fn test_check_empty_registry_reports_missing_anchors_not_success() {
    let findings = check("# 空登记表\n", &[], "");
    assert!(
        rules_of(&findings).contains(&"adr/registry-section-missing"),
        "锚点全缺时必须失败，而不是「没有登记项所以一致」：{findings:?}"
    );
    assert!(
        !rules_of(&findings).contains(&"adr/next-number-wrong"),
        "一个号都没有时不重复报下一个可用编号"
    );
}

// --- leading_status_keyword ---

#[test]
fn test_leading_status_keyword_strips_markup_and_takes_first_word() {
    assert_eq!(
        leading_status_keyword("Accepted → **Superseded**（by ADR-0029）").as_deref(),
        Some("Accepted")
    );
    assert_eq!(
        leading_status_keyword("**Proposed**").as_deref(),
        Some("Proposed")
    );
    assert_eq!(leading_status_keyword("`Draft`").as_deref(), Some("Draft"));
    assert_eq!(leading_status_keyword("   "), None);
}
