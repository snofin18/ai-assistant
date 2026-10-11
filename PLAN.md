# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-10-11（**TASK-264 Done**（`crates/tool-bus` 独立行覆盖率 73.12% → 85.62%；380 行公共契约测试；生产源码零改动）；此前 **TASK-263 Done**（replay 89.77%）、**TASK-262 Done**（automation-host 83.39%））
当前阶段    ：**阶段 1（三试点闭环）进行中**；**TASK-264 Done**（`crates/tool-bus` 独立行覆盖率 73.12% → 85.62%；380 行公共契约测试；生产源码零改动）；**TASK-263 Done**（replay 89.77%）；此前阶段 1a GO 与 Paint T3.1 运行时装配均已收口。
当前任务卡  ：**TASK-264 Done**（`crates/tool-bus` 独立行覆盖率 73.12% → 85.62%；380 行公共契约测试；生产源码零改动）；**TASK-263 Done**（`crates/replay` 独立行覆盖率 89.77%；PR #302 / merge `53c9418`）；**TASK-262 Done**（`assistant-automation-host` 独立行覆盖率 83.39%；PR #300 / merge `81f196a`）；**TASK-257 Review**（ADR-0086 `SelectorKind::ExactName`）；**TASK-256 Review**（等待真机 Paint 校准）；下一张待 Orchestrator 派单；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203 / 204 / 205** 均 Done
本日自动化 ：TASK-264 / TASK-263 / TASK-262 / TASK-261 / TASK-260 / TASK-259 / TASK-258 等质量加固已收口；TASK-256 真机验收仍待有人值守流程。
最新完成卡 ：**TASK-264 Done**（`crates/tool-bus` 独立行覆盖率 85.62%）；此前 **TASK-263 Done**（replay 89.77%）、**TASK-262 Done**（automation-host 83.39%）。
最新实现卡 ：**TASK-264 Done** - tool-bus 公共 API 契约覆盖率提升到 85.62%；此前 **TASK-263 Done** - replay 契约覆盖率提升到 89.77%。
阻塞项      ：gov **#9 覆盖率**与 **#11 `cargo doc`** 仍是 SOFT 门禁（TASK-002 已于 2026-10-05 裁决为 **Done**，不再列阻塞）
下一步动作  ：**TASK-256** — Review：打开真实 Paint 后执行 `production_paint` ignored 验收；通过后再进入 TASK-044 真机十次运行；承接 `DRIFT-256-1` / `DRIFT-044-1` / `PL-113`
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
