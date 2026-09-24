//! # 未实现项登记表（把「还没做」变成显式事实）
//!
//! 职责：登记 xtask 中**尚未实现的子命令**与**尚未实现的卫生规则**，
//! 并给出每一项的归属任务卡号；让工具在未实现时**显式失败**而不是静默返回成功。
//!
//! ## 为什么需要这张表
//! 铁律 1「无静默失败」在工具链上的具体形态是：CI 里一条 `cargo run -p xtask -- check-comments`
//! 如果因为"还没写"而返回 0，人类会以为这项检查一直在保护仓库 —— 这比没有这项检查更危险，
//! 因为它制造了虚假的安全感。所以未实现项必须：① 登记在册 ② 运行时报错并指出归属卡号
//! ③ 在正常运行的输出里声明"13 项规则只实现了 3 项"（口径见 ADR-0025）。
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

/// 尚未分配任务卡时的占位说明。
///
/// 这不是"留空"：它明确指出了**该去哪里补**（`docs/PARKING_LOT.md`），
/// 因此仍然满足不变量 3 的意图（有人类可执行的下一步）。
pub const UNASSIGNED_CARD: &str = "未分配（见 docs/PARKING_LOT.md PL-002，需人类补卡）";

/// gov §5.4 表格中的卫生规则总项数（**13 项**，口径由 ADR-0025 统一）。
///
/// 这个数字必须与 gov §5.4 的表格行数一致；不一致由下面的不变量 1 单测拦不住
/// （单测只校验"已实现 + 未实现 == 总数"的自洽性，不校验与文档的一致性）。
/// 因此改 gov §5.4 的行数时**必须**同步改这里 —— 让工具直接解析文档表格行数
/// 是更彻底的做法，已记入 `docs/PARKING_LOT.md` PL-022。
pub const TOTAL_HYGIENE_RULE_COUNT: usize = 13;

/// 本卡（TASK-001）已实现的卫生规则项数：文件行数、注释标签、注释掉的代码块。
pub const IMPLEMENTED_HYGIENE_RULE_COUNT: usize = 3;

/// 一个尚未实现的子命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredCommand {
    /// 子命令名（与 `xtask <name>` 一致）。
    pub command: &'static str,
    /// 它对应 gov §5.1 的哪一项 CI 门禁（用于说明"缺了它会漏掉什么"）。
    pub ci_gate: &'static str,
    /// 归属任务卡号；未拆卡时为 `UNASSIGNED_CARD`。
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
pub const DEFERRED_COMMANDS: &[DeferredCommand] = &[
    DeferredCommand {
        command: "replay-skeleton",
        ci_gate: "gov §5.1 #13",
        owning_card: "TASK-034",
        reason: "skeleton 已实现（TASK-015-pt3，dry-run 解析+校验）；真实 fixture + diff 留待 TASK-034 完整版",
    },
    DeferredCommand {
        command: "check-comments",
        ci_gate: "gov §5.1 #15",
        owning_card: UNASSIGNED_CARD,
        reason: "naming §10 的 8 条注释/命名规则尚未拆成任务卡",
    },
];

/// 未拆卡的卫生规则归属说明（PL-060：需要 ADR / 阈值设计前置，尚无任务卡）。
///
/// 这不是"留空"：它明确指出了**下一步去哪**（`docs/PARKING_LOT.md` PL-060），
/// 因此仍满足不变量 3 的意图（有人类可执行的下一步）。
pub const UNASSIGNED_HYGIENE_CARD: &str =
    "未拆卡（见 docs/PARKING_LOT.md PL-060：需 ADR / 阈值设计前置）";

/// 未实现的卫生规则清单（gov §5.4 共 13 项；TASK-001 实现 3 项，其余 10 项的归属见 PL-059）。
///
/// **2026-09-24 归属修正（PL-059）**：这 10 项原来一律写 `TASK-015`，而 TASK-015 已 Done
/// 且**没有**实现它们 —— 登记表指向一张已完成的卡，等于「有人会做」的信号消失
/// （与本模块头部反对的「静默失败」同型）。修正后按**实现机制**分三组：
/// - `TASK-085`：需要「函数与属性扫描器」的 5 项（函数行数 / 参数个数 / 圈复杂度 / STUB 标记 / `#[ignore]` 原因）；
/// - `TASK-086`：读文件 + 结构化比对的 3 项（CRLF / 末行换行 / 依赖登记）；
/// - `UNASSIGNED_HYGIENE_CARD`：需 ADR / 阈值设计前置的 2 项（重复代码相似度 / 顶层目录白名单）。
///
/// 分组轴是**实现机制**（同一扫描器的规则放一张卡），不是「谁提的」。
pub const DEFERRED_HYGIENE_RULES: &[DeferredRule] = &[
    DeferredRule {
        rule: "单函数行数 > 80 警告",
        reason: "需要函数边界扫描（rustscan 的降噪视图已就绪，扫描器待写）",
        owning_card: "TASK-085",
    },
    DeferredRule {
        rule: "函数参数个数 > 6 警告",
        reason: "同上，需要函数签名解析",
        owning_card: "TASK-085",
    },
    DeferredRule {
        rule: "圈复杂度 > 15 警告",
        reason: "需要分支计数，依赖函数体扫描",
        owning_card: "TASK-085",
    },
    DeferredRule {
        rule: "重复代码（跨文件相似度）警告",
        reason: "需要跨文件指纹与相似度阈值设计，属独立议题（PL-060：先裁决阈值口径）",
        owning_card: UNASSIGNED_HYGIENE_CARD,
    },
    DeferredRule {
        rule: "新增顶层目录必须在 ADR 白名单中",
        reason: "需要先有 ADR 白名单文件（docs/adr/ 下已有多份 ADR，但白名单本身尚未落地；另见 PL-023 的 scripts/ 归属）→ PL-060",
        owning_card: UNASSIGNED_HYGIENE_CARD,
    },
    DeferredRule {
        rule: "新增依赖必须已登记 docs/DEPENDENCIES.md",
        reason: "登记表已建（TASK-001），解析与比对逻辑待写",
        owning_card: "TASK-086",
    },
    DeferredRule {
        rule: "空实现 stub 必须带 STUB 卡号标记",
        reason: "「空实现」的判定需要函数体分析；卡号部分已由 hygiene/missing-card-reference 覆盖",
        owning_card: "TASK-085",
    },
    DeferredRule {
        rule: "被跳过的测试（#[ignore] / .skip）必须带原因与卡号",
        reason: "需要属性解析与测试函数关联",
        owning_card: "TASK-085",
    },
    // 以下两条由 ADR-0025 D1 新增（关闭 PL-011 / PL-020）。
    DeferredRule {
        rule: "文件不得含 CRLF（hygiene/crlf-line-endings）",
        reason: "需要先确定「文本文件」的判定方式（ADR-0025 D1 定为扩展名白名单）；Error 级，可直接上线（2026-09-24 实测全仓文本文件 CRLF = 0）。⚠ 现状只覆盖 .md（docscan 的 file/encoding），见 PL-061",
        owning_card: "TASK-086",
    },
    DeferredRule {
        rule: "必须以单个换行结尾（hygiene/missing-final-newline）",
        reason: "上线即为 Error 会当场红既有文件 → 必须先以 Warning 上线、清扫完再升 Error（ADR-0025 D1）。⚠ 2026-09-24 实测：`.md` 之外仍有 8 处（2 个 `crates/*/Cargo.toml` + 5 个 `protocol/*/*.json` + 1 个 `spikes/*.ps1`）—— PL-027 在 2026-09-18 清过一轮，**因为没有门禁又长回来了**（ADR-0030）",
        owning_card: "TASK-086",
    },
];

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
        "-- deferred-rules: gov §5.4 共 {} 项，已实现 {} 项，未实现 {} 项（归属 TASK-085 / TASK-086，未拆卡的见 PL-060；`--list-deferred` 查看清单）",
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
    fn test_find_command_returns_registered_entry() {
        // TASK-011 之后 codegen / verify-schemas 已实现并从表里移除，改用仍待实现的 replay-skeleton。
        let entry = find_command("replay-skeleton").expect("replay-skeleton 应在未实现表里");
        assert_eq!(entry.command, "replay-skeleton");
        assert!(
            !entry.owning_card.trim().is_empty(),
            "登记项必须写明归属卡号"
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
    fn test_not_implemented_message_names_the_owning_card() {
        let entry = find_command("replay-skeleton").expect("replay-skeleton 应已登记");
        let message = not_implemented_message(entry);
        // 断言绑定**登记表里的实际归属**，不硬编码卡号：硬编码会在「归属修正」
        // （PL-059 那种）时变成一条假红灯。这里同时把修正后的归属**锁死**。
        assert!(
            message.contains(entry.owning_card),
            "报错必须指出归属卡号（{}），实际：{message}",
            entry.owning_card
        );
        assert_eq!(
            entry.owning_card, "TASK-034",
            "replay 的完整版归 TASK-034（record-replay 框架），不是已 Done 的 TASK-015（PL-059）"
        );
        assert!(message.contains("尚未实现"));
        assert!(
            message.contains("replay-skeleton"),
            "报错必须回显命令名，便于在长日志里定位"
        );
    }

    #[test]
    fn test_deferred_hygiene_rules_only_point_at_the_two_owning_cards() {
        // PL-059 的机器判据（ADR-0019 N1 同型）：10 项未实现规则只许归
        // TASK-085 / TASK-086 / 未拆卡指针 —— 不许再出现「指向一张已 Done 的卡」
        // 那种形态（TASK-015 就是这样过期的）。
        let allowed = ["TASK-085", "TASK-086", UNASSIGNED_HYGIENE_CARD];
        for entry in DEFERRED_HYGIENE_RULES {
            assert!(
                allowed.contains(&entry.owning_card),
                "规则 `{}` 的归属 `{}` 不在允许集合里 —— 改归属时必须同时更新本测试（PL-059）",
                entry.rule,
                entry.owning_card
            );
        }
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
            note.contains("已实现 3 项"),
            "必须声明只实现了一部分，实际：{note}"
        );
        assert!(
            note.contains("未实现 10 项"),
            "ADR-0025：gov §5.4 为 13 项、已实现 3 项 → 未实现必须是 10 项，实际：{note}"
        );
    }

    #[test]
    fn test_describe_functions_list_every_entry() {
        let commands = describe_deferred_commands();
        for entry in DEFERRED_COMMANDS {
            assert!(
                commands.contains(entry.command),
                "清单遗漏命令 {}",
                entry.command
            );
        }
        let rules = describe_deferred_rules();
        for entry in DEFERRED_HYGIENE_RULES {
            assert!(rules.contains(entry.rule), "清单遗漏规则 {}", entry.rule);
        }
    }
}
