//! # ADR 编号一致性规则（ADR-0030 D3 的「判定」一半）
//!
//! 职责：把 `adr_registry` 解析出的三方数据（登记表 / ADR 文件集 / `decisions.md` 待建条目）
//! 交叉比对，产出 11 条规则的 `Finding`。
//!
//! ## 为什么需要这个模块
//! 编号是 ADR 体系的主键。2026-09-18 发生过 **0019 号被两条不同决策占用**的真实事故
//! （ADR-0026）：建文件时没查预留表，两条决策静默共号 —— git 不报错、CI 不报错、
//! 人也不会天天核对。主键重复会让之后所有 `Supersedes` / `Superseded by` 链条失真，
//! 而夜间自动化一旦上线就会在无人值守下**自动踩上去**。
//! 其中 `adr/number-collision`（§1 表 ∩ §2 表 ≠ ∅）就是那次事故的**机器判据**。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO，也不解析文本：解析在 `adr_registry.rs`，IO 在 `main.rs`。
//! - 不自动改写登记表：发现不一致时**打印应有的编号与状态**，由人粘贴
//!   （与 `memory_counts` 同理，见 ADR-0030 选项 1）。
//! - 不检查「裸引用」（ADR-0030 D4 / PL-032）：现存 2 处违规在 ADR 正文里，而 ADR 只增不改，
//!   现在实现就是一条永久红灯。
//! - 不校验 `Supersedes` 链条闭环（ADR-0030「重新评估触发条件」③）。
//!
//! ## 不变量
//! 1. 判定是**纯函数**：同样输入必得同样的 `Vec<Finding>`（含顺序）。
//! 2. 规则标识符是稳定契约，改动需 ADR。
//! 3. 锚点缺失必须报 Error，**不得退化成「没有登记项所以全部一致」**（铁律 1）。
//! 4. 编号比较一律基于 `u32` 数值，不基于字符串（`0018` 与 `18` 是同一个号）。
//!
//! 相关：`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`、
//! `docs/adr/0026-adr-number-registry-and-0019-collision.md`、`xtask/src/adr_registry.rs`

use crate::adr_registry::{
    ADR_DIR, AdrFileSummary, DECISIONS_PATH, REGISTRY_PATH, Registry, RegistryRow,
    collect_pending_numbers, parse_registry,
};
use crate::report::{Finding, Severity};

/// 执行全部 11 条规则，返回发现项（已排序，保证输出确定）。
///
/// - `registry_content`：`docs/adr/README.md` 全文
/// - `adr_files`：`docs/adr/NNNN-*.md` 的摘要（**不含** `README.md`，它没有编号）
/// - `decisions_content`：`docs/memory/decisions.md` 全文
#[must_use]
pub fn check(
    registry_content: &str,
    adr_files: &[AdrFileSummary],
    decisions_content: &str,
) -> Vec<Finding> {
    let registry = parse_registry(registry_content);
    let decisions_numbers = collect_pending_numbers(decisions_content);

    let mut findings: Vec<Finding> = registry
        .missing_anchors
        .iter()
        .map(|anchor| {
            Finding::new(
                "adr/registry-section-missing",
                Severity::Error,
                REGISTRY_PATH,
                0,
                format!(
                    "登记表缺少锚点：{anchor}。处置：按 ADR-0026 D1 的四节结构补回\
                     （§1 已存在文件 / §2 待建号 / §3 已知事故与退役清单 / §4 引用规范）；\
                     锚点缺失时本工具**无法**证明编号一致，故必须失败而不是当作一致。"
                ),
            )
        })
        .collect();

    findings.extend(check_allocation(&registry, adr_files));
    findings.extend(check_pending_consistency(&registry, &decisions_numbers));
    findings.extend(check_next_number(&registry, adr_files, &decisions_numbers));
    findings.extend(check_status_cells(&registry, adr_files));
    sort_findings(findings)
}

/// 规则 1~4：编号的分配是否自洽（文件 ↔ §1 表 ↔ §2 表 ↔ 退役清单）。
fn check_allocation(registry: &Registry, adr_files: &[AdrFileSummary]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for file in adr_files {
        if !registry
            .existing
            .iter()
            .any(|row| row.number == file.number)
        {
            findings.push(Finding::new(
                "adr/file-not-in-registry",
                Severity::Error,
                adr_path(&file.file_name),
                0,
                format!(
                    "文件存在但登记表 §1 没有 {:04} 这一行。处置：在 §1 表补一行\
                     （编号 / 文件名 / 状态 / 主题）—— ADR-0026 D1 要求「新建后必回填」。",
                    file.number
                ),
            ));
        }
    }
    for row in &registry.existing {
        if !adr_files.iter().any(|file| file.number == row.number) {
            findings.push(Finding::new(
                "adr/registry-row-without-file",
                Severity::Error,
                REGISTRY_PATH,
                row.line_number,
                format!(
                    "§1 表登记了 {:04}，但 `{ADR_DIR}/` 下没有对应文件。\
                     处置：文件被删/改名 → 同步改表；号只是预留 → 该行应移到 §2 待建表。",
                    row.number
                ),
            ));
        }
    }
    // 这条就是 0019 事故的机器判据：同一个号既「已有文件」又「还在待建」
    for row in &registry.existing {
        if registry.pending.contains(&row.number) {
            findings.push(Finding::new(
                "adr/number-collision",
                Severity::Error,
                REGISTRY_PATH,
                row.line_number,
                format!(
                    "编号 {:04} 同时出现在 §1（已存在文件）与 §2（待建号）—— 这正是 2026-09-18 \
                     的 0019 号双重占用事故（ADR-0026）。处置：两条决策只能留一个号，\
                     另一个改号并按 ADR-0026 D2 追加 `[supersedes: …]` 条目 + §3 退役清单。",
                    row.number
                ),
            ));
        }
    }
    for retired in &registry.retired {
        if registry.pending.contains(retired) {
            findings.push(Finding::new(
                "adr/retired-number-reallocated",
                Severity::Error,
                REGISTRY_PATH,
                0,
                format!(
                    "已退役编号 {retired:04} 又出现在 §2 待建表（被重新分配）。\
                     处置：退役号永久不得复用；给新决策取「下一个可用编号」。"
                ),
            ));
        }
    }
    findings
}

/// 规则 5、6：登记表 §2 与 `decisions.md` 的待建号是否互为镜像。
fn check_pending_consistency(registry: &Registry, decisions_numbers: &[u32]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for number in &registry.pending {
        if !decisions_numbers.contains(number) {
            findings.push(Finding::new(
                "adr/pending-not-in-decisions",
                Severity::Error,
                REGISTRY_PATH,
                0,
                format!(
                    "§2 待建表登记了 {number:04}，但 `{DECISIONS_PATH}` 里找不到 \
                     `[ADR:待建 {number:04}]` 条目。处置：登记表是**描述性**文档（ADR-0026 D1），\
                     决策本体必须先在 decisions.md 落条目。"
                ),
            ));
        }
    }
    for number in decisions_numbers {
        if is_expected_outside_pending(registry, *number) {
            continue;
        }
        findings.push(Finding::new(
            "adr/decisions-not-in-pending",
            Severity::Error,
            DECISIONS_PATH,
            0,
            format!(
                "`decisions.md` 有 `[ADR:待建 {number:04}]`，但登记表 §2 没有这一行。\
                 处置：在 §2 补一行；若该号已退役（如 0019）→ 加进 §3 的 `{}`；\
                 若已建成文件 → §1 应有该行。",
                "已退役编号："
            ),
        ));
    }
    findings
}

/// 一个 `decisions.md` 里的待建号，在什么情况下**不需要**出现在 §2 表里。
///
/// 三种合法情况：① 已退役（改号后老条目按「只追加」规矩永久留在文件里）；
/// ② 已经建成 ADR 文件（号从「待建」毕业了）；③ 已在 §2 表里（那就是正常情况，返回 false）。
fn is_expected_outside_pending(registry: &Registry, number: u32) -> bool {
    registry.retired.contains(&number)
        || registry.pending.contains(&number)
        || registry.existing.iter().any(|row| row.number == number)
}

/// 规则 7：「下一个可用编号」是否等于 max(全部已用号) + 1。
fn check_next_number(
    registry: &Registry,
    adr_files: &[AdrFileSummary],
    decisions_numbers: &[u32],
) -> Vec<Finding> {
    let mut used: Vec<u32> = Vec::new();
    used.extend(registry.existing.iter().map(|row| row.number));
    used.extend(registry.pending.iter().copied());
    used.extend(registry.retired.iter().copied());
    used.extend(decisions_numbers.iter().copied());
    used.extend(adr_files.iter().map(|file| file.number));
    let Some(highest) = used.iter().max() else {
        // 一个号都没有：说明锚点全缺，已由 adr/registry-section-missing 报错，不重复报
        return Vec::new();
    };
    let expected = highest + 1;
    if registry.next_available == Some(expected) {
        return Vec::new();
    }
    vec![Finding::new(
        "adr/next-number-wrong",
        Severity::Error,
        REGISTRY_PATH,
        0,
        format!(
            "「下一个可用编号」写的是 {:?}，应为 {expected:04}（= 已用最大号 {highest:04} + 1）。\
             处置：改这一处；新建 ADR 前必查本表（ADR-0026 D1）。",
            registry.next_available
        ),
    )]
}

/// 规则 8~10：ADR 文件的状态行与 §1 表的状态列是否一致。
fn check_status_cells(registry: &Registry, adr_files: &[AdrFileSummary]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for file in adr_files {
        if file.status.is_none() {
            findings.push(Finding::new(
                "adr/missing-status-line",
                Severity::Error,
                adr_path(&file.file_name),
                0,
                "找不到含「状态：」的行（gov §9.3 模板要求）。处置：补状态行，\
                 形如 `状态：**Accepted**（日期，来源）　Supersedes：—　Superseded by：—`。",
            ));
        }
        let Some(row) = registry
            .existing
            .iter()
            .find(|row| row.number == file.number)
        else {
            // 未登记的情况已由 adr/file-not-in-registry 报过，不重复报状态问题
            continue;
        };
        findings.extend(compare_status(row, file));
    }
    findings
}

/// 比对单个 ADR 的状态：登记表状态列的首个关键词 vs 文件状态行的关键词，以及取代标注。
fn compare_status(row: &RegistryRow, file: &AdrFileSummary) -> Vec<Finding> {
    let mut findings = Vec::new();
    // let-chain（edition 2024）：「状态行缺失」与「状态不一致」是两件事 —— 缺失已由
    // adr/missing-status-line 单独报，这里只在两侧都取到值且不同时报 status-mismatch。
    if let (Some(declared), Some(actual)) = (leading_status_keyword(&row.status_cell), &file.status)
        && declared != *actual
    {
        findings.push(Finding::new(
            "adr/status-mismatch",
            Severity::Error,
            REGISTRY_PATH,
            row.line_number,
            format!(
                "{:04} 的状态：登记表写 `{declared}`，文件状态行写 `{actual}`。\
                 处置：以**文件**为准改登记表（ADR 正文只增不改，状态行是唯一例外）。",
                row.number
            ),
        ));
    }
    if file.superseded_by.is_some() && !row.status_cell.contains("Superseded") {
        findings.push(Finding::new(
            "adr/superseded-not-marked",
            Severity::Error,
            REGISTRY_PATH,
            row.line_number,
            format!(
                "{:04} 的文件状态行写了 `Superseded by：ADR-{:04}`，但登记表 §1 的状态列\
                 没有标 Superseded。处置：状态列写成 \
                 `Accepted → **Superseded**（by ADR-{:04}）`，让读者一眼看出它已不是现行决策。",
                row.number,
                file.superseded_by.unwrap_or(0),
                file.superseded_by.unwrap_or(0)
            ),
        ));
    }
    findings
}

/// 取状态列的**首个**关键词：先剥掉 `*` 与反引号，再截到第一个空格或全角/半角左括号。
///
/// 例：`` `Accepted → **Superseded**（by ADR-0029）` `` → `Accepted`。
/// 之所以取首个而不是「含 Superseded 就算 Superseded」：被取代的 ADR 其**原始状态**
/// 仍然是 Accepted（它当年确实生效过），登记表要同时表达这两件事。
fn leading_status_keyword(cell: &str) -> Option<String> {
    let cleaned = cell.trim().trim_matches('`').trim_matches('*').trim();
    if cleaned.is_empty() {
        return None;
    }
    let end = cleaned
        .char_indices()
        .find(|(_, character)| matches!(character, ' ' | '（' | '(' | '　'))
        .map_or(cleaned.len(), |(index, _)| index);
    Some(cleaned.get(..end).unwrap_or(cleaned).to_string())
}

/// 拼出 ADR 文件的仓库相对路径（统一用 `/`）。
fn adr_path(file_name: &str) -> String {
    format!("{ADR_DIR}/{file_name}")
}

/// 按 (路径, 行号, 规则) 排序，保证不变量 1。
fn sort_findings(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    findings
}

#[cfg(test)]
// 测试体外置到 `adr_index_tests.rs`（理由见该文件头）；`#[path]` 让它仍是本模块的私有单测。
#[path = "adr_index_tests.rs"]
mod tests;
