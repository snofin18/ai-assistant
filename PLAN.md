# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-09-24（TASK-203 收尾：ADR-0039 状态同步契约落地 + PL-043/PL-045 闭环；详 LEDGER 末行）
当前阶段    ：**阶段 1（三试点闭环）** —— stage-0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）
当前任务卡  ：**1a 批次 A1（地基层）**：TASK-011 ✅ / 012 ✅ / 013 ✅ Done → **TASK-014（secrets / OS keychain）= 下一张**；
                  同批 TASK-015 已由 Orchestrator 展开正文（docscan 4 条结构规则 + arch test + PL-047 + ADR-0039 的 check-ledger）；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203** 均 Done
阻塞项      ：① PL-037 已闭环（TASK-201）→ TASK-015 的 arch 宿主已就位；② TASK-002 仍 Blocked（上游 `create_thread` 未关）
下一步动作  ：① TASK-014（`crates/secrets`）→ ② TASK-015 护栏清扫（含 PL-047 + ADR-0039 D3）
                  → ③ stage-1 后续批次；详见 `plans/stage-1-pilots.md` 与 `MEMORY.md` §1（派生值一律不写进本文件，ADR-0030 D2）
```

## 阶段索引（点开当前阶段那一个就够）

| 阶段 | 名称 | 周期 | 详情文件 | 状态 |
|---|---|---|---|---|
| 0 | Spike 技术验证（A/B/C/E/F/G/H + D-lite） | 2~3 周（实际 4 天） | `plans/stage-0-spikes.md` | ✅ closeout 2026-09-20 |
| 1 | 三试点闭环：Notepad → Paint → Edge/Chrome | 10~12 周 | `plans/stage-1-pilots.md` | **当前** |
| 2 | Excel（L1 COM）+ Adapter 抽象正式化 | 6~8 周 | `plans/stage-2-excel.md`（待生成） | 未开始 |
| 3 | Photoshop（纯脚本型）+ 评测体系 | 6~8 周 | `plans/stage-3-photoshop.md`（待生成） | 未开始 |
| 4 | 扩量 + 股票只读 + 图表面 | 持续 | `plans/stage-4-scale.md`（待生成） | 未开始 |
| 5~7 | macOS / Linux（Wayland-first）/ 开放生态·无人值守·交易闸门 | — | 待生成 | 未开始 |

全局工作分解与依赖顺序见 `docs/wbs-overview.md`。

## 任务队列（当前阶段）

见 `plans/stage-1-pilots.md` 的「批次表」与「卡片正文的位置」表。**一张卡一个 agent 一个会话**，write scope 不得重叠，并行度 ≤ 3。

## 范围冻结提示

每个阶段的 `plans/*.md` 都写明 **In scope** 与 **Out of scope**。**Out of scope 清单的效力高于 In scope**：写了 Out of scope 的东西即使"顺手能做"也不许做，发现必要性 → 记 `docs/PARKING_LOT.md` 一行。

## 关联文件

| 文件 | 作用 |
|---|---|
| `AGENTS.md` | Agent 工作契约（铁律、协议、规范速查） |
| `MEMORY.md` | 长期记忆：已知事实 / 已否决方案 / 踩坑（**动手前必读 §2 §4 §5**） |
| `LEDGER.md` | 执行台账（只追加） |
| `docs/PARKING_LOT.md` | 跑题想法停车场（阶段末评审处置） |
| `docs/DEPENDENCIES.md` | 依赖登记（先登记后引入） |
| `docs/adr/` | 决策记录（编号登记表见 `docs/adr/README.md`） |
| `tasks/` | 任务卡（一卡一文件，ADR-0031） |

## 变更历史

| 日期 | 变更 | 依据 | 批准 |
|---|---|---|---|
| 2026-09-16 | 建立本文件；阶段划分改为「先 Windows 纵深（Notepad→Paint→Edge→Excel→Photoshop）后跨平台横扩」 | 架构 v2.2 §20.2/§20.7、feasibility v1.1 §3.0/§4 | 项目负责人 |
| 2026-09-16 | PLAN.md 拆分为「索引 + 按阶段文件」，支持最小化读取 | 项目负责人要求（第 8 点） | 项目负责人 |
| 2026-09-24 | 阶段 0 → 阶段 1（stage-0 已 closeout）；下一步动作重写；删掉 HEAD 等派生值；修「关联文件」4 处「待创建」 | 用户 chat 2026-09-24 + LEDGER | 人类（chat 2026-09-24，授权 Orchestrator 代行） |
