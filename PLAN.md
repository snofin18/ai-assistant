# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-10-03（**TASK-226 Done**：PL-104 / PL-105 闭环，ignored 真机用例并行风险以异步互斥消除，`production_root.rs` 拆分后 3 个文件均 <600 行；**TASK-225 Done**：Host 装配层接上合成输入目标租约独占，两个任务冲突稳定 `Transient`、释放后重试、失败回基线与 pointer-shaped 专项 5 passed，`PL-101` 闭环；**TASK-223 Done**：ADR-0064 文件通道 + ADR-0065 前置初始指纹步骤落地，T1.1 大文件 Plan 在 fake 平台与真机 UIA 各取证跑通，`DRIFT-223-1` / `PL-100` 闭环；**TASK-220 Done**：资源泄露四类防护（元素表 / anchor / grant 上限与淘汰 + 进程树终止与 reap）经真机 ignored 全套 8 passed，`DRIFT-220-1` 闭环；阶段 1a 结论仍为 **GO**）
当前阶段    ：**阶段 1（三试点闭环）** —— stage-0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）
当前任务卡  ：**A5 批次进行中**：TASK-011~038 全 Done；**TASK-039 ✅（阶段 1a GO）**；**TASK-102 ✅**、**TASK-103 ✅**、**TASK-104 ✅**、**TASK-105 ✅**、**TASK-210 ✅**、**TASK-087 ✅**、**TASK-212 ✅**、**TASK-211 ✅**、**TASK-213 ✅**、**TASK-214 ✅**、**TASK-205 ✅**、**TASK-215 ✅（PL-097 闭环）**；**TASK-216 ✅（`DRIFT-216-4` 由 TASK-217 闭环）**；**TASK-217 ✅（ADR-0061 Accepted）**；**TASK-218 ✅**、**TASK-219 ✅**、**TASK-221 ✅**、**TASK-222 ✅**；**TASK-085 ✅（hygiene 8/13）**；**TASK-086 ✅（hygiene 11/13）**；**TASK-223 ✅（ADR-0064 文件通道 + ADR-0065 前置初始指纹；`DRIFT-223-1` / `PL-100` 闭环）**；**TASK-220 ✅（元素表 4096 / anchor 64 / grant 1024 上限与淘汰 + 进程树 kill/reap；真机 ignored 8 passed）**；**TASK-225 ✅（合成输入目标租约独占；`PL-101` 闭环）**；**TASK-226 ✅（PL-104 / PL-105 测试卫生闭环）**；下一张待 Orchestrator 派单；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203 / 204 / 205** 均 Done
阻塞项      ：① TASK-002 仍 Blocked；② **PL-092**（storage 缺 conversation/session 公开记录 API）；③ **PL-094**（`RoleAndParent` helper 候选需契约治理）；④ gov **#9 覆盖率**与 **#11 `cargo doc`** 仍是 SOFT 门禁
下一步动作  ：1a 补救批次已收口，PL-104 / PL-105 已由 **TASK-226** 闭环；下一张由 Orchestrator 派单；**TASK-040** 真机校准已通过（4 passed / 0 failed），跨层目标 lease 已由 **TASK-225** 闭环（`PL-101` 关闭）；TASK-040 正文 DoD 复选框仍因正文只读未勾选，等待 Orchestrator 收口。阶段 1a 已完成复验，结论仍为 **GO**。
                  → 复验证据见 `docs/audits/stage-1a-reaudit-2026-10-02.md`；详见 `plans/stage-1-pilots.md` 与 `MEMORY.md` §1（派生值一律不写进本文件，ADR-0030 D2）
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
