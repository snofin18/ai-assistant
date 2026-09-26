# ADR-0051　去夜间化改名（路径 + 运行期标识）

状态：**Accepted**（2026-09-26，人类 chat：「PL-089 可以改名」）　日期：2026-09-26　Supersedes：—　Superseded by：—
关联：`docs/PARKING_LOT.md` PL-089、**ADR-0050** D1（本 ADR 补完其「路径 / 目录 / 锁名 / 分支名本次不改」）、**ADR-0039 D5**（README 冻结段例外）、**ADR-0021 / ADR-0030**（MEMORY.md 路由表例外）、**ADR-0031**（任务卡正文只读）、**ADR-0029 D4**（回退清单正文不改）、ADR-0028（guard 锁）

## 背景（为什么现在要决定）

ADR-0050 把**规范文字**时间中性化了（「当夜」= 运行日、「晨间报告」= 阶段报告），但**路径 / 目录 / 锁名 / 分支名**仍是历史命名 —— `docs/overnight-automation-charter.md`、`docs/nightly/`、`.nightly.lock`、`nightly/<date>`。读的人（尤其开源后的外部读者）仍会以为自动化只能在夜里跑。人类 2026-09-26 授权改名（`docs/PARKING_LOT.md` PL-089）。

## 决策（一句话）

把这四个**历史命名**改成时间中性的名字；**只改「现行文档 + 运行期标识」，不回溯改写只追加 / 只读的历史记录**。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | **文件改名**：`docs/overnight-automation-charter.md` → **`docs/automation-charter.md`** |
| **D2** | **目录改名**：`docs/nightly/` → **`docs/automations/`**（含其下 `codex-automations-operations.md` / `scheduler-acceptance-test.md` / `2026-09-25-report.md` / `.gitkeep`） |
| **D3** | **锁名改名**：`.nightly.lock` → **`.automation.lock`**（章程 §11.2 / G8、手册 §4.1.c、`.gitignore`）。**过渡期**：`.gitignore` **同时**保留 `.nightly.lock` —— 历史 automation 的 prompt 仍可能创建旧名（2026-09-26 14:00 的 PL-084 探针 prompt 用的就是旧名），双忽略避免它变成未跟踪文件 |
| **D4** | **分支名改名**：`nightly/<date>` → **`automation/<date>`**（章程 §0 / G4 / §11.4；**仅约定**，无代码强制） |
| **D5** | **不回溯改写**（旧引用 = 历史事实，保留旧名）：`docs/adr/*.md`（ADR 只增不改）、`LEDGER.md`（只追加）、`tasks/*.md`（正文只读）、`docs/spike-reports/*`（历史报告）、`docs/memory/*` 既有条目（只追加）、各文件「变更历史」行、`docs/automations/2026-09-25-report.md`、`docs/automations/scheduler-acceptance-test.md`（ADR-0029 D4「正文不改」） |
| **D6** | **授权例外（只改路径，其余一字不动）**：`README.md` 的「工程元层」表行（L35）与「路由」表两行（L62 / L63）；`MEMORY.md` 的路由 / 快照两行（L81 / L89）。依据 = 本 ADR 对 ADR-0039 D5 与 ADR-0021 的**当次例外** |
| **D7** | **授权同步**：`docs/automation-charter.md`（自身：正文标识符 + 头部「原名」注记 + §12 追加 v1.15 行）、`docs/automations/codex-automations-operations.md`（自身：正文标识符 + 头部注记 + §9 追加 v1.12 行）、`docs/subagent-orchestration.md`（L274 / L297）、`.gitignore`、`xtask/src/guard.rs` 的**模块文档注释**（仅注释，无逻辑改动） |
| **D8** | **旧引用不算缺陷**：`refscan` 只查 ADR 编号 / 范围写法 / `.ps1` 非 ASCII，**不查路径存在性**；`hygiene` / `docscan` / `card-check` / `memory-counts` / `adr-index` / `check-ledger` 均**不硬编码**这四个名字（已核） |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 不改名 | ❌ 否决 | 与 ADR-0050 D1 的「时间中性」目标冲突 |
| 2 | **改名 + 只改现行文档（本 ADR）** | ✅ **采纳** | 目标达成；不违反「只追加 / 只读」 |
| 3 | 全量改名（连 ADR / LEDGER / 任务卡 / 历史报告一起改） | ❌ 否决 | 同时违反「ADR 只增不改」「LEDGER 只追加」「任务卡正文只读」三条规则 |
| 4 | 在旧路径留重定向 stub | ❌ 否决 | 旧名继续存在 = 改名没完成；且 stub 自身又会成为新的「旧名」来源 |

## 影响（需要改的文档）

- **改名**：1 个文件 + 1 个目录（`git mv`，保留 `--follow` 历史）
- **改内容**：`README.md`(3 行)、`MEMORY.md`(2 行)、`docs/subagent-orchestration.md`(2 行)、`.gitignore`、`xtask/src/guard.rs`(注释)、两个被改名文件自身
- **追加**：`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,decisions}.md`、`docs/adr/README.md`（登记 0051 + 下一个可用号 0052）
- **不改**：所有 ADR 正文、LEDGER 既有行、任务卡、历史报告、memory 既有条目、各文件变更历史行

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 旧引用「找不到文件」 | D5 / D8 明确「历史记录用旧名是有意为之」；两个改名文件头部写「原名」注记 |
| 锁名切换期两个名字并存 | D3：`.gitignore` 同时忽略两个名字；章程 §11.2 改为 `.automation.lock` 并注明旧名 |
| 有工具硬编码旧路径 | D8：已核 6 个文档门禁，均不硬编码 |
| 越权改 README / MEMORY | D6：只改路径，其余不动；改动记入 `LEDGER.md` |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `git log --follow docs/automation-charter.md` 能追到改名前的历史；
2. 六个文档门禁全绿（`hygiene` / `docscan` / `card-check` / `memory-counts` / `adr-index` / `check-ledger`），`refscan` **不新增** baseline；
3. `docs/` 下不再有**现行文档**指向 `docs/nightly/` 或 `overnight-automation-charter`（历史记录除外）；
4. **何时重新评估**：若将来要做「连历史记录一起改」（选项 3），需新 ADR，且要一并处理 ADR-0031 与「只追加」铁律。
