//! # 未实现项登记表（把「还没做」变成显式事实）
//!
//! 职责：登记 xtask 中**尚未实现的子命令**与**尚未实现的卫生规则**，
//! 并给出每一项的归属任务卡号；让工具在未实现时**显式失败**而不是静默返回成功。
//!
//! ## 为什么需要这张表
//! 铁律 1「无静默失败」在工具链上的具体形态是：CI 里一条 `cargo run -p xtask -- check-comments`
//! 如果因为"还没写"而返回 0，人类会以为这项检查一直在保护仓库 —— 这比没有这项检查更危险，
//! 因为它制造了虚假的安全感。所以未实现项必须：① 登记在册 ② 运行时报错并指出归属卡号
//! ③ 在正常运行的输出里声明实现进度（口径见 ADR-0025）。
//!
//! ## 边界（不做什么）
//! - 不做任何 IO：所有函数返回 `String`，打印由 `main.rs` 负责。
//! - 不判断"该不该实现"：那属于任务卡与 PLAN；本模块只如实登记现状。
//!
//! ## 不变量
//! 1. 已实现卫生规则数 = `gov §5.4` 表格数据行数 − `DEFERRED_HYGIENE_RULES.len()`；
//!    负数必须显式失败。
//! 2. 每个 `DeferredCommand::command` 在 `DEFERRED_COMMANDS` 中唯一。
//! 3. 每一项都必须有非空的 `owning_card`（可以是"未分配"，但不能留空 ——
//!    留空意味着没人负责，那才是真正的静默失败）。

/// 从治理文档和 CI 工作流派生出的计数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivedGovernanceCounts {
    /// `gov §5.4` 表格的数据行数。
    pub hygiene_rules: usize,
    /// `gov §5.1` 表格中的门禁编号数量。
    pub governance_gates: usize,
    /// `ci.yml` 中 `# gov-gate: <id>` 标记的数量。
    pub ci_gates: usize,
}

/// 一个尚未实现的子命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredCommand {
    /// 子命令名（与 `xtask <name>` 一致）。
    pub command: &'static str,
    /// 它对应 gov §5.1 的哪一项 CI 门禁（用于说明"缺了它会漏掉什么"）。
    pub ci_gate: &'static str,
    /// 归属任务卡号；未拆卡时写「未分配（见 `docs/PARKING_LOT.md` PL-xxx）」。
    pub owning_card: &'static str,
    /// 为什么现在还不能实现（前置条件），一句话。
    pub reason: &'static str,
}

/// 一条尚未实现的卫生规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredRule {
    /// 规则简称（与 gov §5.4 表格第一列一致）。
    pub rule: &'static str,
    /// 未实现的原因 / 前置条件。
    pub reason: &'static str,
    /// 归属任务卡号。
    pub owning_card: &'static str,
}

/// 未实现的子命令清单。
///
/// 顺序即 `--list-deferred` 的输出顺序，保持稳定以便 diff。
pub const DEFERRED_COMMANDS: &[DeferredCommand] = &[];

/// 未实现的卫生规则清单；TASK-234 后为空。
///
/// 未实现条数参与 `派生总数 − 未实现数` 的计算；未来新增规则时先登记未实现项，
/// 再实现并清空。
pub const DEFERRED_HYGIENE_RULES: &[DeferredRule] = &[];

/// 从 `gov §5.4` 与 `ci.yml` 的文本派生计数并校验门禁集合。
///
/// # Errors
/// 表格 / 标记缺失、重复、额外或不可解析时返回带具体原因的错误。全部解析失败路径都必须
/// 保持显式，禁止把空结果当作 0。
pub fn derive_governance_counts(
    governance_markdown: &str,
    ci_workflow: &str,
) -> Result<DerivedGovernanceCounts, String> {
    let hygiene_rule_total = parse_hygiene_rule_count(governance_markdown)?;
    let governance_gate_ids = parse_governance_gate_ids(governance_markdown)?;
    let ci_gate_ids = parse_ci_gate_ids(ci_workflow)?;
    verify_gate_ids_match(&governance_gate_ids, &ci_gate_ids)?;
    implemented_hygiene_rule_count(hygiene_rule_total)?;
    Ok(DerivedGovernanceCounts {
        hygiene_rules: hygiene_rule_total,
        governance_gates: governance_gate_ids.len(),
        ci_gates: ci_gate_ids.len(),
    })
}

/// 解析 `gov §5.4` 表格的数据行数。
///
/// # Errors
/// 找不到 §5.4 标题、表格头、separator 或数据行时返回错误。
pub fn parse_hygiene_rule_count(markdown: &str) -> Result<usize, String> {
    let section = section_after_heading(markdown, "### 5.4", "gov §5.4")?;
    let rows = markdown_table_rows(section, "检查", "gov §5.4")?;
    Ok(rows.len())
}

/// 解析 `gov §5.1` 表格中的门禁编号集合。
///
/// # Errors
/// 找不到 §5.1 表头 / separator、数据行为空或编号非法时返回错误。
pub fn parse_governance_gate_ids(markdown: &str) -> Result<Vec<String>, String> {
    let section = section_before_next_heading(markdown, "### 5.1", "### 5.2", "gov §5.1")?;
    parse_marked_ids_from_table(section, "#", "gov §5.1")
}

/// 解析 `ci.yml` 中的 `# gov-gate: <id>` 标记。
///
/// # Errors
/// 标记为空、编号非法或重复时返回错误。
pub fn parse_ci_gate_ids(workflow: &str) -> Result<Vec<String>, String> {
    let marker = "# gov-gate:";
    let mut ids = Vec::new();
    for (line_index, line) in workflow.lines().enumerate() {
        let Some(marker_index) = line.find(marker) else {
            continue;
        };
        let tail = &line[marker_index + marker.len()..];
        let id = tail.split_whitespace().next().unwrap_or_default();
        validate_gate_id(id, "ci.yml", line_index + 1)?;
        if ids.iter().any(|existing| existing == id) {
            return Err(format!(
                "ci.yml 第 {} 行重复声明 `{marker} {id}`",
                line_index + 1
            ));
        }
        ids.push(id.to_string());
    }
    if ids.is_empty() {
        return Err("ci.yml 找不到任何 `# gov-gate: <id>` 标记".to_string());
    }
    ids.sort_unstable();
    Ok(ids)
}

/// 校验两个门禁编号集合完全一致。
///
/// # Errors
/// 任一侧缺失或出现额外编号时返回错误。
pub fn verify_gate_ids_match(governance_ids: &[String], ci_ids: &[String]) -> Result<(), String> {
    let missing_from_ci: Vec<&str> = governance_ids
        .iter()
        .filter(|id| !ci_ids.contains(id))
        .map(String::as_str)
        .collect();
    let extra_in_ci: Vec<&str> = ci_ids
        .iter()
        .filter(|id| !governance_ids.contains(id))
        .map(String::as_str)
        .collect();
    if missing_from_ci.is_empty() && extra_in_ci.is_empty() {
        return Ok(());
    }
    Err(format!(
        "gov §5.1 与 ci.yml 的 # gov-gate 集合不一致：\
         gov={} 项，ci={} 项；ci 缺失=[{}]；ci 额外=[{}]",
        governance_ids.len(),
        ci_ids.len(),
        missing_from_ci.join(", "),
        extra_in_ci.join(", ")
    ))
}

/// 从派生总数和未实现清单计算已实现规则数。
///
/// # Errors
/// 未实现条数大于派生总数时返回错误，避免负数或饱和减法掩盖事实源矛盾。
pub fn implemented_hygiene_rule_count(total: usize) -> Result<usize, String> {
    implemented_hygiene_rule_count_for(total, DEFERRED_HYGIENE_RULES.len())
}

/// `implemented_hygiene_rule_count` 的可注入版本，便于用纯负向样本覆盖减法下界。
fn implemented_hygiene_rule_count_for(
    total: usize,
    deferred_count: usize,
) -> Result<usize, String> {
    total.checked_sub(deferred_count).ok_or_else(|| {
        format!("gov §5.4 只派生 {total} 条 hygiene 规则，但未实现清单有 {deferred_count} 条")
    })
}

/// 从 §5.1 / §5.4 标题后的表格中解析指定首列编号。
fn parse_marked_ids_from_table(
    section: &str,
    header_first_cell: &str,
    source_name: &str,
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for (line_number, cells) in markdown_table_rows(section, header_first_cell, source_name)? {
        let raw_id = cells
            .first()
            .ok_or_else(|| format!("{source_name} 第 {line_number} 行缺首列"))?
            .trim()
            .trim_matches('*');
        validate_gate_id(raw_id, source_name, line_number)?;
        ids.push(raw_id.to_string());
    }
    ids.sort_unstable();
    Ok(ids)
}

/// 取 Markdown 表格的数据行（含 1-based 行号与单元格）。
///
/// # Errors
/// 表头、separator 或数据行缺失时返回错误，避免把“没解析到表格”当成空表成功。
fn markdown_table_rows<'text>(
    section: &'text str,
    header_first_cell: &str,
    source_name: &str,
) -> Result<Vec<(usize, Vec<&'text str>)>, String> {
    let mut saw_header = false;
    let mut saw_separator = false;
    let mut rows = Vec::new();
    for (line_index, line) in section.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if saw_separator && !rows.is_empty() {
                break;
            }
            continue;
        }
        let cells = split_table_row(trimmed)?;
        if !saw_header {
            saw_header = cells.first().is_some_and(|cell| *cell == header_first_cell);
            continue;
        }
        if !saw_separator {
            if !is_table_separator(&cells) {
                return Err(format!("{source_name} 的表头后缺少 Markdown separator"));
            }
            saw_separator = true;
            continue;
        }
        rows.push((line_index + 1, cells));
    }
    if !saw_header {
        return Err(format!(
            "{source_name} 找不到表头 `| {header_first_cell} |`"
        ));
    }
    if !saw_separator {
        return Err(format!("{source_name} 找不到表格 separator"));
    }
    if rows.is_empty() {
        return Err(format!("{source_name} 表格没有数据行"));
    }
    Ok(rows)
}

/// 取指定 Markdown 标题到下一个以 `###` 开头的标题之间的内容。
fn section_before_next_heading<'text>(
    markdown: &'text str,
    heading: &str,
    next_heading: &str,
    source_name: &str,
) -> Result<&'text str, String> {
    let section = section_after_heading(markdown, heading, source_name)?;
    Ok(section.split(next_heading).next().unwrap_or(section))
}

/// 取指定 Markdown 标题后的内容。
fn section_after_heading<'text>(
    markdown: &'text str,
    heading: &str,
    source_name: &str,
) -> Result<&'text str, String> {
    markdown
        .split_once(heading)
        .map(|(_, section)| section)
        .ok_or_else(|| format!("{source_name} 找不到标题 `{heading}`"))
}

/// 按 Markdown 表格规则拆分一行并去掉首尾空单元格。
fn split_table_row(line: &str) -> Result<Vec<&str>, String> {
    let trimmed = line.trim().trim_matches('|');
    let cells: Vec<&str> = trimmed.split('|').map(str::trim).collect();
    if cells.is_empty() {
        return Err("Markdown 表格行为空".to_string());
    }
    Ok(cells)
}

/// 判断一行是否是 Markdown 表格 separator。
fn is_table_separator(cells: &[&str]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            !cell.is_empty() && cell.chars().all(|character| matches!(character, '-' | ':'))
        })
}

/// 校验门禁编号只含 ASCII 字母数字。
fn validate_gate_id(id: &str, source_name: &str, line_number: usize) -> Result<(), String> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return Err(format!(
            "{source_name} 第 {line_number} 行的门禁编号 `{id}` 非法"
        ));
    }
    Ok(())
}

/// 按名字查找未实现的子命令；找不到说明它是未知命令（由 `main.rs` 区分处理）。
#[must_use]
pub fn find_command(name: &str) -> Option<&'static DeferredCommand> {
    DEFERRED_COMMANDS.iter().find(|entry| entry.command == name)
}

/// 生成「子命令未实现」的完整报错文本（由 `main` 写往 stderr）。
///
/// 文本必须包含归属卡号：这样看到报错的人（或 agent）知道该去哪张卡补实现，
/// 而不是自己去猜或者顺手实现（那会造成跨卡漂移）。
///
/// 只接受**已登记**的条目：未登记的命令由 `main` 判为用法错误，不走这条路径。
#[must_use]
pub fn not_implemented_message(entry: &DeferredCommand) -> String {
    format!(
        "子命令 `{}` 尚未实现。\n  门禁：{}\n  归属：{}\n  原因：{}\n\
         本工具**故意**返回失败而不是成功（AGENTS.md 铁律 1「无静默失败」）。",
        entry.command, entry.ci_gate, entry.owning_card, entry.reason
    )
}

/// 生成未实现子命令清单的可读文本（`--list-deferred` 用）。
#[must_use]
pub fn describe_deferred_commands() -> String {
    if DEFERRED_COMMANDS.is_empty() {
        return "未实现的子命令：0 项。\n".to_string();
    }
    let rows: Vec<String> = DEFERRED_COMMANDS
        .iter()
        .map(|entry| {
            format!(
                "  {:<15} {:<13} {:<44} {}",
                entry.command, entry.ci_gate, entry.owning_card, entry.reason
            )
        })
        .collect();
    format!("未实现的子命令（运行时显式失败）：\n{}\n", rows.join("\n"))
}

/// 生成未实现卫生规则清单的可读文本（`--list-deferred` 用）。
pub fn describe_deferred_rules(total_hygiene_rule_count: usize) -> Result<String, String> {
    let implemented = implemented_hygiene_rule_count(total_hygiene_rule_count)?;
    if DEFERRED_HYGIENE_RULES.is_empty() {
        return Ok(format!(
            "gov §5.4 的 {total_hygiene_rule_count} 项卫生规则已全部实现：未实现 0 项。\n"
        ));
    }
    let rows: Vec<String> = DEFERRED_HYGIENE_RULES
        .iter()
        .map(|entry| {
            format!(
                "  - {:<34} 归属 {}；{}",
                entry.rule, entry.owning_card, entry.reason
            )
        })
        .collect();
    Ok(format!(
        "gov §5.4 的 {total_hygiene_rule_count} 项卫生规则中，已实现 {implemented} 项，未实现 {} 项：\n{}\n",
        DEFERRED_HYGIENE_RULES.len(),
        rows.join("\n")
    ))
}

/// 生成一行「进度声明」，在每次 `hygiene` 运行时打印。
///
/// 为什么每次都要打印：如果不声明，`verdict: PASSED` 会被误读成"13 项卫生规则全过"。
/// 让工具主动承认自己只检查了一部分，是防止虚假安全感的最低成本手段。
pub fn hygiene_progress_note(total_hygiene_rule_count: usize) -> Result<String, String> {
    let implemented = implemented_hygiene_rule_count(total_hygiene_rule_count)?;
    Ok(format!(
        "-- deferred-rules: gov §5.4 共 {} 项，已实现 {} 项，未实现 {} 项（`--list-deferred` 查看清单）",
        total_hygiene_rule_count,
        implemented,
        DEFERRED_HYGIENE_RULES.len()
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const GOVERNANCE_FIXTURE: &str = r"### 5.1 CI 门禁

| # | 检查 |
|---|---|
| 1 | fmt |
| **2b** | derived |

### 5.2 下一节

### 5.4 仓库卫生检查

| 检查 | 阈值 |
|---|---|
| 单文件行数 | warning |
| 圈复杂度 | warning |
";

    const CI_FIXTURE: &str = r"# skeleton
# gov-gate: 1
run: cargo fmt --all --check
# gov-gate: 2b
run: cargo run -p xtask -- hygiene
";

    #[test]
    fn test_derived_counts_follow_the_governance_tables() {
        let counts =
            derive_governance_counts(GOVERNANCE_FIXTURE, CI_FIXTURE).expect("匹配的正负样本应通过");
        assert_eq!(counts.hygiene_rules, 2);
        assert_eq!(counts.governance_gates, 2);
        assert_eq!(counts.ci_gates, 2);
        assert_eq!(implemented_hygiene_rule_count(counts.hygiene_rules), Ok(2));
    }

    #[test]
    fn test_derived_counts_reject_missing_ci_gate_marker() {
        let ci = CI_FIXTURE.replace("# gov-gate: 2b\n", "");
        let error =
            derive_governance_counts(GOVERNANCE_FIXTURE, &ci).expect_err("缺失 CI 标记必须失败");
        assert!(error.contains("集合不一致"), "实际错误：{error}");
        assert!(error.contains("2b"), "实际错误：{error}");
    }

    #[test]
    fn test_derived_counts_reject_duplicate_ci_gate_marker() {
        let ci = format!("{CI_FIXTURE}# gov-gate: 1\n");
        let error =
            derive_governance_counts(GOVERNANCE_FIXTURE, &ci).expect_err("重复标记必须失败");
        assert!(error.contains("重复声明"), "实际错误：{error}");
    }

    #[test]
    fn test_derived_counts_reject_unparsable_governance_table() {
        let broken = GOVERNANCE_FIXTURE.replace("|---|---|\n| 1 | fmt |", "| 1 | fmt |");
        let error =
            derive_governance_counts(&broken, CI_FIXTURE).expect_err("缺 separator 必须失败");
        assert!(error.contains("separator"), "实际错误：{error}");
    }

    #[test]
    fn test_derived_implemented_count_rejects_negative_difference() {
        let error =
            implemented_hygiene_rule_count_for(0, 1).expect_err("未实现数大于派生总数必须失败");
        assert!(error.contains("未实现清单有 1 条"), "实际错误：{error}");
    }

    #[test]
    fn test_deferred_command_names_are_unique() {
        // 不变量 2：命令名唯一，否则 find_command 的返回值不可预测
        let mut names: Vec<&str> = DEFERRED_COMMANDS
            .iter()
            .map(|entry| entry.command)
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "DEFERRED_COMMANDS 中存在重名命令");
    }

    #[test]
    fn test_every_deferred_entry_has_owning_card() {
        // 不变量 3：不允许空归属
        for entry in DEFERRED_COMMANDS {
            assert!(
                !entry.owning_card.trim().is_empty(),
                "命令 {} 缺归属",
                entry.command
            );
            assert!(
                !entry.ci_gate.trim().is_empty(),
                "命令 {} 缺门禁引用",
                entry.command
            );
            assert!(
                !entry.reason.trim().is_empty(),
                "命令 {} 缺原因",
                entry.command
            );
        }
        for entry in DEFERRED_HYGIENE_RULES {
            assert!(
                !entry.owning_card.trim().is_empty(),
                "规则 {} 缺归属",
                entry.rule
            );
        }
    }

    #[test]
    fn test_find_command_returns_none_after_replay_completion() {
        assert!(
            find_command("replay-skeleton").is_none(),
            "TASK-236 完成后 replay 不再属于未实现子命令"
        );
    }

    #[test]
    fn test_find_command_returns_none_for_unknown_name() {
        assert!(find_command("deploy-to-production").is_none());
        assert!(
            find_command("hygiene").is_none(),
            "hygiene 已实现，不应出现在未实现表中"
        );
    }

    #[test]
    fn test_not_implemented_message_remains_available_for_future_entries() {
        let entry = DeferredCommand {
            command: "future-command",
            ci_gate: "gov §5.1 #future",
            owning_card: "TASK-999",
            reason: "future",
        };
        let message = not_implemented_message(&entry);
        assert!(message.contains(entry.owning_card));
        assert!(message.contains(entry.command));
        assert!(message.contains("尚未实现"));
    }

    #[test]
    fn test_deferred_hygiene_rules_are_empty_after_pl060() {
        assert!(
            DEFERRED_HYGIENE_RULES.is_empty(),
            "TASK-234 后 gov §5.4 的 13 条规则必须全部有实现；未实现表不得回流"
        );
    }

    #[test]
    fn test_not_implemented_message_covers_every_deferred_command() {
        for entry in DEFERRED_COMMANDS {
            let message = not_implemented_message(entry);
            assert!(
                message.contains(entry.command),
                "{} 的消息缺命令名",
                entry.command
            );
            assert!(
                message.contains(entry.owning_card),
                "{} 的消息缺归属卡号",
                entry.command
            );
        }
    }

    #[test]
    fn test_progress_note_states_partial_coverage() {
        let note = hygiene_progress_note(13).expect("13 条派生总数应可计算");
        assert!(
            note.contains("已实现 13 项"),
            "必须声明 13/13 已实现，实际：{note}"
        );
        assert!(
            note.contains("未实现 0 项"),
            "ADR-0068 / ADR-0069 落地后未实现项必须为 0，实际：{note}"
        );
    }

    #[test]
    fn test_describe_functions_list_every_entry() {
        let commands = describe_deferred_commands();
        assert!(
            commands.contains("未实现的子命令：0 项"),
            "清单必须显式说明 replay 完整版后为 0 项：{commands}"
        );
        let rules = describe_deferred_rules(2).expect("2 条派生总数应可计算");
        for entry in DEFERRED_HYGIENE_RULES {
            assert!(rules.contains(entry.rule), "清单遗漏规则 {}", entry.rule);
        }
        assert!(
            rules.contains("2 项卫生规则已全部实现"),
            "派生总数必须进入输出：{rules}"
        );
    }
}
