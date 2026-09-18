//! # 记忆文件规模一致性规则（ADR-0030 D1/D2 的「判定」一半）
//!
//! 职责：把 `memory_table` 测出来的**实测值**与 `MEMORY.md` 规模表里的**登记值**逐格比对，
//! 产出 8 条规则的 `Finding`。
//!
//! ## 为什么需要这个模块（而不是「写文档时更小心」）
//! 规模表里的数字全部是**派生值**，却被手抄进了文档，于是每次追加记忆条目都要手工同步
//! 7~10 个数字。2026-09-18 一天之内失败两次（PL-022 与其现场复现，第二次发生在
//! 刚修完第一次之后）。危害不只是数字难看：规模表是「该读多少」的路由依据，
//! 而「任一文件 > 400 行触发归档」这条规则**依赖行数正确** —— 行数过期 = 归档永不触发。
//! 结论：靠注意力兜不住，只能机器校验（ADR-0030「背景」节的三方案对比）。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO，也不解析表格：测量与解析在 `memory_table.rs`，IO 在 `main.rs`。
//! - 不自动改写 `MEMORY.md`：发现不一致时**打印可直接粘贴的正确值**，由人粘贴
//!   （ADR-0030 选项 1 已否决自动改写，因为它会违反 xtask 不变量 3「只读」）。
//! - 不判断条目内容对不对，只判断计数对不对。
//!
//! ## 不变量
//! 1. 判定是**纯函数**：同样输入必得同样的 `Vec<Finding>`（含顺序，见 [`sort_findings`]）。
//! 2. 规则标识符（`Finding::rule`）是稳定契约，改动需 ADR —— CI 与任务卡都按它引用。
//! 3. 阈值来自 ADR-0021，**不得在本文件调整**：`INDEX_LINE_LIMIT` = 150（硬上限，不得提高）、
//!    `ARCHIVE_LINE_THRESHOLD` = 400（按体积归档的触发点）。
//! 4. 表解析失败时**只做仓库级报错，不再逐格比对**：否则会刷一屏次生报错，
//!    把真正的根因（表结构坏了）淹没掉。
//!
//! 相关：`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`、
//! `docs/adr/0021-memory-layering-and-app-profiles.md`、`xtask/src/memory_table.rs`

use crate::memory_table::{
    INDEX_PATH, MEMORY_DIR, MeasuredFile, RegisteredFile, SCALE_SECTION_HEADING, count_lines,
    parse_scale_table,
};
use crate::report::{Finding, Severity};

/// `MEMORY.md` 的行数硬上限（ADR-0021）。
///
/// 超过说明有内容该下沉到 L1/L2，**不得提高上限**（ADR-0021 明确否决过「提到 600 行」）。
pub const INDEX_LINE_LIMIT: usize = 150;

/// 单个 L1 记忆文件的归档触发阈值（ADR-0021：> 400 行按主题拆入 `docs/memory/archive/`）。
pub const ARCHIVE_LINE_THRESHOLD: usize = 400;

/// `MEMORY.md` 正文中禁止出现的**派生合计**字样（ADR-0030 D2）。
///
/// 为什么禁：「L1 现合计 207 条」这类数字会随每次追加漂移，而它又不在规模表里，
/// 属于「同一事实手写多处」。迁移当时的历史快照数字（155 / 200 / 7）不含此字样，故不受影响。
const DERIVED_TOTAL_MARKER: &str = "现合计";

/// 执行全部 8 条规则，返回发现项（已按 路径 → 行号 → 规则 排序，保证输出确定）。
///
/// `index_content` 是 `MEMORY.md` 全文；`measured` 是 `docs/memory/` 下被扫描文件的实测规模，
/// 其 `relative_path` 必须相对于 `docs/memory/`（与规模表第 1 列同口径）。
#[must_use]
pub fn check(index_content: &str, measured: &[MeasuredFile]) -> Vec<Finding> {
    let mut findings = check_index_itself(index_content);
    match parse_scale_table(index_content) {
        Ok(registered) => {
            findings.extend(check_registered_rows(&registered, measured));
            findings.extend(check_unlisted_and_oversized(&registered, measured));
        }
        // 不变量 4：表都解析不出来，逐格比对没有意义
        Err(reason) => findings.push(unparsable_finding(0, &reason, "恢复四列表结构")),
    }
    sort_findings(findings)
}

/// 规则 1、2：只针对 `MEMORY.md` 自身（行数上限、正文派生合计）。
fn check_index_itself(index_content: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let index_lines = count_lines(index_content);
    if index_lines > INDEX_LINE_LIMIT {
        findings.push(Finding::new(
            "memory/index-too-long",
            Severity::Error,
            INDEX_PATH,
            0,
            format!(
                "{INDEX_PATH} 共 {index_lines} 行，超过硬上限 {INDEX_LINE_LIMIT} 行（ADR-0021）。\
                 处置：把内容下沉到 docs/memory/ 的 L1 文件，**不要提高上限**。"
            ),
        ));
    }
    if let Some(marker_line) = find_line_containing(index_content, DERIVED_TOTAL_MARKER) {
        findings.push(Finding::new(
            "memory/derived-total-in-prose",
            Severity::Error,
            INDEX_PATH,
            marker_line,
            format!(
                "正文出现「{DERIVED_TOTAL_MARKER}」这类会随追加漂移的派生合计。\
                 处置：删掉该数字，改为指向规模表（唯一落点）；\
                 历史快照请写「当时值」而不用此字样（ADR-0030 D2）。"
            ),
        ));
    }
    findings
}

/// 规则 3~6：遍历规模表的每一行，与实测值比对。
fn check_registered_rows(registered: &[RegisteredFile], measured: &[MeasuredFile]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for row in registered {
        findings.extend(check_one_registered_row(row, measured));
    }
    findings
}

/// 检查规模表的单行：结构是否可读 → 文件是否存在 → 两个计数是否相符。
///
/// 三种情况互斥，故用提前返回而不是嵌套 `if`（把圈复杂度压在 clippy.toml 的 15 以内）。
fn check_one_registered_row(row: &RegisteredFile, measured: &[MeasuredFile]) -> Vec<Finding> {
    if row.line_count.is_none() || row.entry_count.is_none() {
        return vec![unparsable_finding(
            row.line_number,
            &format!(
                "规模表第 {} 行的计数列不是纯数字（行数={:?} 条目数={:?}）",
                row.line_number, row.line_count, row.entry_count
            ),
            "填整数；未知就写实测值，不要写 `—` 或留空",
        )];
    }
    let Some(actual) = find_measured(measured, &row.relative_path) else {
        return vec![Finding::new(
            "memory/file-missing",
            Severity::Error,
            INDEX_PATH,
            row.line_number,
            format!(
                "规模表登记了 `{}/{}`，但该文件不存在。\
                 处置：文件被移动或改名 → 同步改表；被误删 → 从 git 恢复。",
                MEMORY_DIR, row.relative_path
            ),
        )];
    };
    compare_counts(row, actual)
}

/// 规则 5、6：行数与条目数逐格比对，并在消息里给出**可直接粘贴的正确值**。
fn compare_counts(row: &RegisteredFile, actual: &MeasuredFile) -> Vec<Finding> {
    let mut findings = Vec::new();
    // 前置条件：调用方（check_one_registered_row）已保证两个计数都是 Some
    let declared_lines = row.line_count.unwrap_or(0);
    if declared_lines != actual.line_count {
        findings.push(Finding::new(
            "memory/line-count-mismatch",
            Severity::Error,
            INDEX_PATH,
            row.line_number,
            format!(
                "`{}` 行数：表中 {} ≠ 实测 {}。处置：把该行第 2 列改为 {}。",
                actual.relative_path, declared_lines, actual.line_count, actual.line_count
            ),
        ));
    }
    let declared_entries = row.entry_count.unwrap_or(0);
    if declared_entries != actual.entry_count {
        findings.push(Finding::new(
            "memory/entry-count-mismatch",
            Severity::Error,
            INDEX_PATH,
            row.line_number,
            format!(
                "`{}` 条目数：表中 {} ≠ 实测 {}。处置：把该行第 3 列改为 {}。",
                actual.relative_path, declared_entries, actual.entry_count, actual.entry_count
            ),
        ));
    }
    findings
}

/// 规则 7、8：反向遍历实测文件 —— 未登记的、超过归档阈值的。
fn check_unlisted_and_oversized(
    registered: &[RegisteredFile],
    measured: &[MeasuredFile],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for actual in measured {
        let full_path = format!("{MEMORY_DIR}/{}", actual.relative_path);
        let is_registered = registered
            .iter()
            .any(|row| row.relative_path == actual.relative_path);
        if !is_registered {
            findings.push(Finding::new(
                "memory/file-unlisted",
                Severity::Error,
                full_path.clone(),
                0,
                format!(
                    "该文件未登记进 {INDEX_PATH} 的规模表。可直接粘贴的行：\
                     | `{}` | {} | {} | <读法待填> |",
                    actual.relative_path, actual.line_count, actual.entry_count
                ),
            ));
        }
        if actual.line_count > ARCHIVE_LINE_THRESHOLD {
            findings.push(Finding::new(
                "memory/archive-threshold",
                // Warning 而非 Error：归档是有成本的人工动作（要选主题、要留指针、要登记两处），
                // 不该因为它阻塞一次与归档无关的提交
                Severity::Warning,
                full_path,
                0,
                format!(
                    "实测 {} 行 > 归档阈值 {ARCHIVE_LINE_THRESHOLD} 行（ADR-0021）。\
                     处置：按**主题**（不按时间）拆入 docs/memory/archive/，原处留指针，\
                     并在 MEMORY.md §7 归档索引与 LEDGER.md 各登记一行。",
                    actual.line_count
                ),
            ));
        }
    }
    findings
}

/// 构造 `memory/scale-table-unparsable` 发现项（表结构缺陷，铁律 1：不得静默通过）。
fn unparsable_finding(line_number: usize, reason: &str, remedy: &str) -> Finding {
    Finding::new(
        "memory/scale-table-unparsable",
        Severity::Error,
        INDEX_PATH,
        line_number,
        format!(
            "{reason}。处置：{remedy}（ADR-0030 D1 规定的形态是 \
             「| 文件 | 行数 | 条目数 | 读法 |」四列，锚点为 `{SCALE_SECTION_HEADING}`）。"
        ),
    )
}

/// 在实测列表里按相对路径查一个文件。
fn find_measured<'a>(
    measured: &'a [MeasuredFile],
    relative_path: &str,
) -> Option<&'a MeasuredFile> {
    measured
        .iter()
        .find(|file| file.relative_path == relative_path)
}

/// 返回第一个包含 `needle` 的行的 1 基行号。
fn find_line_containing(content: &str, needle: &str) -> Option<usize> {
    content
        .lines()
        .position(|line| line.contains(needle))
        .map(|index| index + 1)
}

/// 按 (路径, 行号, 规则) 排序，保证不变量 1（输出确定，可逐字节比对两次运行）。
fn sort_findings(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    findings
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// 一份与 [`consistent_measured`] 完全一致的 `MEMORY.md`（正向基准）。
    const CONSISTENT_INDEX: &str = "\
# MEMORY.md — 项目长期记忆

## 各文件当前规模（Orchestrator 每次归档/大改后更新本表）

| 文件 | 行数 | 条目数 | 读法 |
|---|---|---|---|
| `facts.md` | 3 | 2 | grep 优先 |
| `apps/notepad.md` | 1 | 0 | 全量读 |

## §1 快照
";

    /// 与 [`CONSISTENT_INDEX`] 匹配的实测值。
    fn consistent_measured() -> Vec<MeasuredFile> {
        vec![
            MeasuredFile {
                relative_path: "facts.md".to_string(),
                line_count: 3,
                entry_count: 2,
            },
            MeasuredFile {
                relative_path: "apps/notepad.md".to_string(),
                line_count: 1,
                entry_count: 0,
            },
        ]
    }

    /// 只取规则标识符，让断言读起来是「命中了哪些规则」。
    fn rules_of(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(|finding| finding.rule).collect()
    }

    /// 取出命中某规则的那条发现项（级别由调用方断言）。
    fn the_finding<'a>(findings: &'a [Finding], rule: &str) -> &'a Finding {
        findings
            .iter()
            .find(|finding| finding.rule == rule)
            .unwrap_or_else(|| panic!("应命中规则 {rule}，实际：{findings:?}"))
    }

    // --- 正向 ---

    #[test]
    fn test_check_consistent_state_produces_no_findings() {
        let findings = check(CONSISTENT_INDEX, &consistent_measured());
        assert!(findings.is_empty(), "不该有发现项：{findings:?}");
    }

    #[test]
    fn test_check_is_deterministic() {
        let measured = consistent_measured();
        assert_eq!(
            check(CONSISTENT_INDEX, &measured),
            check(CONSISTENT_INDEX, &measured),
            "不变量 1：纯函数，两次结果必须相同"
        );
    }

    // --- 负向：每条规则至少一个用例 ---

    #[test]
    fn test_check_index_over_line_limit_reports_error() {
        let long_index = format!(
            "{CONSISTENT_INDEX}{}\n",
            "填充行\n".repeat(INDEX_LINE_LIMIT)
        );
        let findings = check(&long_index, &consistent_measured());
        assert_eq!(
            the_finding(&findings, "memory/index-too-long").severity,
            Severity::Error
        );
    }

    #[test]
    fn test_check_index_exactly_at_limit_does_not_report() {
        // 负向边界：判据是「> 150」，恰好 150 行不告警
        let padding = INDEX_LINE_LIMIT - count_lines(CONSISTENT_INDEX);
        // 不要再补结尾换行：`repeat` 的产物已以 \n 结尾，
        // 而 `count_lines` 是按 \n 计数的（见 memory_table.rs），多一个就多出一行空行
        let index = format!("{CONSISTENT_INDEX}{}", "填充行\n".repeat(padding));
        assert_eq!(
            count_lines(&index),
            INDEX_LINE_LIMIT,
            "前置条件：正好 150 行"
        );
        assert!(
            !rules_of(&check(&index, &consistent_measured())).contains(&"memory/index-too-long")
        );
    }

    #[test]
    fn test_check_derived_total_in_prose_reports_error_with_line_number() {
        let index = format!("{CONSISTENT_INDEX}\n> L1 现合计 207 条。\n");
        let findings = check(&index, &consistent_measured());
        let finding = the_finding(&findings, "memory/derived-total-in-prose");
        assert!(finding.line > 0, "必须定位到具体行，实际 {}", finding.line);
    }

    #[test]
    fn test_check_line_count_mismatch_gives_pasteable_correct_value() {
        let mut measured = consistent_measured();
        measured.first_mut().expect("应有 facts.md").line_count = 99;
        let findings = check(CONSISTENT_INDEX, &measured);
        let finding = the_finding(&findings, "memory/line-count-mismatch");
        assert!(
            finding.message.contains("改为 99"),
            "必须给出可直接粘贴的正确值：{}",
            finding.message
        );
    }

    #[test]
    fn test_check_entry_count_mismatch_reports_error() {
        let mut measured = consistent_measured();
        measured.first_mut().expect("应有 facts.md").entry_count = 7;
        let findings = check(CONSISTENT_INDEX, &measured);
        assert!(rules_of(&findings).contains(&"memory/entry-count-mismatch"));
    }

    #[test]
    fn test_check_registered_file_absent_on_disk_reports_file_missing() {
        let measured = vec![MeasuredFile {
            relative_path: "facts.md".to_string(),
            line_count: 3,
            entry_count: 2,
        }];
        let findings = check(CONSISTENT_INDEX, &measured);
        let finding = the_finding(&findings, "memory/file-missing");
        assert!(
            finding.message.contains("apps/notepad.md"),
            "应指出是哪个文件不见了：{}",
            finding.message
        );
    }

    #[test]
    fn test_check_unlisted_file_reports_error_with_pasteable_row() {
        let mut measured = consistent_measured();
        measured.push(MeasuredFile {
            relative_path: "rejected.md".to_string(),
            line_count: 5,
            entry_count: 4,
        });
        let findings = check(CONSISTENT_INDEX, &measured);
        let finding = the_finding(&findings, "memory/file-unlisted");
        assert!(
            finding.message.contains("| `rejected.md` | 5 | 4 |"),
            "应给出可直接粘贴的表行：{}",
            finding.message
        );
        assert_eq!(
            finding.path, "docs/memory/rejected.md",
            "路径口径应是仓库相对路径"
        );
    }

    #[test]
    fn test_check_unparsable_table_reports_error_and_skips_cellwise_comparison() {
        let findings = check("# MEMORY.md\n\n没有规模表\n", &consistent_measured());
        assert!(rules_of(&findings).contains(&"memory/scale-table-unparsable"));
        assert!(
            !rules_of(&findings).contains(&"memory/file-unlisted"),
            "不变量 4：表坏了就不要再刷一屏次生报错"
        );
    }

    #[test]
    fn test_check_non_numeric_cell_reports_unparsable_on_that_row() {
        let index = CONSISTENT_INDEX.replace("| `facts.md` | 3 | 2 |", "| `facts.md` | — | 2 |");
        let findings = check(&index, &consistent_measured());
        let finding = the_finding(&findings, "memory/scale-table-unparsable");
        assert_eq!(finding.line, 7, "应定位到出问题的那一行");
    }

    #[test]
    fn test_check_archive_threshold_is_warning_not_error() {
        let mut measured = consistent_measured();
        let facts = measured.first_mut().expect("应有 facts.md");
        facts.line_count = ARCHIVE_LINE_THRESHOLD + 1;
        let index = CONSISTENT_INDEX.replace("| `facts.md` | 3 | 2 |", "| `facts.md` | 401 | 2 |");
        let findings = check(&index, &measured);
        let finding = the_finding(&findings, "memory/archive-threshold");
        assert_eq!(
            finding.severity,
            Severity::Warning,
            "归档是人工动作，不该阻塞 CI"
        );
    }

    #[test]
    fn test_check_exactly_at_archive_threshold_does_not_warn() {
        // 负向边界：判据是「> 400」，恰好 400 不告警
        let mut measured = consistent_measured();
        measured.first_mut().expect("应有 facts.md").line_count = ARCHIVE_LINE_THRESHOLD;
        let index = CONSISTENT_INDEX.replace("| `facts.md` | 3 | 2 |", "| `facts.md` | 400 | 2 |");
        let findings = check(&index, &measured);
        assert!(!rules_of(&findings).contains(&"memory/archive-threshold"));
    }

    // --- 输出确定性 ---

    #[test]
    fn test_check_findings_are_sorted_by_path_then_line() {
        // 排序至少需要两条发现项，而且要跨**不同 path**才有意义：
        // ① facts.md 的行数与表里不符 → 落在 MEMORY.md 的表格行上；
        // ② aaa.md 完全没登记 → 落在 docs/memory/aaa.md 上。
        // 只造 ② 的话永远只有 1 条，"已排序"就成了空断言（恒真），测不出任何东西。
        let mut measured = consistent_measured();
        measured.first_mut().expect("应有 facts.md").line_count = 99;
        measured.push(MeasuredFile {
            relative_path: "aaa.md".to_string(),
            line_count: 1,
            entry_count: 1,
        });
        let findings = check(CONSISTENT_INDEX, &measured);
        let rules = rules_of(&findings);
        assert!(
            rules.contains(&"memory/line-count-mismatch")
                && rules.contains(&"memory/file-unlisted"),
            "前置条件：两类发现项都要命中，实际 {rules:?}"
        );
        let keys: Vec<(String, usize)> = findings
            .iter()
            .map(|finding| (finding.path.clone(), finding.line))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "发现项必须排序（不变量 1）");
    }
}
