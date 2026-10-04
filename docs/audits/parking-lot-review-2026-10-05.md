# 停车位存量复核报告（2026-10-05）

> 执行者：TASK-239　范围：`docs/PARKING_LOT.md` 截至 2026-10-05 仍开放或存在“部分关闭”余项的条目。
>
> 本报告只记录复核结论，不改写停车位历史行。机器命令均在本轮 `origin/main` 同步后执行；
> 无法机器验证的条目标注“需人工”，不得当作已关闭。

## 复核口径

- **已闭环（2026-10-05，本轮复核确认）**：触发条件已由现有文件、ADR、任务卡或门禁实现满足。
- **已被后续 ADR/卡取代，无需再做**：原问题被后续决策替换，继续按原文处理会偏离当前契约。
- **复核保留（2026-10-05）**：仍有明确未完成面；需要后续治理卡、ADR 或人类裁决。

## 仍开放条目

| 编号 | 原状态 | 复核结论 | 证据命令 | 结果 | 备注 |
|---|---|---|---|---|---|
| PL-004 | 待评审 | 复核保留（2026-10-05） | `git grep -n "banned-comment-tag" -- xtask/src/hygiene.rs xtask/src/comments.rs` | 机器验证：注释标签规则仍直接扫描文档注释，没有反引号豁免分支。 | 需要裁决“规则说明可否引用标签字面量”；需人工。 |
| PL-005 | 待评审 | 复核保留（2026-10-05） | `Test-Path docs/OPEN_SOURCE_CHECKLIST.md` | 机器验证：`False`。 | 开源前补齐；需 Orchestrator。 |
| PL-007 | 待评审 | 复核保留（2026-10-05） | `Test-Path .github/workflows/.gitkeep` | 机器验证：`True`。 | 冗余文件仍在；低风险清理。 |
| PL-009 | 仍待评审，但风险已降低 | 复核保留（2026-10-05） | `Test-Path cross-platform-ai-assistant-architecture.md`；`Test-Path docs/history` | 机器验证：v1 文件 `True`，`docs/history` `False`。 | 需要决定是否迁移 v1 并加 superseded 横幅；需人工。 |
| PL-010 | 待评审 | 复核保留（2026-10-05） | `Select-String -Path docs/governance-ai-agent-execution.md -Pattern "pending"` | 机器验证：§9.2 LEDGER 模板仍无 commit 占位约定。 | 需要 Orchestrator 改模板；仍开放。 |
| PL-014 | 待评审 | 复核保留（2026-10-05） | `git grep -n "model_context_window" -- docs/memory/facts.md` | 机器验证：已有本机环境记录，但仓库无法证明当前端点元数据问题已消失。 | 环境侧事项；需人工复测。 |
| PL-019 | 已关闭（采纳方案 ①，方案 ② 仍归 TASK-015） | 复核保留（2026-10-05） | `Get-Content xtask/src/hygiene_io.rs -Raw` | 机器验证：依赖登记扫描明确排除 `spikes/`、`fixtures/`、`tools/`；方案 ② 未实现。 | 仍需决定是否覆盖 spike manifest；需人工。 |
| PL-022 | 已关闭（但正文承认 ② 未实现） | 复核保留（2026-10-05） | `git grep -n "TOTAL_HYGIENE_RULE_COUNT" -- xtask/src/deferred.rs` | 机器验证：常量仍硬编码为 13；gov 清单与 CI 步骤数未派生校验。 | 这是“表面关闭、剩余项未落”的典型；需后续治理卡。 |
| PL-024 | 待评审 | 复核保留（2026-10-05） | `git grep -n "#36315" -- docs/subagent-orchestration.md docs/memory/open.md` | 机器验证：仓库仍记录人工建会话为强制前置。 | 上游 issue 状态无法由仓库证明；需人工核验。 |
| PL-025 | 待评审 | 复核保留（2026-10-05） | `Get-Content docs/governance-ai-agent-execution.md` 统计 `- [x]` / `- [ ]` | 机器验证：`checked=5 unchecked=27`。 | 清单仍与事实脱节，且未明确勾选责任人。 |
| PL-027 | 待后续卡 / 待裁决 | 复核保留（2026-10-05） | `cargo run -p xtask -- hygiene` | 机器验证：仍有 10 条 `hygiene/missing-final-newline` Warning。 | 清扫或定义生成物例外后才能升 Error；需后续卡。 |
| PL-035 | 待评审 | 复核保留（2026-10-05） | `(Get-Content AGENTS.md).Count`；`git grep -n "实际 185 行" -- docs/subagent-orchestration.md` | 机器验证：`AGENTS.md` 实测 250 行；文档仍写 185 行。 | 派生值仍有残留；需后续文档清扫。 |
| PL-040 | 待评审 | 复核保留（2026-10-05） | `Test-Path docs/spec/README.md` | 机器验证：`False`。 | 是否需要 spec 索引仍待决定；需人工。 |
| PL-042 | 待评审（接受显式失败现状） | 复核保留（2026-10-05） | `git grep -n "separate_db_full\|UnsupportedDurability" -- crates/audit crates/storage` | 机器验证：该档位仍显式失败，未实现独立审计库。 | 触发条件 = 合规场景；需独立 ADR/卡。 |
| PL-044 | 待评审（接受残余风险） | 复核保留（2026-10-05） | `git grep -n "外部锚点\|已知限制" -- crates/audit/README.md` | 机器验证：审计链仍没有外部锚点。 | 触发条件 = 合规/无人值守；需人类接受风险。 |
| PL-049 | 待评审 | 复核保留（2026-10-05） | `git grep -n "secret\\." -- protocol/audit-event/audit-event-1.0.json crates/secrets` | 机器验证：审计 schema 仍无 `secret.*`，secrets README 仍明确标注未落库。 | 需要 ADR/schema 扩展；仍开放。 |
| PL-050 | 待评审 | 复核保留（2026-10-05） | `git grep -n "thiserror" -- Cargo.toml crates docs/DEPENDENCIES.md` | 机器验证：部分 crate 用 `thiserror`，storage/audit/secrets 仍手写错误。 | 需要统一方案或改 spec；需 ADR。 |
| PL-052 | 待评审 | 复核保留（2026-10-05） | `Get-Content PLAN.md -TotalCount 6` | 机器验证：文件头仍写“agent 不得修改本文件，只能提案”。 | 与 ADR-0039/0041 的可写面冲突；需 Orchestrator。 |
| PL-055 | 待评审 | 复核保留（2026-10-05） | `cargo run -p xtask -- docscan` | 机器验证：`0 error(s), 342 warning(s)`。 | docscan Warning 存量与豁免判据仍未收口；需后续治理卡。 |
| PL-057 | 待评审 | 复核保留（2026-10-05） | `git grep -n "docs/nightly/logs" -- docs` | 机器验证：ADR / 验收文档仍保留旧日志路径引用。 | 需要决定是否为回退方案专用；需人工。 |
| PL-063 | 待评审 | 复核保留（2026-10-05） | `git grep -n "TASK-083\|TASK-084\|TASK-100\|TASK-101" -- plans PLAN.md MEMORY.md` | 机器验证：plans/PLAN 无登记，仅 MEMORY 快照提到 TASK-083。 | 卡片登记结构仍未裁决；需 Orchestrator。 |
| PL-071 | 待评审 | 复核保留（2026-10-05） | `git grep -n "unsafe_code" -- docs/adr/0035-workspace-lint-policy-no-exceptions.md` | 机器验证：ADR-0035 baseline 未登记 crate 级 `unsafe_code` allow。 | 需要治理清扫；仍开放。 |
| PL-072 | 待评审 | 复核保留（2026-10-05） | `git grep -n "TitleRegex\|NameRegex" -- docs/spec/naming.md crates/platform/api/src/target.rs` | 机器验证：代码仍用 `*Regex` 名称，naming spec 未登记子串匹配语义。 | 需命名契约裁决；仍开放。 |
| PL-073 | 待评审 | 复核保留（2026-10-05） | `Get-Content AGENTS.md` §8；`Get-Content docs/adr/0031-task-card-one-file-per-card.md` 的正文/记录区分界规则 | 人工判断：AGENTS §8 仍写正文区只读，状态行例外未被明文授权。 | 需要 ADR/规则文本收口；状态行仍需本批人工处理。 |
| PL-075 | 待评审 | 复核保留（2026-10-05） | `git grep -n "栈式 PR\|baseRefName" -- docs` | 机器验证：仓库没有栈式 PR base 切换的硬规则，只有 ADR-0039 的冲突面讨论。 | 需 Orchestrator/ADR 定流程；仍开放。 |
| PL-078 | 待评审 | 复核保留（2026-10-05） | `git grep -n "pub fn new\|struct Source\|struct Truncation" -- crates/protocol/src/generated` | 机器验证：生成结构体仍缺统一构造器。 | 需要 codegen 治理卡；仍开放。 |
| PL-079 | 待评审 | 复核保留（2026-10-05） | `git grep -n "pub trait Clock" -- crates` | 机器验证：`storage` 与 `tool-bus` 仍各有 `Clock` trait。 | 需要共同时间 crate/ADR；仍开放。 |
| PL-080 | 待评审 | 复核保留（2026-10-05） | `git grep -n "toolset.list\|<app>.<domain>.<action>" -- docs/spec/tool-schema.md crates/tool-bus/src` | 机器验证：spec 仍要求三段式，代码仍使用两段式元工具名。 | 必须在真实 Adapter 前裁决；仍开放。 |
| PL-083 | 待评审 | 复核保留（2026-10-05） | `git grep -n "update_task" -- crates apps` | 机器验证：无 `update_task` / latest task projection。 | storage 任务行仍停在创建时状态；需立卡。 |
| PL-087 | 待评审（复发项已缓解） | 复核保留（2026-10-05） | `git grep -n "≤3 轮\|阶段报告\|自动链式" -- docs/automation-charter.md docs/adr/0046-automation-one-shot-instruction-driven.md` | 机器验证：章程仍保留轮数/阶段报告规则，且 ADR-0046 已改变任务形态。 | 需要决定哪些规则适用于一次性任务；需 ADR/人类。 |
| PL-088 | 待裁决 | 复核保留（2026-10-05） | `git grep -n "自动链式\|计数器" -- docs/adr/0046-automation-one-shot-instruction-driven.md docs/PARKING_LOT.md` | 机器验证：ADR-0046 D1/选项 4 仍否决自动链式续建。 | 若采纳有界链，必须先新 ADR；需人类。 |
| PL-089 | 已闭环（措辞）；前缀 / 脚本名待另案 | 复核保留（2026-10-05） | `git grep -n "nightly-probe-\|scripts/nightly-run.ps1" -- docs` | 机器验证：旧前缀仍在 GATE-0 文档，旧脚本名仍在回退验收文档。 | 需要决定回退方案命名是否保留；需人工。 |
| PL-091 | 待评审 | 复核保留（2026-10-05） | `git grep -n "锁被占用" -- docs/automation-charter.md` | 机器验证：章程仍规定锁被占用直接结束，没有 3 分钟/30 分钟有界等待。 | 需决定是否采纳有界等待；需人工。 |
| PL-098 | 待裁决 | 复核保留（2026-10-05） | `git grep -n "unwrap_used\|expect_used\|panic" -- docs/spec/testing.md` | 机器验证：testing.md 仍缺三项测试允许清单。 | 需 spec 补丁或立卡；仍开放。 |
| PL-099 | 待后续卡 / 待裁决 | 复核保留（2026-10-05） | `Get-Content xtask/src/refscan.rs` 的 `ADR_BARE_PENDING`；`Get-ChildItem docs/adr` | 机器验证：0016/0017/0020/0027 仍是常量待建号，但对应 Draft 文件已存在。 | 需要动态登记表或移除常量；仍开放。 |

## 本轮复核确认已闭环 / 已取代

| 编号 | 原状态 | 复核结论 | 证据命令 | 结果 | 备注 |
|---|---|---|---|---|---|
| PL-002 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `cargo run -p xtask -- check-comments` | 机器验证：规则覆盖 8/8，命令 exit 0。 | 后续 TASK-087 已补上护栏。 |
| PL-003 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `Select-String -Path docs/spec/testing.md -Pattern "### 4.3"` | 机器验证：§4.3 已存在。 | TASK-200 已补节。 |
| PL-006 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `cargo deny --version`；`Test-Path docs/dev-env-setup.md` | 机器验证：`cargo-deny 0.20.2`，清单文件存在。 | 本地验收与 CI 已等价。 |
| PL-008 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `git config --get user.name`；`git config --get user.email` | 机器验证：`snofin18 (via Codex)` / `snofin@gmail.com`。 | 已替换占位身份。 |
| PL-012 | 待评审 | 已被后续 ADR/卡取代，无需再做 | `cargo run -p xtask -- adr-index`；`Get-Content docs/automation-charter.md` | 机器验证：ADR-0018 被 ADR-0029 取代，章程已改写。 | 原 heartbeat 兼容性问题不再是当前方案。 |
| PL-013 | 待评审 | 已被后续 ADR/卡取代，无需再做 | `cargo run -p xtask -- adr-index`；`Get-Content docs/automation-charter.md` | 机器验证：ADR-0018 被 ADR-0029 取代，章程已改写。 | 原 cron 建议已被新机制处理。 |
| PL-015 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `git grep -n "heartbeat" -- MEMORY.md docs/memory` | 机器验证：旧快照口径已被分层记忆和后续 supersedes 条目处理。 | ADR-0018/0029 已替代原决策。 |
| PL-016 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `git grep -n "cargo-deny-action@v2.1.1\|cargo deny check" -- .github/workflows docs/dev-env-setup.md` | 机器验证：CI action 已钉版本，清单含本地命令。 | 原 argv/version 漂移已修。 |
| PL-021 | 待评审 | 已被后续机制取代，无需再做 | `Get-ChildItem -Filter Cargo.toml -Recurse` 检查 `license.workspace` | 机器验证：工作区成员统一继承 license，排除目录显式带 license。 | 不需要新增第 14 条 hygiene 规则。 |
| PL-023 | 待评审 | 已被后续 ADR/卡取代，无需再做 | `Test-Path docs/nightly/logs`；`Test-Path scripts/nightly-run.ps1`；`Get-Content docs/adr/0054-automation-run-evidence-landing.md` | 机器验证：旧路径/脚本均不存在；ADR-0054 已把自动化产物改为 PR 前落地。 | 回退方案若要启用应新开条目。 |
| PL-041 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `git grep -n "envelope_id\|event_id\|Uuid" -- docs/spec/envelope.md docs/spec/audit-event.md docs/spec/naming.md` | 机器验证：契约使用 UUID 与 newtype，不存在“所有 id 都是 u64”的现行声明。 | TASK-200 删除误导性通用声明。 |
| PL-061 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `cargo run -p xtask -- hygiene`；`cargo run -p xtask -- --list-deferred` | 机器验证：文本文件规则已覆盖非 `.md` 文件，13/13 卫生规则已实现。 | TASK-086 已补扫描范围。 |
| PL-093 | 待评审 | 已闭环（2026-10-05，本轮复核确认） | `Get-Content apps/desktop-ui/package.json`；`git grep -n "pnpm test" -- .github/workflows/ci.yml` | 机器验证：`vitest` + testing-library + jsdom + `pnpm test` 已在 package/CI 中。 | 后续 TASK-210/213 已补 UI 测试基础设施。 |

## 已由既有追加行闭环的条目

以下编号已有明确的关闭/闭环追加行，且正文复核未发现会影响当前事实的剩余项，本轮不重复判定：

`PL-001`、`PL-011`、`PL-017`、`PL-018`、`PL-020`、`PL-026`、`PL-028`、`PL-029`、`PL-030`、`PL-031`、`PL-032`、`PL-033`、`PL-034`、`PL-036`、`PL-037`、`PL-038`、`PL-039`、`PL-043`、`PL-045`、`PL-046`、`PL-047`、`PL-048`、`PL-051`、`PL-056`、`PL-058`、`PL-059`、`PL-060`、`PL-062`、`PL-064`、`PL-065`、`PL-066`、`PL-067`、`PL-068`、`PL-069`、`PL-070`、`PL-074`、`PL-076`、`PL-077`、`PL-081`、`PL-082`、`PL-084`、`PL-085`、`PL-086`、`PL-090`、`PL-092`、`PL-094`、`PL-095`、`PL-096`、`PL-097`、`PL-100`、`PL-101`、`PL-102`、`PL-103`、`PL-104`、`PL-105`、`PL-106`、`PL-107`、`PL-108`。

`PL-019`、`PL-022`、`PL-040`、`PL-042`、`PL-055` 虽有历史关闭字面或阶段性结论，但因正文剩余项仍在，已列入上方开放表，不在此列。

## 结论与下一手

- 本轮可补记闭环/取代的条目：`PL-002`、`PL-003`、`PL-006`、`PL-008`、`PL-012`、`PL-013`、`PL-015`、`PL-016`、`PL-021`、`PL-023`、`PL-041`、`PL-061`、`PL-093`。
- 仍需后续治理的条目：`PL-004`、`PL-005`、`PL-007`、`PL-009`、`PL-010`、`PL-014`、`PL-019`、`PL-022`、`PL-024`、`PL-025`、`PL-027`、`PL-035`、`PL-040`、`PL-042`、`PL-044`、`PL-049`、`PL-050`、`PL-052`、`PL-055`、`PL-057`、`PL-063`、`PL-071`、`PL-072`、`PL-073`、`PL-075`、`PL-078`、`PL-079`、`PL-080`、`PL-083`、`PL-087`、`PL-088`、`PL-089`、`PL-091`、`PL-098`、`PL-099`。
- 机器可验证项已在本轮重跑；`PL-004`、`PL-009`、`PL-014`、`PL-024`、`PL-042`、`PL-044`、`PL-052`、`PL-057`、`PL-073`、`PL-075`、`PL-087`、`PL-088`、`PL-089`、`PL-091` 含人工判断或需要人类裁决。
