# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-09-29（**TASK-212 check-comments 9 处真实违规修复已 Done**：只加 30 行注释，`check-comments` 0 error；**TASK-087 CI 硬门禁负向验证与 check-comments 落地已进入 Review**：fmt/clippy/build 三类 canary 已写入 gate-selftest，`check-comments` 已接入 CI，待一次 gate-selftest 成功 run 后收口。真实 UI↔Core 传输仍缺（**PL-095**）。TASK-039 审计结论仍为阶段 1a **NO-GO**）
当前阶段    ：**阶段 1（三试点闭环）** —— stage-0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）
当前任务卡  ：**A5 批次进行中**：TASK-011~038 全 Done；**TASK-207 ✅**（Planner）、**TASK-206 ✅**（storage FTS5）、**TASK-208 ✅**（Memory）、**TASK-029 ✅**（binary 装配）、**TASK-030 ✅**（审批/时间线）、**TASK-031 ✅**（元素拾取/绑定）、**TASK-032 ✅**（策略/能力/成本）、**TASK-033 ✅**（notepad-like 靶机）、**TASK-034 ✅**（录制回放）、**TASK-035 ✅**（Notepad Adapter 声明式包）、**TASK-036 ✅**（T1.1 任务包/评测集）、**TASK-037 ✅**（T1.2 替换/保存/审批/撤销）与 **TASK-038 ✅**（T1.3 新建标签/写入/跨进程另存为）均 Done；**TASK-039 = Review（审计完成，阶段 1a NO-GO）**；**TASK-102 = Done（ADR-0056 Accepted）**、**TASK-103 ✅（真实执行器 + Host 分发 + VerifyReceipt 接线）**、**TASK-104 ✅（UI ↔ Core typed IPC 与审批接线）**、**TASK-210 ✅（UI 与提交质量门禁）**、**TASK-087 = Review（gate-selftest 成功 run 待回填）**、**TASK-212 ✅（check-comments 9 处真实违规修复）**；下一张 **TASK-105**（或先立 PL-095 的 UI↔Core 传输卡）；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203 / 204** 均 Done；**TASK-205** = Ready
阻塞项      ：① 真实执行器（TASK-103）与 UI↔Core 契约/审批接线（TASK-104）已落地，但**真实 UI↔Core 传输未接通（PL-095）**，且 T1.x 10 次运行成功率、静默失败、撤销成功率（TASK-105）仍未验证；② **TASK-087 只剩 gate-selftest 成功 run + PL-018 关闭回填**（fmt/clippy/build canary 与 `check-comments` CI 接线已实现）；③ TASK-002 仍 Blocked；④ **PL-092**（storage 缺 conversation/session 公开记录 API）；⑤ **PL-094**（`RoleAndParent` helper 候选需契约治理）；⑥ **PL-095**（UI↔Core 线上传输与事件推送待立卡 + ADR）
下一步动作  ：推送 PR #104 并手工触发 **gate-selftest**，成功后回填 run 号并收口 **TASK-087 / PL-018**；**TASK-105** 需先裁决 **PL-095** 的 UI↔Core 传输卡，且仍缺真实 ModelProvider 与 Notepad Host handler；最后由 **TASK-211** 收口停车位并触发 1a 复验。阻断项闭环前不得进入 1b。
                  → 审计证据见 `docs/audits/stage-1a-integration-audit-2026-09-29.md`；详见 `plans/stage-1-pilots.md` 与 `MEMORY.md` §1（派生值一律不写进本文件，ADR-0030 D2）
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
