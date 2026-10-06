# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-10-06（**TASK-244 Done**：PL-022 机器派生计数收口 —— ADR-0075 冻结 gov §5.4 表格行数为 hygiene 总数 SSOT，gov §5.1 ↔ ci.yml 的 # gov-gate 集一致性为 Error；TOTAL_HYGIENE_RULE_COUNT 硬编码删除；**TASK-041 Done**：截图管线拆分 A —— ADR-0073 冻结像素 / 隐私 / 滚动边界，`crates/capture` + `crates/dlp/src/redact` 零依赖纯逻辑与 15 个专项测试落地；**TASK-242 Done**：三连发审计收口 —— 补正 TASK-076 滞后状态行（LEDGER L88 早已声称 Done，commit `0f05ef7` 实际未改），TASK-002 卡面 / PLAN / LEDGER 三方不一致开成 **PL-109** 待裁决；**TASK-239 Done**：停车位存量复核收口，新增 `docs/audits/parking-lot-review-2026-10-05.md`；`docs/PARKING_LOT.md` 仅追加 48 行结论，补记 13 条已闭环/取代、保留 35 条待治理或人工裁决；**TASK-238 Done**：ADR-0071 冻结截图管线 / 出域脱敏的 crate 边界，新增 `crates/capture` + `crates/dlp` 两个零第三方依赖骨架并闭环 `PL-102`，解锁 1b TASK-041 / 042 与 1c TASK-050；**TASK-237 Done**：三连发自动化审计收口（`PL-062` 闭环 + 孤儿自动化目录清理 + TASK-235 状态行归一）；**TASK-236 Done**：Recording v2 真实 UIA 树快照序列 + 树级 diff + `--suite core`，`--list-deferred` replay 未实现项清零；**TASK-235 Done**：ADR-0070 明确 `RoleAndParent` 父候选仅作用域，`PL-094` 闭环（PR #220 / merge `8d9241d`；CI 11/11）；**TASK-234 Done**：ADR-0068 / 0069 Accepted，gov §5.4 hygiene 13/13，重复代码与顶层目录白名单规则及正负样本落地，`PL-060` 闭环（PR #218 / merge `e417a2a`；CI 11/11）；**TASK-233 Done**：生产装配接入 storage 会话持久化（`StorageSessionStore` 适配器 + `with_storage_session_store()` 开关；生产 `production.rs` / `main.rs` 两处不再注入 `MemorySessionStore`；`PL-108` 闭环）；**TASK-230 Done**：storage `0005` 建 `conversations` + `conversation_messages`，跨重启持久化测试与非法父子 / 缺失会话负向证据到位，`PL-092` 闭环；**TASK-224 Done**：ADR-0066 命令行 `write` 唯一通道 + Windows `share_mode(0)` 占用探测 + 有界退避，`PL-107` 闭环；**TASK-228 Done**：真机验收 PASS/SKIP/FAIL 结构化记录 + 严格汇总入口，真机 4 pass / 0 skip / 0 fail，`PL-106` 闭环；**TASK-226 Done**：PL-104 / PL-105 闭环，ignored 真机用例并行风险以异步互斥消除，`production_root.rs` 拆分后 3 个文件均 <600 行；**TASK-225 Done**：Host 装配层接上合成输入目标租约独占，两个任务冲突稳定 `Transient`、释放后重试、失败回基线与 pointer-shaped 专项 5 passed，`PL-101` 闭环；**TASK-223 Done**：ADR-0064 文件通道 + ADR-0065 前置初始指纹步骤落地，T1.1 大文件 Plan 在 fake 平台与真机 UIA 各取证跑通，`DRIFT-223-1` / `PL-100` 闭环；**TASK-220 Done**：资源泄露四类防护（元素表 / anchor / grant 上限与淘汰 + 进程树终止与 reap）经真机 ignored 全套 8 passed，`DRIFT-220-1` 闭环；**TASK-227 Done**：TASK-040 / TASK-225 状态行按实更正为 Done，`PL-103` 给出可直接粘贴的 spec 改法、`PL-106` 开卡 TASK-228（Ready）；**TASK-229 Done**：`PL-103` 已按该改法落笔（spec §3 改为指向 `RESERVED_RUNTIME_TOOLS` 唯一事实源）、`docs/memory/pitfalls.md` 落「收口类提交必须逐张比对状态行与 LEDGER」四步硬规则、轮次 A 残留目录已清理；**TASK-232 Done**：四轮自动化审计收口（TASK-224 滞后状态行按实修正、state-line 比对规则补强、storage 轮的 `SessionStore` 适配器遗留转 `PL-108`）；阶段 1a 结论仍为 **GO**）
当前阶段    ：**TASK-244 Done**：PL-022 机器派生计数收口（ADR-0075）；**阶段 1（三试点闭环）**；**TASK-041 Done（拆分 A + B）**：ADR-0073 零依赖纯截图管线 / 脱敏规则模型 + ADR-0076 Windows GDI 单窗口截图 / 像素遮挡 / `ImageBlobSink` 注入（`DRIFT-041-2` / `PL-112` 闭环）—— stage-0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）；**TASK-231 Done**：ADR-0067 已把 pointer 起点与 `DragTo` 释放点的 `CoordinateSpace` 显式化并删除混合 DPI 收敛启发式（`PL-074` 闭环）；**TASK-233 Done**：生产装配接入 storage 会话持久化 —— 新增 `StorageSessionStore` 适配器与 `with_storage_session_store()` 开关，生产两处不再注入 `MemorySessionStore`（`PL-108` 闭环）
当前任务卡  ：**A5 批次进行中**：TASK-011~038 全 Done；**TASK-039 ✅（阶段 1a GO）**；**TASK-102 ✅**、**TASK-103 ✅**、**TASK-104 ✅**、**TASK-105 ✅**、**TASK-210 ✅**、**TASK-087 ✅**、**TASK-212 ✅**、**TASK-211 ✅**、**TASK-213 ✅**、**TASK-214 ✅**、**TASK-205 ✅**、**TASK-215 ✅（PL-097 闭环）**；**TASK-216 ✅（`DRIFT-216-4` 由 TASK-217 闭环）**；**TASK-217 ✅（ADR-0061 Accepted）**；**TASK-218 ✅**、**TASK-219 ✅**、**TASK-221 ✅**、**TASK-222 ✅**；**TASK-085 ✅（hygiene 8/13）**；**TASK-086 ✅（hygiene 11/13）**；**TASK-223 ✅（ADR-0064 文件通道 + ADR-0065 前置初始指纹；`DRIFT-223-1` / `PL-100` 闭环）**；**TASK-220 ✅（元素表 4096 / anchor 64 / grant 1024 上限与淘汰 + 进程树 kill/reap；真机 ignored 8 passed）**；**TASK-225 ✅（合成输入目标租约独占；`PL-101` 闭环）**；**TASK-226 ✅（PL-104 / PL-105 测试卫生闭环）**；**TASK-227 ✅（状态行收口 + PL-103 改法 + PL-106 开卡）**；**TASK-229 ✅（PL-103 spec 落笔 + 状态行硬规则 + 残留目录清理）**；**TASK-228 ✅（真机结构化记录 + 严格汇总入口；`PL-106` 闭环）**；**TASK-224 ✅（ADR-0066 命令行 write 通道 + Windows 独占占用探测；`PL-107` 闭环）**；**TASK-230 ✅（storage 会话 / 消息树持久化 API；`PL-092` 闭环）**；**TASK-232 ✅（四轮自动化审计收口；`PL-108` 登记）**；**TASK-233 ✅（生产装配接入 storage 会话持久化；`PL-108` 闭环）**；**TASK-234 ✅（ADR-0068 / 0069 + hygiene 13/13；`PL-060` 闭环；PR #218 / `e417a2a`）**；**TASK-235 ✅（ADR-0070 `RoleAndParent` 父候选仅作用域；`PL-094` 闭环；PR #220 / `8d9241d`）**；**TASK-236 ✅（replay 完整版：Recording v2 序列 + 树级 diff + `--suite core`；PR #223 / `d7f61c8`）**；**TASK-237 ✅（三连发自动化审计收口；`PL-062` 闭环）**；**TASK-238 ✅（ADR-0071 `capture` / `dlp` 边界骨架；`PL-102` 闭环；解锁 1b TASK-041 / 042）**；**TASK-239 ✅（停车位存量复核报告 + PARKING_LOT 只追加复核；13 条补记闭环/取代、35 条复核保留）**；**TASK-240 ✅（全量状态行清扫：18 张滞后卡按 LEDGER/main/测试证据更正；零代码改动）**；**TASK-241 ✅（ADR-0072 派生值指针化 + PL-035 闭环；PL-022 机器派生余项已由 TASK-244 闭环）**；**TASK-242 ✅（三连发审计收口：TASK-076 状态行 + TASK-002 落点 PL-109）**；**TASK-243 ✅（PL-109 裁决：TASK-002 = Done，各处协调一致）**；**TASK-041 ✅（截图管线拆分 A：纯逻辑 + ADR-0073）**；下一张待 Orchestrator 派单；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203 / 204 / 205** 均 Done
最新完成卡 ：**TASK-247 Done** —— 把 `visual_assert` 以**加法式**接进后置断言引擎（ADR-0077）：新增 `Postcondition::VisualAssert` 与 `*_with_visual` 入口，图像作为**并列参数**传入而不进可序列化的 `Observation`；无图 / 低置信一律 `NotEvaluable`（不判成功、不铸 receipt），既有三个入口签名与行为不变；`PL-110` / `DRIFT-042-1` 闭环
阻塞项      ：gov **#9 覆盖率**与 **#11 `cargo doc`** 仍是 SOFT 门禁（TASK-002 已于 2026-10-05 裁决为 **Done**，不再列阻塞）
下一步动作  ：**TASK-041 ✅（拆分 A + B）**（ADR-0073 纯截图管线 / 脱敏规则模型 + ADR-0076 Windows GDI 截图 / 像素遮挡 / `ImageBlobSink` 注入；真机截图与真机 blob 落盘由 TASK-246 取证）；**TASK-042 ✅**（视觉验证纯逻辑：ADR-0074 + `crates/verify/src/visual/**` 零依赖 + 21 专项测试；接入后置条件引擎另立卡 `DRIFT-042-1` / `PL-110`）；**TASK-245 ✅**（四连发审计收口：`LEDGER.md` 去重 404→400、完全重复行组归零；`PL-111` 落点 = DRIFT-041-2 storage sink 待裁决 + ADR-0074 号冲突 + DRIFT-042-1 / PL-110）；**TASK-244 ✅**（PL-022 机器派生计数收口：ADR-0075 + 运行时解析 + gov/CI 标记集合校验）；下一张由 Orchestrator 派单
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
