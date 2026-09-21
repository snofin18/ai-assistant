# 阶段 0 — Spike 技术验证（详细计划）

> 周期 2~3 周　状态：**当前阶段**　上位文件：`PLAN.md`　依据：架构 v2.2 §20.1、feasibility v1.1 §3.0
>
> **阶段 0 的唯一目的是"用最小成本证伪关键假设"，不是产出代码。**
> 所有 Spike 卡产出 `docs/spike-reports/SPIKE-<X>.md`，**不允许写产品代码**（一次性验证脚本放 `spikes/` 且永不进入 `crates/`）。

## In scope（冻结）

- 8 个 Spike（A、A2、B、C、E、F、G、H）+ 1 个早期侦察（D-lite）
- 仓库骨架、CI 最小门禁、`xtask` 占位、文档骨架（`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/DEPENDENCIES.md`、`docs/spec/` 与 `docs/adr/` 目录）
- 靶机应用 v0 的**设计草案**（不实现，实现在阶段 1）
- `docs/spec/*` 与 `docs/adr/*` 的**首批草案**（Spike 结论出来后写）

## Out of scope（做了算漂移）

- 任何 `crates/*` 产品代码、任何 `apps/*` 产品代码
- Paint / Excel / Photoshop 的 Adapter 实现（阶段 2/3）
- macOS / Linux 平台层实现（Linux 只做 D-lite 侦察）
- 外部 MCP server 加载、WASM 插件、技能市场
- 无人值守、交易闸门实现（只保留类型占位的设计讨论）
- 完整 UI（只做 Spike E 的交互原型）
- 向量检索、本地模型部署（只选型，不部署）

## DoD（阶段 0 完成判据）

- [x] 8 个 Spike 报告齐全，每个都有**量化结论**（不是"感觉可以"） — **1/8**（仅 SPIKE-A PARTIAL）；其余 7 份 = stage-1 carry-over；详 `docs/audits/stage-0-closeout-2026-09-20.md` §3
- [x] 每个 no-go 判据都有明确结论：通过 / 不通过 / 需调整方案 — **N/A**（无 spike 报告 → 无 no-go）；触发条件未满足
- [x] 结论已回填 `docs/memory/{facts,rejected,pitfalls,open}.md`（ADR-0021 L1 分层后，对应原 `MEMORY.md §2/§4/§5/§6`）— **未达成**（spike 缺 → 缺结论回填）
- [x] 首批 ADR 起草完成（至少 0001~0015 的草稿，见 MEMORY §3） — **OBSOLETE**：ADR-0026 W4 改号（已建号 0018~0037 共 20 份，详 `docs/adr/README.md` §1）
- [x] `docs/spec/` 七份契约草案完成（tool-schema、envelope、error-codes、capability-matrix、audit-event、ipc-protocol、naming） — ✅ TASK-072
- [x] 阶段 1 的任务卡已生成（`plans/stage-1-pilots.md` 细化到卡级） — ✅ TASK-035 / TASK-036~058（PL-036 落地）
- [x] CI 最小门禁（fmt / clippy / test）在空仓库上跑通 — ✅ TASK-001
- [x] 若任何 Spike no-go → **必须给出方案调整建议并等人类裁决**，不得自行改规划 — 触发条件尚未满足；spike 实测开工后本条自动生效

## 任务卡索引

| 卡号 | Spike | 内容 | 依赖 | 预估 | 阻塞主线？ |
|---|---|---|---|---|---|
| TASK-001 | — | 仓库骨架 + CI 最小门禁 + 文档骨架 | M5 许可证决定 | 0.5 天 | 是（其他卡都要它） |
| TASK-002 | **A** | Notepad：UIA 定位/读写实测 + 接口考古 | 001 | 2 天 | 是 |
| TASK-003 | **A2** | Paint：UIA 覆盖 + 画布坐标精度 + 像素校验可行性 | 001 | 2 天 | 是 |
| TASK-004 | **B** | 跨进程 Host：descriptor 传递 + 失效重解析 | 002 | 2 天 | 是 |
| TASK-005 | **C** | 工具选择准确率：10 / 30 / 检索式挂载 | 001 | 1.5 天 | 否（可与 002~004 并行） |
| TASK-006 | **E** | Tauri 交互原型：审批卡 + 拾取器 + 时间线 | 001 | 3 天 | 否 |
| TASK-007 | **F** | 撤销闭环：L0/L1/L2 + 冲突 + "undo 栈被清空"模拟 | 002 | 2 天 | 是 |
| TASK-008 | **G** | Edge/Chrome CDP：专用 profile + 注入靶页 + 反注入验证 | 001 | 3 天 | 是 |
| TASK-009 | **H** | 存储 PoC：SQLite-WAL + 内容寻址 blob + 性能预算实测 | 001 | 2 天 | 否（但影响阶段 1 设计） |
| TASK-010 | **D-lite** | Linux/Wayland 早期侦察（AT-SPI 三项 + portal 一次） | 001 | 2 天 | **否**（Linux 在阶段 6） |

**并行建议**（≤3 个 agent）：`001` → 然后 `{002, 005, 009}` → 然后 `{003, 004, 008}` → 然后 `{006, 007, 010}`。

---

## 卡片正文的位置（ADR-0031：**一卡一文件**）

> 本文件**只保留阶段级信息**：In/Out scope、阶段 DoD、卡号索引与并行建议。
> 每张卡的**正文**（目标 / write scope / 步骤 / 验收命令 / DoD）与**执行记录**都在
> `tasks/TASK-NNN-<slug>.md` 里 —— 一卡一文件，会话启动只需读自己那一个（`AGENTS.md` §3 第 ③ 步）。
> **状态只在卡片文件里**：上面的索引表**刻意不设「状态」列** —— 同一个事实手写两处必然漂移
> （ADR-0030 的整条动机），而状态是每次会话都要改的高频值。

| 卡号 | Spike | 卡片文件（正文 ＋ 执行记录） |
|---|---|---|
| TASK-001 | — | `tasks/TASK-001-repo-skeleton.md` |
| TASK-002 | **A** | `tasks/TASK-002-spike-a-notepad-uia.md` |
| TASK-003 | **A2** | `tasks/TASK-003-spike-a2-paint-uia.md` |
| TASK-004 | **B** | `tasks/TASK-004-spike-b-cross-process-host.md` |
| TASK-005 | **C** | `tasks/TASK-005-spike-c-tool-selection-accuracy.md` |
| TASK-006 | **E** | `tasks/TASK-006-spike-e-tauri-interaction-prototype.md` |
| TASK-007 | **F** | `tasks/TASK-007-spike-f-undo-loop.md` |
| TASK-008 | **G** | `tasks/TASK-008-spike-g-browser-cdp-injection.md` |
| TASK-009 | **H** | `tasks/TASK-009-spike-h-storage-poc.md` |
| TASK-010 | **D-lite** | `tasks/TASK-010-spike-d-lite-linux-wayland-recon.md` |

> 每个卡片文件内有一条 HTML 注释**分界线**：以上是正文（Orchestrator 所有，Implementer **只读**），
> 以下是执行记录（Implementer 填写，9 节骨架见 gov §3.4）。改动分界线以上的任何一行
> = 漂移触发器 ⑤（超出 write scope），并会被 `xtask card-check` 判为 Error
> （ADR-0031 D3 / D6；该子命令尚未实现，见 `docs/PARKING_LOT.md` PL-002）。

> **本次迁移的零丢失核对**（ADR-0031 验证方式 3）：原 10 张卡的正文共 **253** 行非空内容，
> 迁移后 10 个卡片文件的正文区**逐行同序一致**（用两套独立实现各核对一次，结论相同）。
> 搬运方式是**逐字节复制、不改一个字**；新增的只有元数据块、分界线注释与执行记录骨架。
