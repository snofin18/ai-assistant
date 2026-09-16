# PARKING_LOT.md — 停车位（不相关但值得记住的事）

> **用途**：执行任务卡时发现"这也有问题"，但**不属于本卡范围** —— 一行记在这里，本卡不动手。
> 这是 gov §3.3 禁止 drive-by refactor 的配套设施：没有停车位，agent 只能选择
> "顺手改掉"（漂移）或"假装没看见"（丢失信息），两者都更糟。
> **处置时机**：阶段末评审（gov §7.2）。模板见 gov §9.5。

| 日期 | 提出者 | 事项 | 相关卡 | 处置（阶段末评审） |
|---|---|---|---|---|
| 2026-09-16 | TASK-001 | PL-001 `gov §5.4` 表格实际是 **11 行**，但 `plans/stage-1-pilots.md` 的 TASK-015 DoD 写"hygiene **12** 项检查全部生效"。数字不一致，需人类裁决以哪个为准（`plans/*` 不在 TASK-001 write scope 内，故未改） | TASK-015 | 待评审 |
| 2026-09-16 | TASK-001 | PL-002 `check-comments` / `check-ledger` / `card-check` 三个护栏子命令**没有任何任务卡认领**（gov §5.1 的 #15 #16 门禁因此无人负责）。需补卡或并入 TASK-015 | TASK-015 | 待评审 |
| 2026-09-16 | TASK-001 | PL-003 `xtask/src/report.rs` 的模块注释引用了 `docs/spec/testing.md` §4.3，但该 spec 属于"后续卡写"，目前不存在（悬空引用）。写 spec 的卡需要回填这一节 | 写 docs/spec/testing.md 的卡 | 待评审 |
| 2026-09-16 | TASK-001 | PL-004 「文档注释中用反引号引用的标签字样」（例如在 `///` 里解释什么是被禁标签）是否应豁免 `hygiene/banned-comment-tag` 与 `hygiene/missing-card-reference`？当前实现**不豁免**，因此写规则说明时必须绕开字面量。TASK-001 采取"改措辞"绕过，未改规则语义 | TASK-015（check-comments 设计） | 待评审 |
| 2026-09-16 | TASK-001 | PL-005 `docs/OPEN_SOURCE_CHECKLIST.md` 在 gov §10 清单里被要求，但不在 TASK-001 的 write scope 内，故未创建。开源前（M5 之后）需要补 | 开源准备 | 待评审 |
| 2026-09-16 | TASK-001 | PL-006 `cargo deny` 未在本机安装，CI 里装了但本地跑不了 → 本地验收与 CI 验收不等价。建议在 TASK-015 前把 `cargo-deny` 写进开发环境准备清单 | TASK-015 | 待评审 |
| 2026-09-16 | TASK-001 | PL-007 `.github/workflows/.gitkeep` 在 `ci.yml` 就位后已冗余，可删（未删：删除文件不在本卡必要动作内） | — | 待评审 |
| 2026-09-16 | TASK-001 | PL-008 git 提交身份目前是**仓库本地占位**（`Codex (ai-assistant agent)` / `codex@localhost.invalid`），本机没有任何 global gitconfig。推 GitHub 前必须设成真实身份，否则历史里全是占位作者 | 开源准备 | 待评审 |
| 2026-09-16 | TASK-001 | PL-009 仓库根同时存在 `cross-platform-ai-assistant-architecture.md`（v1 原始规划）与 `...-v2.md`（SSOT）。AGENTS.md 只指向 v2，但两者并排放在根目录容易让新 agent 读错版本。建议把 v1 移到 `docs/history/` 并加"已被 v2 取代"横幅（v1 不在任何卡的 write scope 内，故未动） | — | 待评审 |
| 2026-09-16 | TASK-001 | PL-010 `docs/governance-ai-agent-execution.md` §9.2 的 LEDGER 模板没有"commit 哈希在建库前无法预知"的处理约定，导致首条台账只能先写占位再回填（违反"只追加不改写"的字面要求）。建议在模板里明确：commit 列允许写 `pending`，并在下一次提交中**追加一行**回填而非改写 | TASK-015（card-check） | 待评审 |
| 2026-09-16 | TASK-001 | PL-011 建议给 gov §5.4 增加第 12 项卫生规则：**文件不得含 CRLF**（`hygiene/crlf-line-endings`，Error 级）。本次就真实踩到了 —— 编辑脚本按旧约定写回 CRLF，`.gitattributes` 只能保证入库形态、管不住工作区，而 `cargo fmt --check` 会在 Linux runner 上因此变红。**顺带**：这可能正是 PL-001 里"hygiene 12 项"与"表格 11 行"数字不一致的来源，两条可一并裁决 | TASK-015 | 待评审 |