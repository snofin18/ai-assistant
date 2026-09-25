# PLAN.md — 进度事实源（索引）

> **本文件刻意保持在 60 行内**：它是索引与当前状态，细节全部下放到 `plans/<阶段>.md`，
> 以便 agent **按步最小化读取**（启动只读本文件 + 当前阶段文件 + 自己的任务卡）。
> 冲突裁决顺序见 `AGENTS.md`。agent **不得修改本文件**，只能提案。

## 当前状态

```text
更新日期    ：2026-09-25（TASK-023 `verify` 后置断言引擎 Done：§7.4 的 11 种断言 + §7.3 状态指纹 + §8.5 幂等判定 + `on_violation` 分派；**验证失败绝不返回 ok** —— `Violated` / `Inconclusive` 都带 `VerifyFailed`；同批新增 PL-086）
当前阶段    ：**阶段 1（三试点闭环）** —— stage-0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）
当前任务卡  ：**A3 批次进行中**：TASK-011 ✅ / 012 ✅ / 013 ✅ / 014 ✅ / 015 ✅ / 016 ✅ / 017 ✅ / 018 ✅ / 019 ✅ / 020 ✅ / 021 ✅ / 022 ✅ / **023 ✅** → **TASK-024（`undo`：可逆性四级 + 锚点 + 回滚剧本 + 冲突检测）= 下一张**（依赖 012+023 已解锁；025 / 026 / 027 亦已解锁，可并行）；
                  跨阶段治理卡 **TASK-200 / 201 / 202 / 203 / 204** 均 Done
阻塞项      ：① TASK-002 仍 Blocked（上游 `create_thread` 未关）；② gov §5.1 门禁清单尚未登记 `check-migrations`（PL-056，需 ADR）；③ PL-071（ADR-0035 allow 表未登记 crate 级 `unsafe_code`）/ PL-072（`*Regex` 命名与子串语义不一致）/ PL-073（卡片 `- 状态：` 行归属机制）/ **PL-074**（`pointer_action` 不带目标窗口 → 混合 DPI 多屏下逻辑点无法唯一归属显示器，改公共接口需 ADR）需 ADR 或 Orchestrator 处置；④ **PL-078**（`xtask codegen` 生成类型无构造器/builder → 信封与审计事件只能 serde 组装）/ **PL-079**（`crates/storage` 与 `crates/tool-bus` 各自一份 `Clock` trait，建议抽 `assistant-time`）/ **PL-080**（元工具名 `toolset.list`/`toolset.search` 两段式与 `docs/spec/tool-schema.md` §4 不变量 1 冲突）/ **PL-082**（`PolicyDecision` 无法承载 confirmation scope/show_diff，安全投影有损，扩展 schema 需 ADR）/ **PL-083**（storage 缺 task 行更新入口 → 最新 Task 状态在 checkpoint，不在 `tasks.status`）；⑤ **PL-084**（待实测：UI 保存是否抹掉 `COUNT=1`）/ **PL-085**（一次性 automation 只在工具侧建删，避开 UI 往返）；⑥ **PL-086**（架构 v2 附录 A 的 `assert` 自由字符串未实现 —— 只接受结构化 `field`+`op`+`value`，需人类裁决是否补表达式语言）
下一步动作  ：① TASK-024 `crates/undo/**`：可逆性四级 + 锚点（内容快照 / 影子副本 / 步数级）+ 回滚剧本执行 + 冲突检测 + incident 上报
                  → ② TASK-025 / 026 / 027 亦可并行；详见 `plans/stage-1-pilots.md` 与 `MEMORY.md` §1（派生值一律不写进本文件，ADR-0030 D2）
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
