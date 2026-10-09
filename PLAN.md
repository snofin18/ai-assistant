# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-10-09（**TASK-257 Done**：ADR-0086 `SelectorKind::ExactName` 落地，UIA 精确 Name / replay 精确匹配 / 空值与非 Text fail-closed；**TASK-256 Review**：等待恢复 handler read-back 校准；**TASK-106 Done**：Paint T3.1 运行时装配）
当前阶段    ：**TASK-255 Done**（PL-057 旧日志目录引用 live scope 清零并闭环）；**TASK-254 Done**（PL-027 末行换行清扫与 Error 化；PR #278 / merge `1aa082b`）；**TASK-253 Done**（ADR-0083 状态行唯一可写例外；`PL-073` 闭环）；**TASK-244 Done**：PL-022 机器派生计数收口（ADR-0075）；**阶段 1（三试点闭环）**；**TASK-041 Done（拆分 A + B）**：ADR-0073 零依赖纯截图管线 / 脱敏规则模型 + ADR-0076 Windows GDI 单窗口截图 / 像素遮挡 / `ImageBlobSink` 注入（`DRIFT-041-2` / `PL-112` 闭环）—— stage-0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）；**TASK-231 Done**：ADR-0067 已把 pointer 起点与 `DragTo` 释放点的 `CoordinateSpace` 显式化并删除混合 DPI 收敛启发式（`PL-074` 闭环）；**TASK-233 Done**：生产装配接入 storage 会话持久化 —— 新增 `StorageSessionStore` 适配器与 `with_storage_session_store()` 开关，生产两处不再注入 `MemorySessionStore`（`PL-108` 闭环）
当前任务卡  ：**TASK-106 ✅**（Paint T3.1 运行时装配；`DRIFT-106-1` / `DRIFT-106-2` 闭环；PR #284 / merge `f5bdfa6`）；**TASK-255 Done**（PL-057 旧日志目录引用 live scope 清零并闭环）；**TASK-254 Done**（PL-027 末行换行清扫与 Error 化；PR #278 / merge `1aa082b`）；**A5 批次进行中**：TASK-011~038 全 Done；**TASK-039 ✅（阶段 1a GO）**；**TASK-102 ✅**、**TASK-103 ✅**、**TASK-104 ✅**、**TASK-105 ✅**、**TASK-210 ✅**、**TASK-087 ✅**、**TASK-212 ✅**、**TASK-211 ✅**、**TASK-213 ✅**、**TASK-214 ✅**、**TASK-205 ✅**、**TASK-215 ✅（PL-097 闭环）**；**TASK-216 ✅（`DRIFT-216-4` 由 TASK-217 闭环）**；**TASK-217 ✅（ADR-0061 Accepted）**；**TASK-218 ✅**、**TASK-219 ✅**、**TASK-221 ✅**、**TASK-222 ✅**；**TASK-085 ✅（hygiene 8/13）**；**TASK-086 ✅（hygiene 11/13）**；**TASK-223 ✅（ADR-0064 文件通道 + ADR-0065 前置初始指纹；`DRIFT-223-1` / `PL-100` 闭环）**；**TASK-220 ✅（元素表 4096 / anchor 64 / grant 1024 上限与淘汰 + 进程树 kill/reap；真机 ignored 8 passed）**；**TASK-225 ✅（合成输入目标租约独占；`PL-101` 闭环）**；**TASK-226 ✅（PL-104 / PL-105 测试卫生闭环）**；**TASK-227 ✅（状态行收口 + PL-103 改法 + PL-106 开卡）**；**TASK-229 ✅（PL-103 spec 落笔 + 状态行硬规则 + 残留目录清理）**；**TASK-228 ✅（真机结构化记录 + 严格汇总入口；`PL-106` 闭环）**；**TASK-224 ✅（ADR-0066 命令行 write 通道 + Windows 独占占用探测；`PL-107` 闭环）**；**TASK-230 ✅（storage 会话 / 消息树持久化 API；`PL-092` 闭环）**；**TASK-232 ✅（四轮自动化审计收口；`PL-108` 登记）**；**TASK-233 ✅（生产装配接入 storage 会话持久化；`PL-108` 闭环）**；**TASK-234 ✅（ADR-0068 / 0069 + hygiene 13/13；`PL-060` 闭环；PR #218 / `e417a2a`）**；**TASK-235 ✅（ADR-0070 `RoleAndParent` 父候选仅作用域；`PL-094` 闭环；PR #220 / `8d9241d`）**；**TASK-236 ✅（replay 完整版：Recording v2 序列 + 树级 diff + `--suite core`；PR #223 / `d7f61c8`）**；**TASK-237 ✅（三连发自动化审计收口；`PL-062` 闭环）**；**TASK-238 ✅（ADR-0071 `capture` / `dlp` 边界骨架；`PL-102` 闭环；解锁 1b TASK-041 / 042）**；**TASK-239 ✅（停车位存量复核报告 + PARKING_LOT 只追加复核；13 条补记闭环/取代、35 条复核保留）**；**TASK-240 ✅（全量状态行清扫：18 张滞后卡按 LEDGER/main/测试证据更正；零代码改动）**；**TASK-241 ✅（ADR-0072 派生值指针化 + PL-035 闭环；PL-022 机器派生余项已由 TASK-244 闭环）**；**TASK-242 ✅（三连发审计收口：TASK-076 状态行 + TASK-002 落点 PL-109）**；**TASK-243 ✅（PL-109 裁决：TASK-002 = Done，各处协调一致）**；**TASK-041 ✅（截图管线拆分 A：纯逻辑 + ADR-0073）**；**TASK-249 ✅（ADR-0079；`visual_assert` 图像来源与运行时接线）**；**TASK-050 ✅（ADR 0007 三档出域策略；22 专项测试）**；**TASK-053 ✅（静态注入靶页 fixture；6 类 marker + 5 条正常提取记录；PR #260 / `79b5566`）**；**TASK-250 ✅（refscan 待建 ADR 集合对齐登记表；PL-099 / DRIFT-W4-1 闭环；PR #262 / `7be80b6`）**；**TASK-251 ✅（ADR-0027 Accepted + `docs/spec/testing.md` §4.4 测试 lint 允许清单；PL-098 / DRIFT-W3-1 闭环）**；**TASK-253 ✅（ADR-0083 任务卡状态行唯一可写例外；PL-073 闭环）**；下一张待 Orchestrator 派单；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203 / 204 / 205** 均 Done
本日自动化 ：TASK-050 / TASK-053 / TASK-250 / TASK-251 / TASK-253 / TASK-254 / TASK-255 均已收口；2026-10-07 运行日阶段报告已补轮次 5~7 的收口段。
最新完成卡 ：**TASK-257 Done**（ADR-0086 ExactName selector contract；API / Windows / replay / Paint fake 与 adapter 契约全绿）；此前 **TASK-106 Done**（ADR-0085 + Paint T3.1 7-handler 运行时装配；PR #284 / merge `f5bdfa6`）
最新实现卡 ：**TASK-257 ✅** - Paint 形状 / 调色板容器 ExactName 锚点；`SelectorKind::ExactName` + UIA 精确条件 + replay fail-closed + selector pack 兜底；此前 **TASK-106 ✅** - Paint T3.1 runtime assembly
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
