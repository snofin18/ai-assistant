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
//! 1. `IMPLEMENTED_HYGIENE_RULE_COUNT + DEFERRED_HYGIENE_RULES.len() == TOTAL_HYGIENE_RULE_COUNT`
//!    （有单测锁死；不一致说明有人加了规则却没更新登记）。
//! 2. 每个 `DeferredCommand::command` 在 `DEFERRED_COMMANDS` 中唯一。
//! 3. 每一项都必须有非空的 `owning_card`（可以是"未分配"，但不能留空 ——
//!    留空意味着没人负责，那才是真正的静默失败）。

/// gov §5.4 表格中的卫生规则总项数（**13 项**，口径由 ADR-0025 统一）。
///
/// 这个数字必须与 gov §5.4 的表格行数一致；不一致由下面的不变量 1 单测拦不住
/// （单测只校验"已实现 + 未实现 == 总数"的自洽性，不校验与文档的一致性）。
/// 因此改 gov §5.4 的行数时**必须**同步改这里 —— 让工具直接解析文档表格行数
/// 是更彻底的做法，已记入 `docs/PARKING_LOT.md` PL-022。
pub const TOTAL_HYGIENE_RULE_COUNT: usize = 13;

/// 已实现的卫生规则项数：TASK-001 / TASK-085 / TASK-086 / TASK-234 已覆盖全部 13 条。
pub const IMPLEMENTED_HYGIENE_RULE_COUNT: usize = 13;

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
/// 保留数组与类型是让不变量 1 继续机器校验「已实现数 + 未实现数 == 13」；
/// 未来新增规则时先登记未实现项，再实现并清空。
pub const DEFERRED_HYGIENE_RULES: &[DeferredRule] = &[];

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
#[must_use]
pub fn describe_deferred_rules() -> String {
    if DEFERRED_HYGIENE_RULES.is_empty() {
        return format!(
            "gov §5.4 的 {TOTAL_HYGIENE_RULE_COUNT} 项卫生规则已全部实现：未实现 0 项。\n"
        );
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
    format!(
        "gov §5.4 的 {TOTAL_HYGIENE_RULE_COUNT} 项卫生规则中，未实现 {} 项：\n{}\n",
        DEFERRED_HYGIENE_RULES.len(),
        rows.join("\n")
    )
}

/// 生成一行「进度声明」，在每次 `hygiene` 运行时打印。
///
/// 为什么每次都要打印：如果不声明，`verdict: PASSED` 会被误读成"13 项卫生规则全过"。
/// 让工具主动承认自己只检查了一部分，是防止虚假安全感的最低成本手段。
#[must_use]
pub fn hygiene_progress_note() -> String {
    format!(
        "-- deferred-rules: gov §5.4 共 {} 项，已实现 {} 项，未实现 {} 项（`--list-deferred` 查看清单）",
        TOTAL_HYGIENE_RULE_COUNT,
        IMPLEMENTED_HYGIENE_RULE_COUNT,
        DEFERRED_HYGIENE_RULES.len()
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_hygiene_rule_counts_are_consistent() {
        // 不变量 1：已实现 + 未实现 == 总数
        assert_eq!(
            IMPLEMENTED_HYGIENE_RULE_COUNT + DEFERRED_HYGIENE_RULES.len(),
            TOTAL_HYGIENE_RULE_COUNT,
            "规则登记表与 gov §5.4 的项数不一致"
        );
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
        let note = hygiene_progress_note();
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
        let rules = describe_deferred_rules();
        for entry in DEFERRED_HYGIENE_RULES {
            assert!(rules.contains(entry.rule), "清单遗漏规则 {}", entry.rule);
        }
    }
}
